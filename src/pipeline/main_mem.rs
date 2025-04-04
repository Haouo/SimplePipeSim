use std::future::pending;

use crate::pipeline::abstract_mem::*;
use crate::pipeline::l1cache; // TODO
use crate::sim::elf::elf_loader;

const MEM_SIZE: usize = 0x100000;
const MAIN_MEM_LATENCY: usize = 50;

pub struct MemRequest {
    addr: u32,
    len: u32,
    access_type: AccessType,
    finish_callback: fn(),
    payload: Option<Vec<u8>>,
}

pub struct SimpleMem {
    /// the data structure used to store instructions and data
    data: [u8; MEM_SIZE],
    /// It indicates whether there is pending memory request from I$ or D$.
    ///
    /// If `pending_request != None`, it indicates that the main memory
    /// is currently busy for some early request
    pending_request: Option<MemRequest>,
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
        let mut elf_content = [0; MEM_SIZE];
        // TODO: load elf...

        SimpleMem {
            data: elf_content,
            pending_request: None,
            stall_countdown: MAIN_MEM_LATENCY,
            load_access_count: 0,
            store_access_count: 0,
        }
    }
    /// used by the master to check whether the main memory is busy for that moment
    pub fn check_busy(&self) -> bool {
        return self.pending_request.is_some();
    }
    /// unified access function of main memory
    pub fn access_register(&mut self, mem_request: &mut MemRequest, finish_callback: fn()) {
        //
    }
}

impl AbstraceMemInterface for SimpleMem {
    //
}
