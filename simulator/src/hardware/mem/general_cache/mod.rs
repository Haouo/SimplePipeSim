// sub-modules
pub mod cache_set; // model for single cache set (might contains multiple ways)
pub mod config; // validated geometry and cache configuration
mod lower_memory; // next-level memory request transaction
pub mod prefetcher; // model for cache prefetcher (Null, NextLine, ...)
pub mod replacement_policy; // model for cache replacement policy (e.g., Random, FIFO, LRU)
pub mod statistic; // utils of statistics for cache
pub mod write_policy; // model for cache write policy (WB/WT × WA/NWA)

use super::super::statistic::Statistic;
use crate::hardware::clock::Clocked;
use crate::hardware::mem::abstract_mem::*;
use cache_set::GeneralCacheSetUnit;
pub use config::{GeneralCacheConfig, GeneralCacheConfigError};
use lower_memory::LowerMemoryTransaction;
use prefetcher::Prefetcher;
use replacement_policy::ReplacementPolicy;
use statistic::StatisticInfo;
use write_policy::WritePolicy;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

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
///                        ┌──────────────────────────────────────────────┐
///                        │  (hit / WT commit done / WA done / prefetch   │
///                        │   hit) — return to Idle                       │
///                        ▼                                              │
///   ┌──────┐ pending_req=Some                                           │
///   │ Idle │──────────────────────┐                                     │
///   │      │ prefetch_queue !=∅   │                                     │
///   │      │ && !in_flight        │                                     │
///   │      │ && pending_req=None  │                                     │
///   └──────┘──────────────────────┤                                     │
///       ▲                         ▼                                     │
///       │                  ┌──────────────┐                             │
///       │                  │   Lookup     │  tag compare                │
///       │                  │ (demand req  │                             │
///       │                  │  or synthetic│                             │
///       │                  │  prefetch)   │                             │
///       │                  └──┬────────┬──┘                             │
///       │             HIT     │        │ MISS                           │
///       │       ┌─────────────┘        └──────────────┐                 │
///       │       ▼                                     ▼                 │
///       │ ┌───────────────────────┐         ┌──────────────────────┐    │
///       │ │ Load: fill buffer     │         │ Store + NWA?         │    │
///       │ │ Store(WB): dirty+done │         │  ── yes ──┐          │    │
///       │ │ Store(WT): clean,     │         │           ▼          │    │
///       │ │            propagate  │         │   ┌─────────────┐    │    │
///       │ └────────┬──────────────┘         │   │ WriteAround │    │    │
///       │          │ Store(WT)              │   │ (tx, req)   │    │    │
///       │          ▼                        │   └──────┬──────┘    │    │
///       │ ┌───────────────────────┐         │          │ tx done   │    │
///       │ │ WriteThroughCommit    │         │          │ + ack     │    │
///       │ │ (tx, caller_req)      │         │          └──────────────────┐
///       │ └────────┬──────────────┘         │  ── no ── select victim    │
///       │          │ tx done + ack          │       │                    │
///       │          └─────────────────────────────────│                    │
///       │                                            │                    │
///       │                              dirty victim  │  clean victim      │
///       │                                     ┌──────┴──────┐             │
///       │                                     ▼             ▼             │
///       │                              ┌────────────┐       │             │
///       │                              │ WriteBack  │       │             │
///       │                              │ (tx, way)  │       │             │
///       │                              └─────┬──────┘       │             │
///       │                                    │ tx done      │             │
///       │                                    ▼              ▼             │
///       │                              ┌────────────────────────┐         │
///       │                              │ Allocate (tx, way)     │         │
///       │                              └──────────┬─────────────┘         │
///       │                                         │ tx done,              │
///       │                                         │ insert_block          │
///       │                                         ▼                       │
///       │                              ┌────────────────────────┐         │
///       │                              │ AdditionalMissPenalty  │         │
///       │                              │ (countdown)            │         │
///       │                              └──────────┬─────────────┘         │
///       │                                         │ counter == 0,         │
///       │                                         │ re-Lookup(backup_req) │
///       └─────────────────────────────────────────┘                       │
///                                                                         │
///       (all return paths feed back into Idle) ────────────────────────────┘
/// ```
///
/// State payloads
/// --------------
/// - `WriteBack(tx, way)` / `Allocate(tx, way)` carry a lower-memory
///   transaction plus the way index being evicted/filled.
/// - `WriteThroughCommit(tx, caller_req)` carries the propagated store
///   transaction and the original requester to ack on completion.
/// - `WriteAround(tx, caller_req)` is the no-write-allocate counterpart
///   for store misses: the cache is left untouched and the store goes
///   straight to the next level.
/// - `AdditionalMissPenalty(n)` counts down `miss_penalty` cycles before
///   re-entering `Lookup` with `backup_req` to finish the original
///   demand/prefetch request.
///
/// Prefetcher interaction
/// ----------------------
/// The prefetch arm of `Idle` is listed first in `tick()`, but its guard
/// requires `pending_req.is_none()` — so demand requests always win.
/// While a synthetic prefetch is in flight (`prefetch_in_flight = true`)
/// the `Lookup` arm suppresses demand-access HPM updates and skips
/// re-prediction. The flag is cleared when the prefetch finally hits in
/// `Lookup` (either immediately or after the Allocate + MissPenalty
/// loop).
enum MainStates {
    /// No request in flight, no pending request to start.
    Idle,
    /// Tag-compare against the indexed set. On hit, complete in this tick;
    /// on miss, transition to WriteBack (dirty victim) or Allocate (clean).
    Lookup(MemoryReqType),
    /// Sending or waiting for a write-back of a dirty victim to the next
    /// level of memory. `usize` is the evicted way index.
    WriteBack(LowerMemoryTransaction, usize),
    /// Sending or waiting for a refill from the next level of memory.
    /// `usize` is the way index the refill will be inserted into.
    Allocate(LowerMemoryTransaction, usize),
    /// Stall for an additional fixed penalty after a miss completes,
    /// modelling tag-array re-access / pipeline bubble cost.
    AdditionalMissPenalty(usize),
    /// Used by write-through policies after a store hit: the cache copy
    /// has already been updated; this state drives the same store to the
    /// next-level memory. When the next-level store completes we mark
    /// the original request complete and return to Idle.
    WriteThroughCommit(LowerMemoryTransaction, MemoryReqType),
    /// Used by no-write-allocate policies on a store miss: the cache
    /// stays untouched and the store goes straight to the next level.
    /// On completion we mark the original request complete and return
    /// to Idle.
    WriteAround(LowerMemoryTransaction, MemoryReqType),
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
        let block_size = config.geometry.block_size;
        let prefetcher = config.prefetcher_kind.build(block_size);
        GeneralCache {
            offset_bit_width: config.geometry.offset_bit_width,
            index_bit_width: config.geometry.index_bit_width,
            set: (0..config.geometry.num_sets)
                .map(|_| GeneralCacheSetUnit::<RP>::new(config.geometry.num_of_way, block_size))
                .collect(),
            fsm: MainStates::Idle,
            mem_ref,
            pending_req: None,
            backup_req: None,
            miss_penalty: config.miss_penalty,
            write_policy: config.write_policy,
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

impl<RP: ReplacementPolicy, M: AbstractMemoryInterface> AbstractMemoryInterface
    for GeneralCache<RP, M>
{
    fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()> {
        // CPU requests are byte/halfword/word accesses. Refills and
        // write-backs from an upper cache move an aligned region that fits
        // within one lower-level cache block.
        let req_len = req.get_len();
        let req_addr = req.get_addr() as usize;
        let alignment_check = match req_len {
            1 => true,
            2 | 4 => req_addr % req_len == 0,
            _ => {
                let offset = req_addr % self.block_size_bytes;
                req_len > 0
                    && req_len <= self.block_size_bytes
                    && req_len.is_power_of_two()
                    && req_addr % req_len == 0
                    && offset + req_len <= self.block_size_bytes
            }
        };
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
                // as the call to `try_register_req`, i.e. `req.is_done() == true`
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
                let synthetic = MemoryReqType::load(addr, len);
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
                                load_req.complete_from_slice(
                                    &read_block[offset..(offset + req.get_len())],
                                );
                                if !is_prefetch {
                                    self.hpm.load(false); // update HPM
                                }
                            }
                            MemoryReqType::Store(store_req) => {
                                read_block[offset..(offset + req.get_len())]
                                    .clone_from_slice(store_req.data());
                                if !is_prefetch {
                                    self.hpm.store(false); // update HPM
                                }
                                if self.write_policy.is_write_through() {
                                    // Cache copy stays clean (matches
                                    // memory after the propagate
                                    // completes).
                                    self.set[index].write_block_clean(way_index, &read_block);
                                    let propagate =
                                        MemoryReqType::store(req.get_addr(), store_req.data());
                                    next_state = Some(MainStates::WriteThroughCommit(
                                        LowerMemoryTransaction::new(propagate),
                                        req.clone(),
                                    ));
                                } else {
                                    // Write-back: dirty the cache, ack
                                    // the requester immediately.
                                    self.set[index].write_block(way_index, &read_block);
                                    store_req.complete();
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
                            let propagate = MemoryReqType::store(req.get_addr(), req.store_data());
                            self.fsm = MainStates::WriteAround(
                                LowerMemoryTransaction::new(propagate),
                                req.clone(),
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
                                let write_back_store_req =
                                    MemoryReqType::store(write_back_addr, old_dirty_data);

                                // transfer self.fsm to WriteBack
                                self.fsm = MainStates::WriteBack(
                                    LowerMemoryTransaction::new(write_back_store_req),
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
                                let allocate_read_req =
                                    MemoryReqType::load(allocate_addr, allocate_len);

                                // transfer self.fsm to Allocate
                                self.fsm = MainStates::Allocate(
                                    LowerMemoryTransaction::new(allocate_read_req),
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

            // * write-back state -> handling write request to next-level memory
            MainStates::WriteBack(ref mut transaction, ref evict_way) => {
                if transaction.progress(&self.mem_ref) {
                    // prepare for allocate
                    let allocate_addr = self.backup_req.as_ref().unwrap().get_addr()
                        & !((1 << self.offset_bit_width) - 1);
                    let allocate_len = 2usize.pow(self.offset_bit_width as u32);
                    let allocate_req = MemoryReqType::load(allocate_addr, allocate_len);
                    self.fsm =
                        MainStates::Allocate(LowerMemoryTransaction::new(allocate_req), *evict_way);
                }
            }

            // * allocate state: handling read request to next-level memory
            MainStates::Allocate(ref mut transaction, ref evict_way) => {
                if transaction.progress(&self.mem_ref) {
                    let load_data = transaction.load_data();

                    // "insert" (not write) load data into cache set
                    let (new_tag, index, _) =
                        self.addr_transfer(self.backup_req.as_ref().unwrap().get_addr());
                    self.set[index].insert_block(*evict_way, new_tag, load_data.as_ref());

                    // transfer self.fsm to AdditionalMissPenalty state
                    self.fsm = MainStates::AdditionalMissPenalty(self.miss_penalty);
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
            MainStates::WriteThroughCommit(ref mut transaction, ref caller_req) => {
                if transaction.progress(&self.mem_ref) {
                    caller_req.complete_store();
                    self.fsm = MainStates::Idle;
                }
            }

            // * write-around: send the bypassing store to the next level
            //   and, when it completes, ack the caller.
            MainStates::WriteAround(ref mut transaction, ref caller_req) => {
                if transaction.progress(&self.mem_ref) {
                    caller_req.complete_store();
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
        let cache_config =
            GeneralCacheConfig::new("test_cache", 4096, 32, 2).expect("valid cache config");
        let cache = GeneralCache::<FifoRP, SimpleMem>::new(cache_config, Rc::clone(&mem));
        (cache, mem)
    }

    #[test]
    fn allocate_without_write_back() {
        let (mut cache, mem) = initialize_system();

        let read_req_for_cache = MemoryReqType::load(100, 4);
        let read_req_for_mem = MemoryReqType::load(100, 4);

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

        while !read_req_for_cache.is_done() || !read_req_for_mem.is_done() {
            mem.borrow_mut().tick();
            cache.tick();
        }

        let cache_data = read_req_for_cache.load_data();
        let mem_data = read_req_for_mem.load_data();
        for i in 0..4 {
            assert_eq!(cache_data[i], mem_data[i]);
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
            let read_req = MemoryReqType::load(block_size * num_set * i, 4);
            if cache.try_register_req(&read_req).is_err() {
                panic!();
            }
            while !read_req.is_done() {
                cache.tick();
                mem.borrow_mut().tick();
            }
        }

        // 1 write access to make a dirty block in the cache
        let write_req = MemoryReqType::store(0, vec![116u8; 4]);
        if cache.try_register_req(&write_req).is_err() {
            panic!();
        }
        while !write_req.is_done() {
            cache.tick();
            mem.borrow_mut().tick();
        }
        std::mem::drop(write_req);

        // 1 read access to the different memory location
        // while the location is mapped to the same set in the cache
        // This read access causes to the write-back operation
        let read_req = MemoryReqType::load(block_size * num_set * 4, 4);
        let mut ever_been_write_back_state = false;
        if cache.try_register_req(&read_req).is_err() {
            panic!();
        }
        while !read_req.is_done() {
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
        let read_req = MemoryReqType::load(0, 4);
        if mem.borrow_mut().try_register_req(&read_req).is_err() {
            panic!();
        }
        while !read_req.is_done() {
            mem.borrow_mut().tick();
        }
        for item in read_req.load_data().iter() {
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
        let cfg = GeneralCacheConfig::new("test", 4096, 32, 2)
            .expect("valid cache config")
            .with_miss_penalty(miss_penalty_cycles);
        let mut cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));

        let req = MemoryReqType::load(0, 4);
        cache.try_register_req(&req).expect("register req");

        let mut ticks = 0usize;
        while !req.is_done() {
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
        req: &MemoryReqType,
    ) {
        for _ in 0..10_000 {
            if req.is_done() {
                return;
            }
            cache.tick();
            mem.borrow_mut().tick();
        }
        panic!("request never completed");
    }

    #[test]
    fn accepts_aligned_upper_cache_block_transfer() {
        let mem = Rc::new(RefCell::new(SimpleMem::new(vec![0u8; 0x10000])));
        let cfg = GeneralCacheConfig::new("l2", 16384, 64, 4)
            .expect("valid cache config")
            .with_miss_penalty(1);
        let mut cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));
        let upper_cache_fill = MemoryReqType::load(0, 32);

        cache
            .try_register_req(&upper_cache_fill)
            .expect("register upper cache block transfer");
        run_until_done(&mut cache, &mem, &upper_cache_fill);
    }

    /// Build a cache configured with the given write policy, backed by a
    /// fresh zero-initialised SimpleMem.
    fn build_cache_with_policy(
        wp: write_policy::WritePolicy,
    ) -> (GeneralCache<FifoRP, SimpleMem>, Rc<RefCell<SimpleMem>>) {
        let mem = Rc::new(RefCell::new(SimpleMem::new(vec![0u8; 0x10000])));
        let cfg = GeneralCacheConfig::new("test", 4096, 32, 2)
            .expect("valid cache config")
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

        let store = MemoryReqType::store(0x40, vec![0xAB, 0xCD, 0xEF, 0x12]);
        cache.try_register_req(&store).expect("register store");
        run_until_done(&mut cache, &mem, &store);

        // Read straight from memory and assert the bytes match.
        let probe = MemoryReqType::load(0x40, 4);
        mem.borrow_mut()
            .try_register_req(&probe)
            .expect("register mem probe");
        while !probe.is_done() {
            mem.borrow_mut().tick();
        }
        let buf = probe.load_data();
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

        let store = MemoryReqType::store(0x80, vec![0x11, 0x22, 0x33, 0x44]);
        cache.try_register_req(&store).expect("register store");
        run_until_done(&mut cache, &mem, &store);

        let before_load = cache.hpm.load_miss_cnt;
        let load = MemoryReqType::load(0x80, 4);
        cache.try_register_req(&load).expect("register load");
        run_until_done(&mut cache, &mem, &load);
        // Same address ⇒ load should miss because the store did not
        // allocate; the load_miss counter must have advanced.
        assert!(
            cache.hpm.load_miss_cnt > before_load,
            "expected load miss after no-write-allocate store"
        );
        // And the loaded value must equal what the bypassed store wrote.
        let buf = load.load_data();
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
        let cfg = GeneralCacheConfig::new("test", 4096, block_size as usize, 2)
            .expect("valid cache config")
            .with_miss_penalty(1)
            .with_prefetcher_kind(kind);
        let mut cache = GeneralCache::<FifoRP, SimpleMem>::new(cfg, Rc::clone(&mem));

        for i in 0..n_blocks {
            let req = MemoryReqType::load(i * block_size, 4);
            // Retry registration until the FSM is Idle. While a prefetch
            // from the previous demand access is in flight we'll get
            // Err — keep ticking so the prefetch can drain.
            while cache.try_register_req(&req).is_err() {
                cache.tick();
                mem.borrow_mut().tick();
            }
            run_until_done(&mut cache, &mem, &req);

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
