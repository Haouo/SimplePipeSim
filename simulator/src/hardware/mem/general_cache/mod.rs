// sub-modules
pub mod cache_set; // model for single cache set (might contains multiple ways)
pub mod prefetcher; // model for cache prefetcher (Null, NextLine, ...)
pub mod replacement_policy; // model for cache replacement policy (e.g., Random, FIFO, LRU)
pub mod statistic; // utils of statistics for cache
pub mod write_policy; // model for cache write policy (WB/WT × WA/NWA)

use super::super::statistic::Statistic;
use crate::hardware::clock::Clocked;
use crate::hardware::mem::abstract_mem::*;
use cache_set::GeneralCacheSetUnit;
use prefetcher::{Prefetcher, PrefetcherKind};
use replacement_policy::ReplacementPolicy;
use statistic::StatisticInfo;
use write_policy::WritePolicy;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

/// Default additional miss penalty in cycles, used when
/// [`GeneralCacheConfig::with_miss_penalty`] is not called.
const DEFAULT_MISS_PENALTY_CYCLES: usize = 5;

// a special struct data type for cache configuring
#[derive(Default, Clone)]
pub struct GeneralCacheConfig {
    name: String,
    total_size: Option<usize>,
    block_size: Option<usize>,
    num_of_way: Option<usize>,
    /// Additional cycles charged after a miss resolves, on top of the
    /// time taken to fetch from the next-level memory. Models tag-array
    /// re-access and the pipeline bubble while the refilled block becomes
    /// visible. `None` means "use the default".
    miss_penalty: Option<usize>,
    /// Write policy. `None` ⇒ `WritePolicy::default()` (write-back +
    /// write-allocate, the historical behaviour).
    write_policy: Option<WritePolicy>,
    /// Hardware prefetcher. `None` ⇒ `PrefetcherKind::default()`
    /// (`Null`, i.e. disabled).
    prefetcher_kind: Option<PrefetcherKind>,
}

impl GeneralCacheConfig {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn with_total_size(&self, size_in_bytes: usize) -> Self {
        let mut new_self = self.clone();
        new_self.total_size = Some(size_in_bytes);
        new_self
    }

    pub fn with_block_size(&self, size_in_bytes: usize) -> Self {
        assert!(
            self.total_size != None,
            "You have to configure total size first befor configuring block size!"
        );
        let mut new_self = self.clone();
        new_self.block_size = Some(size_in_bytes);
        new_self
    }

    pub fn with_num_of_way(&self, ways: usize) -> Self {
        assert!(
            self.block_size != None,
            "You have to configure block size before configuring number of ways!"
        );
        let mut new_self = self.clone();
        new_self.num_of_way = Some(ways);
        new_self
    }

    /// Set the additional miss penalty (in cycles) for this cache level.
    ///
    /// If not set, the cache uses [`DEFAULT_MISS_PENALTY_CYCLES`] (5). Use this
    /// to express that, for example, an L1 cache has a smaller post-miss
    /// recovery cost than an L2 cache that drives main memory.
    pub fn with_miss_penalty(&self, cycles: usize) -> Self {
        let mut new_self = self.clone();
        new_self.miss_penalty = Some(cycles);
        new_self
    }

    /// Set the write policy. Defaults to write-back + write-allocate.
    pub fn with_write_policy(&self, wp: WritePolicy) -> Self {
        let mut new_self = self.clone();
        new_self.write_policy = Some(wp);
        new_self
    }

    /// Set the hardware prefetcher kind. Defaults to `Null` (disabled).
    pub fn with_prefetcher_kind(&self, kind: PrefetcherKind) -> Self {
        let mut new_self = self.clone();
        new_self.prefetcher_kind = Some(kind);
        new_self
    }
}

