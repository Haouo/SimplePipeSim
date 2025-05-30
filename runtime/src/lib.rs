#![no_std]

extern crate alloc;

use core::arch::global_asm;
use core::fmt::Write;
use core::panic::PanicInfo;

use basic_io::{exit_success, Stdout};

// define the sizes of heap and stack
#[no_mangle]
pub static STACK_SIZE: usize = 0x4000; // 16-KiB
#[no_mangle]
pub static HEAP_SIZE: usize = 0x4000; // 16-KiB

#[no_mangle]
pub fn terminate() -> ! {
    // cleanup
    // @TODO (seems to be unnecessary)

    exit_success();
}

global_asm!(
    ".section .text._start",
    ".global _start",
    "_start:",
    ".option push",
    ".option norelax",
    "la gp, __global_pointer$",
    ".option pop",
    "la sp, stack_top",
    "tail start",
);

#[no_mangle]
pub unsafe extern "C" fn start() -> ! {
    // clean .bss section
    extern "C" {
        fn bss_start();
        fn bss_end();
        fn heap_start();
        fn heap_end();
    }

    // initialize .bss section with zeros
    let bss_range = (bss_start as usize)..(bss_end as usize);
    bss_range.for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0);
    });

    // initialization of Heap allocator
    allocator::ALLOCATOR.init(heap_start as u32, heap_end as u32);

    extern "Rust" {
        fn main();
    }

    main();
    terminate();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let _ = writeln!(Stdout, "Panicked!");
    terminate();
}

// sub modules
pub mod allocator;
pub mod basic_io;
