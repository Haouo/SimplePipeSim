use crate::hardware::clock::Clocked;
use crate::hardware::mem::abstract_mem::*;

const MEM_SIZE: usize = 0x40000;
const MAIN_MEM_LATENCY: usize = 0; // each memory access takes 50 cycles to finish

pub struct SimpleMem {
    /// the data structure used to store instructions and data
    data: Box<[u8]>,
    /// It indicates whether there is pending memory request from I$ or D$.
    ///
    /// If `pending_request != None`, it indicates that the main memory
    /// is currently busy for some early request
    pending_req: Option<MemoryReqType>,
    /// used to imitate the busy stall for memory access
    stall_countdown: usize,
    /// performance counter for load
    load_access_count: usize,
    /// performance counter for store
    store_access_count: usize,
}

impl SimpleMem {
    /// Constructor of struct SimpleMem
    ///
    /// it will load the ELF firstly and
    /// initialize the SimpleMem.data with the ELF contents.
    pub fn new(init_data: Vec<u8>) -> Self {
        SimpleMem {
            data: init_data.into_boxed_slice(),
            pending_req: None,
            stall_countdown: MAIN_MEM_LATENCY,
            load_access_count: 0,
            store_access_count: 0,
        }
    }
}

impl AbstractMemoryInterface for SimpleMem {
    /// # try_register_req for SimpleMem
    ///
    /// The unified memory request try-register function for SimpleMem.
    /// The "try" term means that the register action might fail if the SimpleMem is busying.
    fn try_register_req(&mut self, req: &MemoryReqType) -> Result<(), ()> {
        // check whether the given address is legal
        assert!(
            req.get_addr() < (MEM_SIZE as u32),
            "Out-of-bound access is not allowed, the request addr. is {:#08X}",
            req.get_addr()
        );

        // whether the memory is busy now
        if self.pending_req.is_some() {
            return Err(());
        }
        // the memory is idle, and it can accept new request from master
        match req {
            MemoryReqType::Load(_) => {
                self.load_access_count += 1;
            }
            MemoryReqType::Store(_) => {
                self.store_access_count += 1;
            }
        }
        self.pending_req = Some(req.clone());
        self.stall_countdown = MAIN_MEM_LATENCY;
        Ok(())
    }
}

impl Clocked for SimpleMem {
    fn tick(&mut self) {
        if self.pending_req.is_some() {
            // manipulate stall counter
            if self.stall_countdown > 0 {
                self.stall_countdown -= 1;
                return;
            }

            // handle inflight request
            let req = self.pending_req.take().unwrap();
            match req {
                MemoryReqType::Load(load_req) => {
                    for i in 0..load_req.len {
                        load_req.buffer.borrow_mut()[i] = self.data[load_req.addr as usize + i];
                    }
                    load_req.done.set(true);
                }
                MemoryReqType::Store(store_req) => {
                    for i in 0..store_req.len {
                        self.data[store_req.addr as usize + i] = store_req.store_data[i];
                    }
                    store_req.done.set(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    use rand::Rng;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    fn random_initialize() -> (SimpleMem, Box<[u8]>) {
        let mut rng = rand::rng();
        let mut init_data = Vec::<u8>::with_capacity(MEM_SIZE);
        for i in 0..MEM_SIZE {
            init_data.push(rng.random_range(0..255) as u8);
        }

        (
            SimpleMem {
                data: init_data.clone().into_boxed_slice(),
                pending_req: None,
                stall_countdown: 0,
                load_access_count: 0,
                store_access_count: 0,
            },
            init_data.into(),
        )
    }

    // #[test]
    fn ramdom_access() {
        let (mut mem_dut, mut mem_golden) = random_initialize();
        let mut rng = rand::rng();

        for i in 0..10 {
            let access_length = rng.random_range(0..100) as usize;
            let access_start_addr = rng.random_range(0..(MEM_SIZE - access_length)) as u32;
            if rng.random_bool(0.5) {
                let mut random_store_data: Vec<u8> = vec![];
                for i in 0..access_length {
                    random_store_data.push(rng.random_range(0..256) as u8);
                }

                let store_req = MemoryReqType::Store(MemoryStoreReq {
                    addr: access_start_addr,
                    len: access_length,
                    store_data: random_store_data.clone().into_boxed_slice(),
                    done: Rc::new(Cell::new(false)),
                });
                if let Err(()) = mem_dut.try_register_req(&store_req) {
                    panic!();
                }
                for i in 0..access_length {
                    mem_golden[access_start_addr as usize + i] = random_store_data[i];
                }

                while !store_req.get_store_req_ref().done.get() {
                    mem_dut.tick();
                }

                // verify the store
                let load_req = MemoryReqType::Load(MemoryLoadReq {
                    addr: access_start_addr,
                    len: access_length,
                    buffer: Rc::new(RefCell::new(vec![0u8; access_length].into_boxed_slice())),
                    done: Rc::new(Cell::new(false)),
                });
                if let Err(()) = mem_dut.try_register_req(&load_req) {
                    panic!();
                }
                while !load_req.get_load_req_ref().done.get() {
                    mem_dut.tick();
                }
                for i in 0..access_length {
                    assert_eq!(
                        load_req.get_load_req_ref().buffer.borrow()[i],
                        random_store_data[i]
                    );
                }
            } else {
                // load
            }
        }
    }
}
