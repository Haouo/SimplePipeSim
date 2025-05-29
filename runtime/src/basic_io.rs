use core::arch::asm;
use core::fmt::Write;

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
fn platform_outb(single_char: char) {
    syscall_2(1, single_char as u32);
}

/// Unit struct for implementing text display
pub struct Stdout;
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            platform_outb(c);
        }
        Ok(())
    }
}
