use crate::pipeline::mem::abstract_mem::*;
use crate::pipeline::clock::Clocked;

// external dep.
use std::rc::Weak;
use std::cell::RefCell;

const MEM_SIZE: usize = 0x100000 / 8;
const MAIN_MEM_LATENCY: usize = 50; // each memory access takes 50 cycles to finish
type MainMemoryGranularity = u64;

pub struct SimpleMem {
    /// the data structure used to store instructions and data
    data: [MainMemoryGranularity; MEM_SIZE],
    /// It indicates whether there is pending memory request from I$ or D$.
    ///
    /// If `pending_request != None`, it indicates that the main memory
    /// is currently busy for some early request
    pending_req: Option<MemoryReqType<MainMemoryGranularity>>,
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
    pub fn new(file: &String) -> Self {
        let mut elf_content :[u64; MEM_SIZE]= [0; MEM_SIZE];

        todo!(); // load elf contents

        // SimpleMem {
        //     data: elf_content,
        //     pending_request: None,
        //     stall_countdown: MAIN_MEM_LATENCY,
        //     load_access_count: 0,
        //     store_access_count: 0,
        // }
    }
}

impl AbstraceMemInterface for SimpleMem {
    type Granularity = MainMemoryGranularity;
    /// # try_register_req for SimpleMem
    ///
    /// The unified memory request try-register function for SimpleMem.
    /// The "try" term means that the register action might fail if the SimpleMem is busying.
    fn try_register_req(&mut self, req: MemoryReqType<Self::Granularity>) -> Result<(), ()> {
        // whether the memory is busy now
        if self.pending_req.is_some() {
            return Err(());
        }
        // the memory is idle, and it can accept new request from master
        match req {
            MemoryReqType::Load(_) => {
                self.load_access_count += 1;
            },
            MemoryReqType::Store(_) => {
                self.store_access_count += 1;
            }
        }
        self.pending_req = Some(req);
        self.stall_countdown = MAIN_MEM_LATENCY;
        Ok(())
    }
}

impl Clocked for SimpleMem {
    fn tick(&mut self) {
        if self.pending_req.is_some() {
            // must handle inflight request
            if self.stall_countdown == 0 {
                // ends the pending request
                let end_req = self.pending_req.take().unwrap();
                todo!();
                return;
            }
            self.stall_countdown -= 1;
        }
    }
}


#[cfg(test)]
mod simple_mem_unit_tests {
    use super::*;

    fn random_initialize() -> SimpleMem {
        todo!();
    }

    #[test]
    fn ramdom_access() {
        todo!();
    }
}
