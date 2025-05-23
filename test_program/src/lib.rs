#![no_std]
#![no_main]

extern crate alloc;
use core::alloc::GlobalAlloc;
use core::arch::asm;
use core::panic::PanicInfo;

struct SimpleAllocator;
#[global_allocator]
static allocator: SimpleAllocator = SimpleAllocator {};
unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        todo!();
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        todo!();
    }
}

pub mod basic;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
fn _start() -> ! {
    // unsafe {
    //     asm!("call main");
    // }

    // cleanup and terminate
    unsafe {
        asm!("ecall");
    }

    // dummp loop
    loop {}
}
