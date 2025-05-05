use crate::pipeline::clock::Clocked;
use crate::pipeline::mem::abstract_mem::*;
use crate::pipeline::mem::main_mem::SimpleMem;
use crate::pipeline::mem::replacement_policy::ReplacementPolicy;

use std::cell::RefCell;
use std::rc::Rc;

const CACHE_MISS_ADDITIONAL_PENALTY: usize = 10;

#[derive(Default, Debug)]
struct L1CacheTagUnit {
    tag: u32,
    valid: bool,
    dirty: bool,
}

#[derive(Debug)]
struct L1CacheSetUnit<RP: ReplacementPolicy> {
    data_array: Box<[Box<[u32]>]>,
    tag_array: Box<[L1CacheTagUnit]>,
    rp: RP,
}

#[derive(Clone)]
enum L1CacheStates {
    Idle,
    Lookup(MemoryReqType<u32>),
    WaitForMem(MemoryReqType<u32>),
    ImitateDelay,
}

pub struct L1Cache<RP: ReplacementPolicy> {
    // configuration metadata
    offset_bit_width: usize,
    index_bit_width: usize,
    num_associativity: usize,

    // necessary data
    // The main data struct to cache data.
    data_tag: Box<[L1CacheSetUnit<RP>]>,
    // current state of L1Cache
    state: L1CacheStates,
    // The `Rc<_>` wrapping is for shared ownership
    // because SimpleMem might be shared simultaneously by I$ and D$.
    // The `RefCell<_>` wrapping is for mutability because cache might perform write-back
    mem_ref: Rc<RefCell<SimpleMem>>,

    // pending access request for L1Cache
    pending_req: Option<MemoryReqType<u32>>,
    // used for additional cache miss penalty clock cycles
    miss_penalty_countdown: usize,

    // data as hardware performance counter
    load_count: usize,
    load_miss_count: usize,
    store_count: usize,
    store_miss_count: usize,
}

impl<RP: ReplacementPolicy> L1Cache<RP> {
    // public methods
    /// only constructor for L1Cache
    pub fn new(
        total_size_bytes: usize,
        block_size_bytes: usize,
        num_associativity: usize,
        mem_ref: Rc<RefCell<SimpleMem>>,
    ) -> Self {
        let num_set = total_size_bytes / (block_size_bytes * num_associativity);
        let words_per_block = block_size_bytes / 4;

        let mut cache_sets: Vec<L1CacheSetUnit<RP>> = vec![];
        for i in 0..num_set {
            let data_array: Vec<Box<[u32]>> = (0..num_associativity)
                .map(|_| vec![0u32; words_per_block].into_boxed_slice())
                .collect();
            let tag_array: Vec<L1CacheTagUnit> = (0..num_associativity)
                .map(|_| L1CacheTagUnit {
                    ..Default::default()
                })
                .collect();
            cache_sets.push(L1CacheSetUnit {
                data_array: data_array.into_boxed_slice(),
                tag_array: tag_array.into_boxed_slice(),
                rp: RP::new(num_associativity),
            });
        }
        L1Cache {
            offset_bit_width: (words_per_block.ilog2() + 2) as usize,
            index_bit_width: num_set.ilog2() as usize,
            num_associativity,
            data_tag: cache_sets.into_boxed_slice(),
            state: L1CacheStates::Idle,
            mem_ref,
            pending_req: None,
            miss_penalty_countdown: 0,
            load_count: 0,
            load_miss_count: 0,
            store_count: 0,
            store_miss_count: 0,
        }
    }