/// States for the main FSM that drives a `GeneralCache`.
///
/// External timing contract
/// ------------------------
/// The cache is "synchronous from the requester's point of view":
/// a hit completes in the **same cycle** as `try_register_req`. To make
/// that work while still modelling the controller as a clocked FSM, the
/// `try_register_req` path calls `tick()` twice in-line right after
/// storing the new request — see the "Synchronous-hit pre-tick" note in
/// [`AbstractMemoryInterface::try_register_req`] below.
///
/// State diagram
/// -------------
/// ```text
///                        ┌───────────────────────────────┐
///                        ▼                               │
///   ┌──────┐ pending_req ┌────────┐ hit                 │
///   │ Idle │────────────▶│ Lookup │─────────────────────┘
///   └──────┘             └────────┘
///       ▲                    │ miss
///       │                    ▼
///       │           ┌──────────────────┐  dirty victim
///       │           │ select victim    │─────────────┐
///       │           └──────────────────┘             │
///       │                    │ clean victim          ▼
///       │                    │              ┌────────────────┐
///       │                    │              │ WriteBack      │
///       │                    │              │ (SendReq /     │
///       │                    │              │  WaitForCompl) │
///       │                    │              └────────┬───────┘
///       │                    ▼                       ▼
///       │           ┌──────────────────────────────────┐
///       │           │ Allocate                         │
///       │           │ (SendReq / WaitForComplete)      │
///       │           └────────────────┬─────────────────┘
///       │                            ▼
///       │           ┌──────────────────────────────────┐
///       │           │ AdditionalMissPenalty(countdown) │
///       │           └────────────────┬─────────────────┘
///       │                            │ countdown reaches 0,
///       └────────────────────────────┘ re-enter Lookup to finish
///                                      the original request
/// ```
///
/// `WriteBack` and `Allocate` each carry a small secondary FSM
/// (`StatesForOutMemReq`) plus the index of the way being evicted.
enum MainStates {
    /// No request in flight, no pending request to start.
    Idle,
    /// Tag-compare against the indexed set. On hit, complete in this tick;
    /// on miss, transition to WriteBack (dirty victim) or Allocate (clean).
    Lookup(MemoryReqType),
    /// Sending or waiting for a write-back of a dirty victim to the next
    /// level of memory. `usize` is the evicted way index.
    WriteBack(StatesForOutMemReq, usize),
    /// Sending or waiting for a refill from the next level of memory.
    /// `usize` is the way index the refill will be inserted into.
    Allocate(StatesForOutMemReq, usize),
    /// Stall for an additional fixed penalty after a miss completes,
    /// modelling tag-array re-access / pipeline bubble cost.
    AdditionalMissPenalty(usize),
    /// Used by write-through policies after a store hit: the cache copy
    /// has already been updated; this state drives the same store to the
    /// next-level memory. When the next-level store completes we mark
    /// the original requester's `done` cell and return to Idle.
    WriteThroughCommit(StatesForOutMemReq, Rc<Cell<bool>>),
    /// Used by no-write-allocate policies on a store miss: the cache
    /// stays untouched and the store goes straight to the next level.
    /// On completion we mark the original requester's `done` and return
    /// to Idle.
    WriteAround(StatesForOutMemReq, Rc<Cell<bool>>),
}

/// States of secondary FSM to handling memory requests to next-level memory.
/// This is used when writing-back or allocating.
enum StatesForOutMemReq {
    SendReq(MemoryReqType),
    WaitForComplete(MemoryReqType),
}

pub struct GeneralCache<RP: ReplacementPolicy, M: AbstractMemoryInterface> {
    // configuration metadata
    offset_bit_width: usize,
    index_bit_width: usize,

    // necessary data
    // The main data struct to cache data.
    set: Box<[GeneralCacheSetUnit<RP>]>,
    // current state of GeneralCache
    fsm: MainStates,
    // The `Rc<_>` wrapping is for shared ownership
    // because SimpleMem might be shared simultaneously by I$ and D$.
    // The `RefCell<_>` wrapping is for mutability because cache might perform write-back
    mem_ref: Rc<RefCell<M>>,

    // pending access request for GeneralCache
    pending_req: Option<MemoryReqType>,
    // backup of the request being handled when the cache is writing-back or allocating
    backup_req: Option<MemoryReqType>,

    // additional cycles charged after a miss resolves (see GeneralCacheConfig::with_miss_penalty)
    miss_penalty: usize,

    // write policy in effect (see GeneralCacheConfig::with_write_policy)
    write_policy: WritePolicy,

    // -------- prefetcher subsystem --------
    // Pluggable predictor: inspects each demand-access Lookup and may
    // queue block-aligned addresses to fetch ahead of time.
    prefetcher: Box<dyn Prefetcher>,
    // Addresses the prefetcher wants us to fetch. Drained one-at-a-time
    // from the Idle arm when no demand request is pending and no prefetch
    // is already in flight.
    prefetch_queue: VecDeque<u32>,
    // True while a synthetic prefetch request is occupying the FSM
    // (Lookup → Allocate → MissPenalty → Lookup). Demand-access HPM
    // counters and re-prediction are suppressed while this flag is set.
    prefetch_in_flight: bool,
    // Block size in bytes; used to size the synthetic prefetch load.
    block_size_bytes: usize,

    // statistic information
    pub hpm: StatisticInfo,
}

impl<RP: ReplacementPolicy, M: AbstractMemoryInterface> GeneralCache<RP, M> {
    // public methods
    /// only constructor for GeneralCache
    pub fn new(config: GeneralCacheConfig, mem_ref: Rc<RefCell<M>>) -> Self {
        let block_size = config.block_size.unwrap();
        let num_set = config.total_size.unwrap() / (block_size * config.num_of_way.unwrap());
        let prefetcher = config
            .prefetcher_kind
            .unwrap_or_default()
            .build(block_size);
        GeneralCache {
            offset_bit_width: block_size.ilog2() as usize,
            index_bit_width: num_set.ilog2() as usize,
            set: (0..num_set)
                .map(|_| GeneralCacheSetUnit::<RP>::new(config.num_of_way.unwrap(), block_size))
                .collect(),
            fsm: MainStates::Idle,
            mem_ref,
            pending_req: None,
            backup_req: None,
            miss_penalty: config.miss_penalty.unwrap_or(DEFAULT_MISS_PENALTY_CYCLES),
            write_policy: config.write_policy.unwrap_or_default(),
            prefetcher,
            prefetch_queue: VecDeque::new(),
            prefetch_in_flight: false,
            block_size_bytes: block_size,
            hpm: StatisticInfo::new(config.name),
        }
    }

