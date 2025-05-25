use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

/// only for internal function call
#[inline(always)]
fn __internal_syscall(reg_a0: u32, reg_a1: u32) {
    unsafe {
        asm!("ecall", in("a0") reg_a0, in("a1") reg_a1);
    }
}

#[inline(always)]
pub fn syscall_1(reg_a0: u32) {
    __internal_syscall(reg_a0, 0);
}

#[inline(always)]
pub fn syscall_2(reg_a0: u32, reg_a1: u32) {
    __internal_syscall(reg_a0, reg_a1);
}

#[inline(always)]
pub fn platform_outb(addr: u32, single_char: i8) {
    // it is mapped to sw instruction in RISC-V
    unsafe {
        write_volatile(addr as *mut i8, single_char);
    }
}