    // private methods
    /// # lookup function for L1Cache
    ///
    /// For given address, this function will try to lookup the cache and return a boolean value to
    /// indicate whether it is cache-hit or cache-miss.
    fn lookup(&self, addr: u32) -> bool {
        // extract index and tag
        let index: usize =
            (addr as usize >> self.offset_bit_width) & ((1 << self.index_bit_width) - 1);
        let tag_bit_width: usize = 32 - self.index_bit_width - self.offset_bit_width;
        let tag: usize = (addr as usize >> tag_bit_width) & ((1 << tag_bit_width) - 1);

        // lookup each ways in the current cache set
        // check valid bit and tag matching
        for i in 0..self.num_associativity {
            if self.data_tag[index].tag_array[i].tag as usize == tag
                && self.data_tag[index].tag_array[i].valid
            {
                return true;
            }
        }
        false
    }

    /// load function
    fn load(&self, addr: u32) -> u32 {
        todo!();
    }

    /// store function
    fn store(&mut self, addr: u32, data: u32) {
        todo!();
    }

    /// util function for address to (tag, index, offset) tuple
    fn addr_transfer(&self, addr: u32) -> (usize, usize, usize) {
        todo!();
    }
}

impl<RP: ReplacementPolicy> AbstraceMemInterface for L1Cache<RP> {
    type Granularity = u32;
    fn try_register_req(
        &mut self,
        req: super::abstract_mem::MemoryReqType<Self::Granularity>,
    ) -> Result<(), ()> {
        match &self.state {
            // can register new request if the current state is Idle
            // and self.pending_req is None
            L1CacheStates::Idle if self.pending_req.is_none() => {
                self.pending_req = Some(req);
                match self.pending_req.as_ref().unwrap() {
                    MemoryReqType::Load(_) => {
                        self.load_count += 1;
                    }
                    MemoryReqType::Store(_) => {
                        self.store_count += 1;
                    }
                }
                return Ok(());
            }
            _ => {
                return Err(());
            }
        }
    }
}

impl<RP: ReplacementPolicy> Clocked for L1Cache<RP> {
    fn tick(&mut self) {
        let clone_state = self.state.clone(); // used to avoid lifetime issue
        match clone_state {
            L1CacheStates::Idle if self.pending_req.is_some() => {
                // accept new request and transfer current state
                self.state = L1CacheStates::Lookup(self.pending_req.take().unwrap());
            }
            L1CacheStates::Lookup(req) => {
                let addr = req.get_addr();
                let hit: bool = self.lookup(addr);
                if hit {
                    match &req {
                        MemoryReqType::Load(load_req) => {
                            assert_eq!(load_req.len, 1);
                            let load_data = self.load(load_req.addr);
                            load_req.buffer.upgrade().unwrap().borrow_mut()[0] = load_data;
                        }
                        MemoryReqType::Store(store_req) => {
                            assert_eq!(store_req.len, 1);
                            self.store(store_req.addr, store_req.store_data[0]);
                            store_req.done.upgrade().unwrap().set(true);
                        }
                    }
                } else {
                    // cache miss
                    let (tag, index, offset) = self.addr_transfer(req.get_addr());
                    let (full, way_to_evict) = self.data_tag[index].rp.evict();
                    if !full {
                        // current set has empty slot (way) to allocate new data
                        // request for new data from next level memory directly
                    } else {
                        // current set has not empty slot (way) to allocate new data
                        // then check whether it has to write back dirty data
                        if self.data_tag[index].tag_array[way_to_evict].dirty {
                            // need to write back dirty slot
                        } else {
                            // request for new data from next level memory directly
                        }
                    }
                    todo!();
                }
            }
            L1CacheStates::WaitForMem(req) => {
                todo!();
            }
            L1CacheStates::ImitateDelay => {
                todo!();
            }
            _ => {}
        }
        // otherwise, the cache is idle, nothing to do here
    }
}

#[cfg(test)]
mod l1cache_unit_tests {
    use super::*;
    use crate::pipeline::clock::Clocked;
    use crate::pipeline::mem::main_mem::SimpleMem;
    use crate::pipeline::mem::replacement_policy::fifo::FifoRP;

    #[test]
    fn sequential_read_write() {
        todo!();
    }

    #[test]
    fn random_read_write() {
        todo!();
    }
}