    /// util function for address to (tag, index, offset) tuple
    fn addr_transfer(&self, addr: u32) -> (u32, usize, usize) {
        let offset = addr & ((1 << self.offset_bit_width) - 1);
        let index = (addr >> self.offset_bit_width) & ((1 << self.index_bit_width) - 1);
        let tag_bit_width = 32 - self.offset_bit_width - self.index_bit_width;
        let tag = (addr >> (32 - tag_bit_width)) & ((1 << tag_bit_width) - 1);

        (tag, index as usize, offset as usize)
    }
}

impl<RP: ReplacementPolicy, M: AbstractMemoryInterface> AbstractMemoryInterface for GeneralCache<RP, M> {
    fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()> {
        // check address alignment
        let mut alignment_check: bool = true;
        match req.get_len() {
            4 => {
                alignment_check = (req.get_addr() % 4) == 0;
            }
            2 => {
                alignment_check = (req.get_addr() % 2) == 0;
            }
            1 => {}
            _ => {
                alignment_check = false; // access with length larger than 4 bytes is not allowed
            }
        }
        assert!(
            alignment_check,
            "Cache Access Alignment Checking Fail!\n The Request Address is: {:#08X}, Length is: {}",
            req.get_addr(), req.get_len()
        );

        // try to register new request
        match &self.fsm {
            // can register new request if the current state is Idle
            // and self.pending_req is None
            MainStates::Idle if self.pending_req.is_none() => {
                self.pending_req = Some(req.clone());

                // Synchronous-hit pre-tick.
                //
                // External contract: a cache hit completes in the SAME cycle
                // as the call to `try_register_req`, i.e. `req.done == true`
                // returns to the caller before this method returns.
                //
                // Internal model: the FSM normally needs two ticks to do that
                // work:
                //   tick 1: Idle (with `pending_req` set) → Lookup
                //   tick 2: Lookup body runs tag-compare, and on hit fills
                //           the requester's buffer and sets `done = true`.
                //
                // We compress those two ticks into the registration call so
                // the caller sees a 1-cycle hit latency. On a miss the
                // second tick lands in the Lookup state and the FSM keeps
                // running over subsequent `tick()` calls as usual; the only
                // observable effect is that the miss-penalty timeline
                // starts one cycle earlier, which is the intended model.
                self.tick();
                self.tick();

                return Ok(());
            }
            _ => {}
        }
        Err(())
    }
}

