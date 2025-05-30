extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
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
// #[global_allocator]
// pub static ALLOCATOR: DummyAllocator = DummyAllocator;

// another simple allocator: Bump allocator
pub struct BumpAllocator {
    heap_start: Cell<u32>,
    heap_end: Cell<u32>,
    next: Cell<u32>,
    num_alloc: Cell<usize>,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: Cell::new(0),
            heap_end: Cell::new(0),
            next: Cell::new(0),
            num_alloc: Cell::new(0),
        }
    }

    pub fn init(&self, heap_start: u32, heap_end: u32) {
        self.heap_start.set(heap_start);
        self.heap_end.set(heap_end);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    #[allow(unused)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        todo!();
    }

    #[allow(unused)]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.num_alloc.set(self.num_alloc.get() - 1);
        // deallocate all allocated memory at one times
        if self.num_alloc.get() == 0 {
            self.next.set(self.heap_start.get());
        }
    }
}

unsafe impl Sync for BumpAllocator {}

#[global_allocator]
pub static ALLOCATOR: BumpAllocator = BumpAllocator::new();
