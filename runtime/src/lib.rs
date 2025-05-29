#![no_std]

extern crate alloc;

use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;

use basic_io::syscall_1;

pub fn terminate() -> ! {
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
    allocator::ALLOCATOR.init();

    extern "Rust" {
        fn main();
    }

    main();
    terminate();
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    writeln!(basic_io::Stdout, "Panicked!");
    terminate();
}

// sub modules
pub mod allocator;
pub mod basic_io;
