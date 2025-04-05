use crate::pipeline::abstract_mem::AbstraceMemInterface;
use crate::pipeline::main_mem::SimpleMem;
use std::cell::RefCell;
use std::rc::Rc;

struct L1CacheTagUnit {
    tag: u32,
    valid: bool,
    dirty: bool,
}

/// the granularity of data section is 32-bits
struct L1CacheBlock {
    data: Box<[Box<[u32]>]>,
    tag: Box<[L1CacheTagUnit]>,
}

pub struct L1Cache {
    /// The main data struct to cache data.
    data: Box<[L1CacheBlock]>,
    /// The `Rc<_>` wrapping is for shared ownership
    /// because SimpleMem might be shared simultaneously by I$ and D$.
    /// The `RefCell<_>` wrapping is for mutability because cache might perform write-back
    mem_backdoor: Rc<RefCell<SimpleMem>>,
    is_busy: bool,
}

impl L1Cache {
    pub fn new(
        total_size: usize,
        block_size: usize,
        way_per_set: usize,
        mem_ref: Rc<RefCell<SimpleMem>>,
    ) -> Self {
        let set_num = total_size / (block_size * way_per_set);
        let word_per_block = block_size / 32;
        // bottom-up construction
        let mut sets: Vec<L1CacheBlock> = Vec::with_capacity(set_num);
        for _ in (0..set_num) {
            let data = (0..way_per_set)
                .map(|_| vec![0u32; word_per_block].into_boxed_slice())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let tag = (0..way_per_set)
                .map(|_| L1CacheTagUnit {
                    tag: 0,
                    valid: false,
                    dirty: false,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            sets.push(L1CacheBlock { data, tag });
        }
        L1Cache {
            data: sets.into_boxed_slice(),
            mem_backdoor: mem_ref,
            is_busy: false,
        }
    }
    fn insert(&self) {
        todo!();
    }
    fn writeback(&self) {
        todo!();
    }
    fn finish_callback(&mut self) {
        self.is_busy = false;
        // TODO: call the upper-level master's callback function to send notification
    }
}

impl AbstraceMemInterface<u32> for L1Cache {
    fn load(&self, addr: u32, len: u32) -> Result<Vec<u32>, &'static str> {
        todo!();
    }
    fn store(&mut self, addr: u32, len: u32, data: &[u32]) -> Result<(), &'static str> {
        todo!();
    }
    fn tick() {
        todo!();
    }
}
