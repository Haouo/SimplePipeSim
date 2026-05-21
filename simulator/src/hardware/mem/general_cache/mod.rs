// sub-modules
pub mod cache_set; // model for single cache set (might contains multiple ways)
pub mod replacement_policy; // model for cache replacement policy (e.g., Random, FIFO, LRU)
pub mod statistic; // utils of statistics for cache

use super::super::statistic::Statistic;
use crate::hardware::clock::Clocked;
use crate::hardware::mem::abstract_mem::*;
use cache_set::GeneralCacheSetUnit;
use replacement_policy::ReplacementPolicy;
use statistic::StatisticInfo;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

// constants
const CACHE_MISS_ADDITIONAL_PENALTY: usize = 5;

// a special struct data type for cache configuring
#[derive(Default, Clone)]
pub struct GeneralCacheConfig {
    name: String,
    total_size: Option<usize>,
    block_size: Option<usize>,
    num_of_way: Option<usize>,
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

    // statistic information
    pub hpm: StatisticInfo,
}

impl<RP: ReplacementPolicy, M: AbstractMemoryInterface> GeneralCache<RP, M> {
    // public methods
    /// only constructor for GeneralCache
    pub fn new(config: GeneralCacheConfig, mem_ref: Rc<RefCell<M>>) -> Self {
        let num_set =
            config.total_size.unwrap() / (config.block_size.unwrap() * config.num_of_way.unwrap());
        GeneralCache {
            offset_bit_width: config.block_size.unwrap().ilog2() as usize,
            index_bit_width: num_set.ilog2() as usize,
            set: (0..num_set)
                .map(|_| {
                    GeneralCacheSetUnit::<RP>::new(
                        config.num_of_way.unwrap(),
                        config.block_size.unwrap(),
                    )
                })
                .collect(),
            fsm: MainStates::Idle,
            mem_ref,
            pending_req: None,
            backup_req: None,
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
            // * Idle state with pending request
            MainStates::Idle if self.pending_req.is_some() => {
                // accept new request and transfer current state
                self.fsm = MainStates::Lookup(self.pending_req.take().unwrap());
            }

            // * Lookup state
            MainStates::Lookup(ref req) => {
                let (tag, index, offset) = self.addr_transfer(req.get_addr());
                let tag_compare_result = self.set[index].tag_compare(tag);

                // check whether it is hit or miss
                match tag_compare_result {
                    // match arm 1: cache hit
                    Ok(way_index) => {
                        // read the whole data block at first
                        let mut read_block = self.set[index].read_block(way_index);

                        // handle read or write to the cache
                        match req {
                            MemoryReqType::Load(load_req) => {
                                load_req.buffer.borrow_mut().clone_from_slice(
                                    &read_block[offset..(offset + req.get_len())],
                                );
                                load_req.done.set(true);
                                self.hpm.load(false); // update HPM
                            }
                            MemoryReqType::Store(store_req) => {
                                read_block[offset..(offset + req.get_len())]
                                    .clone_from_slice(&*store_req.store_data);
                                self.set[index].write_block(way_index, &read_block);
                                store_req.done.set(true);
                                self.hpm.store(false); // update HPM
                            }
                        }

                        // reset FSM, back to Idle state
                        self.fsm = MainStates::Idle;
                    }

                    // match arms 2: cache miss
                    Err((need_write_back, evict_way)) => {
                        // update HPM
                        match req {
                            MemoryReqType::Load(_) => {
                                self.hpm.load(true);
                            }
                            MemoryReqType::Store(_) => {
                                self.hpm.store(true);
                            }
                        }

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
                                StatesForOutMemReq::SendReq(MemoryReqType::Load(allocate_read_req)),
                                evict_way,
                            );
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
                            self.fsm =
                                MainStates::AdditionalMissPenalty(CACHE_MISS_ADDITIONAL_PENALTY);
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
