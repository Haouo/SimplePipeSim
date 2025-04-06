use crate::pipeline::abstract_mem::*;
use crate::pipeline::clock::Clocked;

const MEM_SIZE: usize = 0x100000 / 8;
const MAIN_MEM_LATENCY: usize = 50; // each memory access takes 50 cycles to finish

pub struct MemRequest {
    addr: u32,
    len: u32,
    is_store: bool,
    store_data: Option<Vec<u64>>,
    /// ### callback function provided by memory access requester
    ///
    /// The function will be called automatically when the access request is done, and
    /// it returns the load memory data if it's load access; otherwise, it returns None
    finish_callback: MemFinishCallback<u64>,
}

pub struct SimpleMem {
    /// the data structure used to store instructions and data
    data: [u64; MEM_SIZE],
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

        todo!(); // load elf contents

        // SimpleMem {
        //     data: elf_content,
        //     pending_request: None,
        //     stall_countdown: MAIN_MEM_LATENCY,
        //     load_access_count: 0,
        //     store_access_count: 0,
        // }
    }
    /// used by the master to check whether the main memory is busy for that moment
    pub fn check_busy(&self) -> bool {
        return self.pending_request.is_some();
    }
    /// # Unified access function of main memory
    ///
    /// Note that the second parameter `mem_request: MemRequest` will takes the ownership
    /// because `struct MemRequest` does not implement `trait Copy`.
    pub fn access_register(&mut self, mem_request: MemRequest) {
        // register the memory access request
        self.pending_request = Some(mem_request);
        // reset stall counter
        self.stall_countdown = MAIN_MEM_LATENCY;
        // update performance counter
        if self.pending_request.as_ref().unwrap().is_store {
            self.store_access_count += 1;
        } else {
            self.load_access_count += 1;
        }
    }
}

impl Clocked for SimpleMem {
    fn tick(&mut self) {
        if self.pending_request.is_some() {
            // the request is finished
            // ready to call callback function provided by the requester
            if self.stall_countdown == 1 {
                let req = self.pending_request.as_ref().unwrap();
                // check whether the access exceeds the memory boundary
                if req.addr + req.len > MEM_SIZE as u32 {
                    (req.finish_callback)(None, Err(())); // tell the requester that the access is failed
                }
                // accept the memory request
                if req.is_store {
                    let mut_ref = &mut (self.data)[..req.len as usize];
                    for i in 0..(req.len as usize) {
                        mut_ref[i] = (req.store_data).as_ref().unwrap()[i];
                    }
                    (req.finish_callback)(None, Ok(()));
                } else {
                    (req.finish_callback)(
                        Some(
                            (&self.data)[req.addr as usize..(req.addr as usize + req.len as usize)]
                                .to_vec(),
                        ),
                        Ok(()),
                    );
                }
            }
            self.stall_countdown -= 1;
        }
    }
}
