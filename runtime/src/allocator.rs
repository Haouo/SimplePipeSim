extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

// declare the sizes of stack and heap for linker script
extern "C" {
    static STACK_SIZE: usize;
    static HEAP_SIZE: usize;
}

const ARENA_SIZE: usize = 128 * 1024;
const MAX_SUPPORTED_ALIGN: usize = 4096;
#[repr(C, align(4096))] // 4096 == MAX_SUPPORTED_ALIGN
pub struct SimpleAllocator {
    arena: UnsafeCell<[u8; ARENA_SIZE]>,
    remaining: usize, // we allocate from the top, counting down
}

#[global_allocator]
pub static ALLOCATOR: SimpleAllocator = SimpleAllocator {
    arena: UnsafeCell::new([0x55; ARENA_SIZE]),
    remaining: ARENA_SIZE,
};

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!();
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        todo!();
    }
}

impl SimpleAllocator {
    pub unsafe fn init(&self) {
        //
    }
}
