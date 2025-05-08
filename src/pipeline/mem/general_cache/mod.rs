// declare sub-modules
pub mod cache_set;
pub mod replacement_policy;

// internal dep.
use crate::pipeline::clock::Clocked;
use crate::pipeline::mem::abstract_mem::*;
use crate::pipeline::mem::main_mem::SimpleMem;
use cache_set::GeneralCacheSetUnit;
use replacement_policy::ReplacementPolicy;

// external dep.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// constants
const CACHE_MISS_ADDITIONAL_PENALTY: usize = 10;

/// States for Main FSM to control the overall actions of the GeneralCache.
enum MainStates {
    Idle,
    Lookup(MemoryReqType), // with request from upper-level memory (i.e., access requester)
    WriteBack(StatesForOutMemReq, usize), // with write request to next-level memory
    Allocate(StatesForOutMemReq, usize), // with read request to next-level memory
    AdditionalMissPenalty(usize), // with countdown counter for additional miss penalty delay
}

/// States of secondary FSM to handling memory requests to next-level memory.
/// This is used when writing-back or allocating.
enum StatesForOutMemReq {
    SendReq(MemoryReqType),
    WaitForComplete(MemoryReqType),
}

pub struct GeneralCache<RP: ReplacementPolicy> {
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
    mem_ref: Rc<RefCell<SimpleMem>>,

    // pending access request for GeneralCache
    pending_req: Option<MemoryReqType>,
    // backup of the request being handled when the cache is writing-back or allocating
    backup_req: Option<MemoryReqType>,

    // data as hardware performance counter
    load_count: usize,
    load_miss_count: usize,
    store_count: usize,
    store_miss_count: usize,
}

impl<RP: ReplacementPolicy> GeneralCache<RP> {
    // public methods
    /// only constructor for GeneralCache
    pub fn new(
        total_size_bytes: usize,
        num_associativity: usize,
        bytes_per_block: usize,
        mem_ref: Rc<RefCell<SimpleMem>>,
    ) -> Self {
        let num_set = total_size_bytes / (bytes_per_block * num_associativity);
        GeneralCache {
            offset_bit_width: bytes_per_block.ilog2() as usize,
            index_bit_width: num_set.ilog2() as usize,
            set: {
                let mut temp_cache_set: Vec<GeneralCacheSetUnit<RP>> = vec![];
                for i in 0..num_set {
                    temp_cache_set.push(GeneralCacheSetUnit::<RP>::new(
                        num_associativity,
                        bytes_per_block,
                    ));
                }
                temp_cache_set.into_boxed_slice()
            },
            fsm: MainStates::Idle,
            mem_ref,
            pending_req: None,
            backup_req: None,
            load_count: 0,
            load_miss_count: 0,
            store_count: 0,
            store_miss_count: 0,
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

impl<RP: ReplacementPolicy> AbstraceMemInterface for GeneralCache<RP> {
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
        assert!(alignment_check, "Cache Access Alignment Checking Fail!");

        // try to register new request
        match &self.fsm {
            // can register new request if the current state is Idle
            // and self.pending_req is None
            MainStates::Idle if self.pending_req.is_none() => {
                self.pending_req = Some(req.clone());
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

impl<RP: ReplacementPolicy> Clocked for GeneralCache<RP> {
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
                                for i in 0..req.get_len() {
                                    load_req.buffer.borrow_mut()[i] = read_block[offset + i];
                                }
                                load_req.done.set(true);
                            }
                            MemoryReqType::Store(store_req) => {
                                for i in 0..req.get_len() {
                                    read_block[offset + i] = store_req.store_data[i];
                                }
                                self.set[index].write_block(way_index, read_block.as_ref());
                                store_req.done.set(true);
                            }
                        }

                        // reset FSM, back to Idle state
                        self.fsm = MainStates::Idle;
                    }

                    // match arms 2: cache miss
                    Err((need_write_back, evict_way)) => {
                        // record miss count
                        match req {
                            MemoryReqType::Load(_) => {
                                self.load_miss_count += 1;
                            }
                            MemoryReqType::Store(_) => {
                                self.store_miss_count += 1;
                            }
                        }

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
                        if let Ok(_) = self.mem_ref.borrow_mut().try_register_req(req) {
                            *second_state = StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }

                    // wait for the write request to next-level memory to be completed
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        let store_req = req.get_store_req();
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
                    _ => {}
                }
            }

            // * allocate state: handling read request to next-level memory via secondary FSM
            MainStates::Allocate(ref mut second_state, ref evict_way) => {
                match second_state {
                    StatesForOutMemReq::SendReq(ref req) => {
                        // try to send read request to next-level memory
                        if let Ok(_) = self.mem_ref.borrow_mut().try_register_req(req) {
                            *second_state = StatesForOutMemReq::WaitForComplete(req.clone());
                        }
                    }
                    StatesForOutMemReq::WaitForComplete(ref req) => {
                        let load_req = req.get_load_req();
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

                            // transfer to Lookup state
                            self.fsm = MainStates::Lookup(self.backup_req.take().unwrap());

                            // transfer self.fsm to AdditionalMissPenalty state
                            self.fsm =
                                MainStates::AdditionalMissPenalty(CACHE_MISS_ADDITIONAL_PENALTY);
                        }
                    }
                }
            }

            // * Additional Miss Penalty State -> manipulate the countdown counter
            MainStates::AdditionalMissPenalty(ref mut counter) if *counter > 0 => {
                *counter -= 1;
            }
            MainStates::AdditionalMissPenalty(ref counter) if *counter == 0 => {
                self.fsm = MainStates::Lookup(self.backup_req.take().unwrap());
            }

            // * other situations -> do nothing
            _ => {}
        }
    }
}

#[cfg(test)]
mod l1cache_unit_tests {
    use super::*;
    use crate::pipeline::clock::Clocked;
    use crate::pipeline::mem::general_cache::replacement_policy::ReplacementPolicy;
    use crate::pipeline::mem::main_mem::SimpleMem;

    // #[test]
    fn sequential_read_write() {
        todo!();
    }

    // #[test]
    fn random_read_write() {
        todo!();
    }
}