impl<RP: ReplacementPolicy, M: AbstractMemoryInterface> Clocked for GeneralCache<RP, M> {
    /// tick function which is called in every cycles
    fn tick(&mut self) {
        match self.fsm {
            // * Idle, no demand request, but a prefetch is pending in the
            //   queue. Drain one prefetch and inject it as a synthetic
            //   Lookup. Placed BEFORE the demand-Idle arm so the guard
            //   makes the priority explicit (demand always wins because
            //   its guard requires `pending_req.is_some()`).
            MainStates::Idle
                if self.pending_req.is_none()
                    && !self.prefetch_in_flight
                    && !self.prefetch_queue.is_empty() =>
            {
                let addr = self.prefetch_queue.pop_front().unwrap();
                let len = self.block_size_bytes;
                let synthetic = MemoryReqType::Load(MemoryLoadReq {
                    addr,
                    len,
                    buffer: Rc::new(RefCell::new(vec![0u8; len].into_boxed_slice())),
                    done: Rc::new(Cell::new(false)),
                });
                self.prefetch_in_flight = true;
                self.hpm.prefetch_issued_cnt += 1;
                self.fsm = MainStates::Lookup(synthetic);
            }

            // * Idle state with pending request
            MainStates::Idle if self.pending_req.is_some() => {
                // accept new request and transfer current state
                self.fsm = MainStates::Lookup(self.pending_req.take().unwrap());
            }

            // * Lookup state
            MainStates::Lookup(ref req) => {
                // Snapshot the bits of `req` we'll need *after* the inner
                // match closes. This lets us run the post-action
                // (prefetch tracking + predict) without holding a borrow
                // on `self.fsm` that conflicts with later mutations.
                let req_addr = req.get_addr();
                let req_is_store = matches!(req, MemoryReqType::Store(_));
                // Snapshot of prefetch_in_flight at lookup start: if the
                // request we're servicing is itself a synthetic prefetch
                // we suppress demand-access HPM updates and skip
                // re-prediction (which would otherwise feed back on
                // ourselves).
                let is_prefetch = self.prefetch_in_flight;

                let (tag, index, offset) = self.addr_transfer(req_addr);
                let tag_compare_result = self.set[index].tag_compare(tag);
                let is_hit = tag_compare_result.is_ok();

                // check whether it is hit or miss
                match tag_compare_result {
                    // match arm 1: cache hit
                    Ok(way_index) => {
                        // read the whole data block at first
                        let mut read_block = self.set[index].read_block(way_index);
                        // If a store hit on a write-through policy, we
                        // need to *also* drive the same write to the
                        // next level — instead of going back to Idle we
                        // transition to WriteThroughCommit. Track that
                        // here so we can decide after the inner match.
                        let mut next_state: Option<MainStates> = None;

                        // handle read or write to the cache
                        match req {
                            MemoryReqType::Load(load_req) => {
                                load_req.buffer.borrow_mut().clone_from_slice(
                                    &read_block[offset..(offset + req.get_len())],
                                );
                                load_req.done.set(true);
                                if !is_prefetch {
                                    self.hpm.load(false); // update HPM
                                }
                            }
                            MemoryReqType::Store(store_req) => {
                                read_block[offset..(offset + req.get_len())]
                                    .clone_from_slice(&*store_req.store_data);
                                if !is_prefetch {
                                    self.hpm.store(false); // update HPM
                                }
                                if self.write_policy.is_write_through() {
                                    // Cache copy stays clean (matches
                                    // memory after the propagate
                                    // completes).
                                    self.set[index]
                                        .write_block_clean(way_index, &read_block);
                                    let propagate = MemoryStoreReq {
                                        addr: req.get_addr(),
                                        len: req.get_len(),
                                        store_data: store_req.store_data.clone(),
                                        done: Rc::new(Cell::new(false)),
                                    };
                                    let caller_done = store_req.done.clone();
                                    next_state = Some(MainStates::WriteThroughCommit(
                                        StatesForOutMemReq::SendReq(
                                            MemoryReqType::Store(propagate),
                                        ),
                                        caller_done,
                                    ));
                                } else {
                                    // Write-back: dirty the cache, ack
                                    // the requester immediately.
                                    self.set[index].write_block(way_index, &read_block);
                                    store_req.done.set(true);
                                }
                            }
                        }

                        // reset FSM (Idle for WB, propagate for WT)
                        self.fsm = next_state.unwrap_or(MainStates::Idle);
                    }

                    // match arms 2: cache miss
                    Err((need_write_back, evict_way)) => {
                        // update HPM
                        if !is_prefetch {
                            if req_is_store {
                                self.hpm.store(true);
                            } else {
                                self.hpm.load(true);
                            }
                        }

                        if req_is_store && self.write_policy.is_no_write_allocate() {
                            // No-write-allocate: a store miss bypasses
                            // the cache entirely. Send the store to the
                            // next level and skip allocation.
                            let store_req = req.get_store_req_ref();
                            let propagate = MemoryStoreReq {
                                addr: store_req.addr,
                                len: store_req.len,
                                store_data: store_req.store_data.clone(),
                                done: Rc::new(Cell::new(false)),
                            };
                            let caller_done = store_req.done.clone();
                            self.fsm = MainStates::WriteAround(
                                StatesForOutMemReq::SendReq(MemoryReqType::Store(propagate)),
                                caller_done,
                            );
                        } else {
                            // store current req to self.backup_req
                            self.backup_req = Some(req.clone());

                            // judge whether it needs to write-back
                            if need_write_back {
                                // get original tag of the old block (used in address of write-back request)
                                let old_tag = self.set[index].get_tag(evict_way);
                                let write_back_addr = ((old_tag
                                    << (self.index_bit_width + self.offset_bit_width)) // tag part
                                    + ((index as u32) << self.offset_bit_width)) // index part
                                    & !((1 << self.offset_bit_width) - 1); // bit mask
                                // get data of the old block which is dirty
                                let old_dirty_data = self.set[index].read_block(evict_way);

                                // construct MemoryReqType::Store
                                let write_back_store_req = MemoryStoreReq {
                                    addr: write_back_addr,
                                    len: old_dirty_data.len(),
                                    store_data: old_dirty_data,
                                    done: Rc::new(Cell::new(false)),
                                };

                                // transfer self.fsm to WriteBack
                                self.fsm = MainStates::WriteBack(
                                    StatesForOutMemReq::SendReq(MemoryReqType::Store(
                                        write_back_store_req,
                                    )),
                                    evict_way,
                                );
                            } else {
                                // calculate allocate address and length (in bytes)
                                let allocate_addr = ((tag
                                    << (self.index_bit_width + self.offset_bit_width))
                                    + (index << self.offset_bit_width) as u32)
                                    & !((1 << self.offset_bit_width) - 1);
                                let allocate_len = 2usize.pow(self.offset_bit_width as u32);

                                // construct MemoryReqType::Load
                                let allocate_read_req = MemoryLoadReq {
                                    addr: allocate_addr,
                                    len: allocate_len,
                                    buffer: Rc::new(RefCell::new(
                                        vec![0u8; allocate_len].into_boxed_slice(),
                                    )),
                                    done: Rc::new(Cell::new(false)),
                                };

                                // transfer self.fsm to Allocate
                                self.fsm = MainStates::Allocate(
                                    StatesForOutMemReq::SendReq(MemoryReqType::Load(
                                        allocate_read_req,
                                    )),
                                    evict_way,
                                );
                            }
                        }
                    }
                }

                // --- Post-resolution hooks for the prefetcher subsystem ---
                //
                // 1. If the request we just serviced was itself a
                //    synthetic prefetch and it HIT (either immediately,
                //    or on the re-entry that follows Allocate +
                //    MissPenalty), the prefetch is now resolved and the
                //    FSM is free to issue the next queued prefetch.
                // 2. Otherwise (this was a demand access) ask the
                //    prefetcher what it wants next, and queue each
                //    suggestion exactly once.
                if is_prefetch {
                    if is_hit {
                        self.prefetch_in_flight = false;
                    }
                } else {
                    let predictions = self.prefetcher.predict(req_addr, is_hit, req_is_store);
                    for addr in predictions {
                        if !self.prefetch_queue.contains(&addr) {
                            self.prefetch_queue.push_back(addr);
                        }
                    }
                }
            }

            // * write-back state -> handling write request to next-level memory via secondary FSM
            MainStates::WriteBack(ref mut second_state, ref evict_way) => {
                match second_state {
                    // try to send write request to next-level memory until it is accepted
                    StatesForOutMemReq::SendReq(ref req) => {
                        if self.mem_ref.borrow_mut().try_register_req(req).is_ok() {
                            *second_state = StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }

                    // wait for the write request to next-level memory to be completed
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        let store_req = req.get_store_req_ref();
                        if store_req.done.get() {
                            // prepare for allocate
                            let allocate_addr = self.backup_req.as_ref().unwrap().get_addr()
                                & !((1 << self.offset_bit_width) - 1);
                            let allocate_len = 2usize.pow(self.offset_bit_width as u32);
                            let allocate_req = MemoryLoadReq {
                                addr: allocate_addr,
                                len: allocate_len,
                                done: Rc::new(Cell::new(false)),
                                buffer: Rc::new(RefCell::new(
                                    vec![0u8; allocate_len].into_boxed_slice(),
                                )),
                            };
                            // transfer to allocate state, and reset self.second_state
                            self.fsm = MainStates::Allocate(
                                StatesForOutMemReq::SendReq(MemoryReqType::Load(allocate_req)),
                                *evict_way,
                            );
                        }
                    }
                }
            }

            // * allocate state: handling read request to next-level memory via secondary FSM
            MainStates::Allocate(ref mut second_state, ref evict_way) => {
                // println!("Cache Allocate...");
                match second_state {
                    StatesForOutMemReq::SendReq(ref req) => {
                        // try to send read request to next-level memory
                        if self.mem_ref.borrow_mut().try_register_req(req).is_ok() {
                            *second_state = StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        let load_req = req.get_load_req_ref();
                        if load_req.done.get() {
                            // get load data
                            let mut load_data: Vec<u8> = Vec::new();
                            for i in 0..load_req.len {
                                load_data.push(load_req.buffer.borrow()[i]);
                            }

                            // "insert" (not write) load data into cache set
                            let (new_tag, index, _) =
                                self.addr_transfer(self.backup_req.as_ref().unwrap().get_addr());
                            self.set[index].insert_block(*evict_way, new_tag, load_data.as_ref());

                            // transfer self.fsm to AdditionalMissPenalty state
                            self.fsm = MainStates::AdditionalMissPenalty(self.miss_penalty);
                        }
                    }
                }
            }

            // * Additional Miss Penalty State -> manipulate the countdown counter
            MainStates::AdditionalMissPenalty(ref mut counter) if *counter > 0 => {
                // println!("Cache Count...");
                *counter -= 1;
            }
            MainStates::AdditionalMissPenalty(ref counter) if *counter == 0 => {
                // println!("Cache Count...");
                self.fsm = MainStates::Lookup(self.backup_req.take().unwrap());
            }

            // * write-through commit: drive the propagated store to the
            //   next level of memory. When it completes, ack the caller.
            MainStates::WriteThroughCommit(ref mut second_state, ref caller_done) => {
                let mut completed = false;
                match second_state {
                    StatesForOutMemReq::SendReq(ref req) => {
                        if self.mem_ref.borrow_mut().try_register_req(req).is_ok() {
                            *second_state =
                                StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        if req.get_done() {
                            caller_done.set(true);
                            completed = true;
                        }
                    }
                }
                if completed {
                    self.fsm = MainStates::Idle;
                }
            }

            // * write-around: send the bypassing store to the next level
            //   and, when it completes, ack the caller.
            MainStates::WriteAround(ref mut second_state, ref caller_done) => {
                let mut completed = false;
                match second_state {
                    StatesForOutMemReq::SendReq(ref req) => {
                        if self.mem_ref.borrow_mut().try_register_req(req).is_ok() {
                            *second_state =
                                StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        if req.get_done() {
                            caller_done.set(true);
                            completed = true;
                        }
                    }
                }
                if completed {
                    self.fsm = MainStates::Idle;
                }
            }

            // * other situations -> do nothing
            _ => {}
        }
    }
}

impl<RP, M> Statistic for GeneralCache<RP, M>
where
    RP: ReplacementPolicy,
    M: AbstractMemoryInterface,
{
    type StatisticInfo = statistic::StatisticInfo;
    fn get_statistic_info(&self) -> Self::StatisticInfo {
        let mut ret = self.hpm.clone();
        ret.finalize_rates();
        ret
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::hardware::clock::Clocked;
    use crate::hardware::mem::general_cache::replacement_policy::fifo::FifoRP;
    use crate::hardware::mem::simple_mem::SimpleMem;

    use rand::Rng;

    fn initialize_system() -> (GeneralCache<FifoRP, SimpleMem>, Rc<RefCell<SimpleMem>>) {
        let mut rng = rand::rng();
        let mut random_init_data = vec![0u8; 0x10000];
        for i in 0..0x10000 {
            random_init_data[i] = rng.random_range(0..=255);
        }

        let mem = Rc::new(RefCell::new(SimpleMem::new(random_init_data)));
        let cache_config = GeneralCacheConfig::new("test_cache".to_string())
            .with_total_size(4096)
            .with_block_size(32)
            .with_num_of_way(2);
        let cache = GeneralCache::<FifoRP, SimpleMem>::new(cache_config, Rc::clone(&mem));
        (cache, mem)
    }

    #[test]
    fn allocate_without_write_back() {
        let (mut cache, mem) = initialize_system();

        let read_req_for_cache = MemoryReqType::Load(MemoryLoadReq {
            addr: 100,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        let read_req_for_mem = MemoryReqType::Load(MemoryLoadReq {
            addr: 100,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });

        if cache.try_register_req(&read_req_for_cache).is_err() {
            panic!();
        }
        if mem
            .borrow_mut()
            .try_register_req(&read_req_for_mem)
            .is_err()
        {
            panic!();
        };

        while !read_req_for_cache.get_load_req_ref().done.get()
            || !read_req_for_mem.get_load_req_ref().done.get()
        {
            mem.borrow_mut().tick();
            cache.tick();
        }

        for i in 0..4 {
            assert_eq!(
                read_req_for_cache.get_load_req_ref().buffer.borrow()[i],
                read_req_for_mem.get_load_req_ref().buffer.borrow()[i]
            );
        }
    }

    #[test]
    fn cause_write_back() {
        let (mut cache, mem) = initialize_system();
        let num_set = 32u32;
        let block_size = 32u32;

        // it needs at least 5 read access and 1 write access
        // to the cache to cause a write-backe operation
        // the cache has 32 sets, and each set has 4 ways

        // 4 consecutive read accesses to the cache
        for i in 0..4 {
            let read_req = MemoryReqType::Load(MemoryLoadReq {
                addr: 0 + block_size * num_set * i,
                len: 4,
                buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
                done: Rc::new(Cell::new(false)),
            });
            if cache.try_register_req(&read_req).is_err() {
                panic!();
            }
            while !read_req.get_load_req_ref().done.get() {
                cache.tick();
                mem.borrow_mut().tick();
            }
        }

        // 1 write access to make a dirty block in the cache
        let write_req = MemoryReqType::Store(MemoryStoreReq {
            addr: 0,
            len: 4,
            store_data: vec![116u8; 4].into_boxed_slice(),
            done: Rc::new(Cell::new(false)),
        });
        if cache.try_register_req(&write_req).is_err() {
            panic!();
        }
        while !write_req.get_store_req_ref().done.get() {
            cache.tick();
            mem.borrow_mut().tick();
        }
        std::mem::drop(write_req);

        // 1 read access to the different memory location
        // while the location is mapped to the same set in the cache
        // This read access causes to the write-back operation
        let read_req = MemoryReqType::Load(MemoryLoadReq {
            addr: 0 + block_size * num_set * 4,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        let mut ever_been_write_back_state = false;
        if cache.try_register_req(&read_req).is_err() {
            panic!();
        }
        while !read_req.get_load_req_ref().done.get() {
            cache.tick();
            mem.borrow_mut().tick();

            if let MainStates::WriteBack(_, _) = cache.fsm {
                ever_been_write_back_state = true;
            }
        }
        assert!(
            ever_been_write_back_state,
            "The cache never reaches the write-back state"
        );

        // read the data has been written back to mem and check the values
        let read_req = MemoryReqType::Load(MemoryLoadReq {
            addr: 0,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        if mem.borrow_mut().try_register_req(&read_req).is_err() {
            panic!();
        }
        while !read_req.get_load_req_ref().done.get() {
            mem.borrow_mut().tick();
        }
        for item in read_req.get_load_req_ref().buffer.borrow().iter() {
            assert_eq!(*item, 116);
        }
    }

    /// Build a cache with the given miss penalty and resolve one cold-miss
    /// load to completion. Return the number of ticks observed by the caller
    /// (i.e. the cycles spent between issuing the request and seeing `done`
    /// become true, *not counting* the synchronous-hit pre-ticks done inside
    /// `try_register_req`).
    fn cycles_to_resolve_cold_miss(miss_penalty_cycles: usize) -> usize {
        let mem = Rc::new(RefCell::new(SimpleMem::new(vec![0u8; 0x10000])));
        let cfg = GeneralCacheConfig::new("test".to_string())
            .with_total_size(4096)
            .with_block_size(32)
            .with_num_of_way(2)
            .with_miss_penalty(miss_penalty_cycles);
        let mut cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));

        let req = MemoryReqType::Load(MemoryLoadReq {
            addr: 0,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        cache.try_register_req(&req).expect("register req");

        let mut ticks = 0usize;
        while !req.get_load_req_ref().done.get() {
            cache.tick();
            mem.borrow_mut().tick();
            ticks += 1;
        }
        ticks
    }

    #[test]
    fn miss_penalty_is_per_cache_configurable() {
        // Same workload, only the configured miss penalty differs. A miss
        // that takes N cycles at penalty P should take N + delta cycles at
        // penalty P + delta — proving the FSM uses the per-cache value
        // rather than a global constant.
        let low = cycles_to_resolve_cold_miss(2);
        let high = cycles_to_resolve_cold_miss(20);
        assert!(
            high > low,
            "higher miss_penalty must take more cycles (low={}, high={})",
            low,
            high
        );
        assert_eq!(
            high - low,
            18,
            "cycle difference should equal the configured penalty delta"
        );
    }

    /// Drive a `cache` + `mem` pair until `done` is set on `req`. Used
    /// by write-policy tests.
    fn run_until_done<M: AbstractMemoryInterface>(
        cache: &mut GeneralCache<FifoRP, M>,
        mem: &Rc<RefCell<M>>,
        done: &Rc<Cell<bool>>,
    ) {
        for _ in 0..10_000 {
            if done.get() {
                return;
            }
            cache.tick();
            mem.borrow_mut().tick();
        }
        panic!("request never completed");
    }

    /// Build a cache configured with the given write policy, backed by a
    /// fresh zero-initialised SimpleMem.
    fn build_cache_with_policy(
        wp: write_policy::WritePolicy,
    ) -> (GeneralCache<FifoRP, SimpleMem>, Rc<RefCell<SimpleMem>>) {
        let mem = Rc::new(RefCell::new(SimpleMem::new(vec![0u8; 0x10000])));
        let cfg = GeneralCacheConfig::new("test".to_string())
            .with_total_size(4096)
            .with_block_size(32)
            .with_num_of_way(2)
            .with_miss_penalty(1)
            .with_write_policy(wp);
        let cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));
        (cache, mem)
    }

    #[test]
    fn write_through_propagates_store_hit_to_memory() {
        // WT-WA: first store misses → cache allocates → re-enters Lookup
        // with the original Store, which now hits and the WT path also
        // writes through to backing memory. Verify mem holds the store
        // data after the store completes.
        let (mut cache, mem) =
            build_cache_with_policy(write_policy::WritePolicy::WriteThroughWriteAllocate);

        let store = MemoryReqType::Store(MemoryStoreReq {
            addr: 0x40,
            len: 4,
            store_data: vec![0xAB, 0xCD, 0xEF, 0x12].into_boxed_slice(),
            done: Rc::new(Cell::new(false)),
        });
        cache.try_register_req(&store).expect("register store");
        run_until_done(&mut cache, &mem, &store.get_store_req_ref().done);

        // Read straight from memory and assert the bytes match.
        let probe = MemoryReqType::Load(MemoryLoadReq {
            addr: 0x40,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        mem.borrow_mut()
            .try_register_req(&probe)
            .expect("register mem probe");
        while !probe.get_load_req_ref().done.get() {
            mem.borrow_mut().tick();
        }
        let buf = probe.get_load_req_ref().buffer.borrow();
        assert_eq!(buf[0], 0xAB);
        assert_eq!(buf[1], 0xCD);
        assert_eq!(buf[2], 0xEF);
        assert_eq!(buf[3], 0x12);
    }

    #[test]
    fn no_write_allocate_bypasses_cache_on_store_miss() {
        // WB-NWA: a store miss should NOT install the block into the
        // cache; the next load to the same address should also miss.
        let (mut cache, mem) =
            build_cache_with_policy(write_policy::WritePolicy::WriteBackNoWriteAllocate);

        let store = MemoryReqType::Store(MemoryStoreReq {
            addr: 0x80,
            len: 4,
            store_data: vec![0x11, 0x22, 0x33, 0x44].into_boxed_slice(),
            done: Rc::new(Cell::new(false)),
        });
        cache.try_register_req(&store).expect("register store");
        run_until_done(&mut cache, &mem, &store.get_store_req_ref().done);

        let before_load = cache.hpm.load_miss_cnt;
        let load = MemoryReqType::Load(MemoryLoadReq {
            addr: 0x80,
            len: 4,
            buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
            done: Rc::new(Cell::new(false)),
        });
        cache.try_register_req(&load).expect("register load");
        run_until_done(&mut cache, &mem, &load.get_load_req_ref().done);
        // Same address ⇒ load should miss because the store did not
        // allocate; the load_miss counter must have advanced.
        assert!(
            cache.hpm.load_miss_cnt > before_load,
            "expected load miss after no-write-allocate store"
        );
        // And the loaded value must equal what the bypassed store wrote.
        let buf = load.get_load_req_ref().buffer.borrow();
        assert_eq!(buf[0], 0x11);
        assert_eq!(buf[1], 0x22);
        assert_eq!(buf[2], 0x33);
        assert_eq!(buf[3], 0x44);
    }

    /// Drive `n_blocks` sequential block-aligned loads through a cache
    /// configured with the given prefetcher kind, then drain any
    /// outstanding prefetches. Return `(load_miss_cnt, prefetch_issued_cnt)`.
    ///
    /// `miss_penalty` is set to 1 so the prefetch lifecycle (Lookup miss
    /// → Allocate → MissPenalty → Lookup hit) takes few enough ticks
    /// that the drain phase between accesses is bounded.
    fn run_sequential_pattern(
        kind: prefetcher::PrefetcherKind,
        n_blocks: u32,
        block_size: u32,
    ) -> (usize, usize) {
        let mem = Rc::new(RefCell::new(SimpleMem::new(vec![0u8; 0x10000])));
        let cfg = GeneralCacheConfig::new("test".to_string())
            .with_total_size(4096)
            .with_block_size(block_size as usize)
            .with_num_of_way(2)
            .with_miss_penalty(1)
            .with_prefetcher_kind(kind);
        let mut cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));

        for i in 0..n_blocks {
            let req = MemoryReqType::Load(MemoryLoadReq {
                addr: i * block_size,
                len: 4,
                buffer: Rc::new(RefCell::new(vec![0u8; 4].into_boxed_slice())),
                done: Rc::new(Cell::new(false)),
            });
            // Retry registration until the FSM is Idle. While a prefetch
            // from the previous demand access is in flight we'll get
            // Err — keep ticking so the prefetch can drain.
            while cache.try_register_req(&req).is_err() {
                cache.tick();
                mem.borrow_mut().tick();
            }
            run_until_done(&mut cache, &mem, &req.get_load_req_ref().done);

            // Give the just-queued prefetch a chance to complete before
            // the next demand access closes the window. Without this,
            // every prefetch sits in the queue forever because the next
            // try_register_req fills `pending_req` immediately on entry
            // to Idle, which beats the prefetch arm's guard.
            for _ in 0..200 {
                if !cache.prefetch_in_flight && cache.prefetch_queue.is_empty() {
                    break;
                }
                cache.tick();
                mem.borrow_mut().tick();
            }
        }

        (cache.hpm.load_miss_cnt, cache.hpm.prefetch_issued_cnt)
    }

    #[test]
    fn next_line_prefetcher_reduces_misses_on_sequential_pattern() {
        // 5 consecutive block-aligned loads. With no prefetcher we
        // expect one cold miss per block (5 misses). With the next-line
        // prefetcher each demand access triggers a fetch for the
        // following block, so only the first block should miss on
        // demand — subsequent demand accesses should be served from the
        // prefetched lines (prefetch misses are counted separately and
        // are not folded into `load_miss_cnt`).
        let (null_misses, null_prefetches) =
            run_sequential_pattern(prefetcher::PrefetcherKind::Null, 5, 32);
        let (nl_misses, nl_prefetches) =
            run_sequential_pattern(prefetcher::PrefetcherKind::NextLine, 5, 32);

        assert_eq!(
            null_misses, 5,
            "without prefetcher every cold block must miss on demand"
        );
        assert_eq!(
            null_prefetches, 0,
            "null prefetcher must never issue a synthetic fetch"
        );
        assert!(
            nl_prefetches > 0,
            "next-line prefetcher must issue at least one prefetch"
        );
        assert!(
            nl_misses < null_misses,
            "next-line prefetcher must reduce demand misses (null={}, next-line={}, prefetches_issued={})",
            null_misses, nl_misses, nl_prefetches
        );
    }

    // #[test]
    fn synchronous_access_check() {
        todo!();
    }

    // #[test]
    fn sequential_read_write() {
        todo!();
    }

    // #[test]
    fn random_read_write() {
        todo!();
    }
}
