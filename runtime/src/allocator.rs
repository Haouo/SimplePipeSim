extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
// use core::cell::UnsafeCell;

pub struct DummyAllocator;

impl DummyAllocator {
    // init() method for DummyAllocator
    pub fn init(&self) {}
}

unsafe impl GlobalAlloc for DummyAllocator {
    #[allow(unused)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }
    #[allow(unused)]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!("dealloc should never be called!");
    }
}

// declare global allocator
#[global_allocator]
pub static ALLOCATOR: DummyAllocator = DummyAllocator;
