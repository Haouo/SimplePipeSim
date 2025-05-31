extern crate alloc;

pub mod dummp_allocator {
    use core::alloc::{GlobalAlloc, Layout};
    use core::ptr::null_mut;

    pub struct Allocator;

    impl Allocator {
        pub fn init(&self) {}
    }

    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
            null_mut()
        }
        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
            unimplemented!("dealloc should never be called!");
        }
    }
}

pub mod bump_allocator {
    use core::alloc::{GlobalAlloc, Layout};
    use core::cell::Cell;
    use core::ptr::null_mut;

    pub struct Allocator {
        heap_start: Cell<u32>,
        heap_end: Cell<u32>,
        next: Cell<u32>,
        num_alloc: Cell<usize>,
    }

    impl Allocator {
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
            self.next.set(heap_start);
            self.heap_end.set(heap_end);
        }
    }

    unsafe impl GlobalAlloc for Allocator {
        #[allow(unused)]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let alloc_start = if self.next.get() % layout.align() as u32 == 0 {
                // already aligned
                self.next.get()
            } else {
                // need to align
                self.next.get() - (self.next.get() % layout.align() as u32)
                    + (layout.align() as u32)
            };
            // integer addition overflow check
            let alloc_end = match alloc_start.checked_add(layout.size() as u32) {
                Some(end) => end,
                None => return null_mut(),
            };
            // boundary check
            if alloc_end > self.heap_end.get() {
                null_mut()
            } else {
                // allocate new heap memory
                self.num_alloc.set(self.num_alloc.get() + 1);
                self.next.set(alloc_end);
                alloc_start as *mut u8
            }
        }

        #[allow(unused)]
        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
            if self.num_alloc.get() == 0 {
                panic!("Deallocation Error! There is no any allocated memory on the heap.");
            }

            // deallocate all allocated memory at one times
            self.num_alloc.set(self.num_alloc.get() - 1);
            if self.num_alloc.get() == 0 {
                self.next.set(self.heap_start.get());
            }
        }
    }

    // this Sync Trait Implementation is only for compilation limit
    // It does not have actual effect.
    unsafe impl Sync for Allocator {}
}

// define global allocator
#[global_allocator]
pub static ALLOCATOR: bump_allocator::Allocator = bump_allocator::Allocator::new();
