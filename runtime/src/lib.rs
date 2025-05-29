#![no_std]

extern crate alloc;

use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;

use basic_io::syscall_1;

// define the sizes of heap and stack
#[no_mangle]
pub static STACK_SIZE: usize = 0x2000;
#[no_mangle]
pub static HEAP_SIZE: usize = 0x4000;

#[no_mangle]
pub fn terminate() -> ! {
    // cleanup
    // @TODO (seems to be unnecessary)

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

    // initialize .bss section with zeros
    let bss_range = (bss_start as usize)..(bss_end as usize);
    bss_range.for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0);
    });

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
