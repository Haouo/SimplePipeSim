#![no_std]

extern crate alloc;

use core::alloc::GlobalAlloc;
use core::arch::asm;
use core::panic::PanicInfo;

use basic_io::syscall_1;

// declare the sizes of stack and heap for linker script
extern "C" {
    static STACK_SIZE: usize;
    static HEAP_SIZE: usize;
}

/// A custom and simple heap memory allocator
struct SimpleAllocator;
#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator {};
unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        todo!();
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        todo!();
    }
}

fn terminate() -> ! {
    // cleanup
    // @TODO

    // invoke ECALL
    syscall_1(0);

    // dummy loop
    unreachable!();
}

#[link_section = ".text._start"]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    // setup registers $gp and $sp
    asm!(
        ".option push",
        ".option norelax",
        "la gp, __global_pointer$",
        ".option pop",
        "la sp, stack_top",
    );

    // clean .bss section
    extern "C" {
        fn bss_start();
        fn bss_end();
        fn heap_start();
        fn heap_end();
    }

    // initialization of Heap allocator
    // @TODO

    extern "Rust" {
        fn main();
    }

    main();
    terminate();
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    // @TODO: print some info
    terminate();
}

// sub modules
pub mod basic_io;
