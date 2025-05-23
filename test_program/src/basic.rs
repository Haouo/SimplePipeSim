use core::arch::asm;

unsafe fn __internal_syscall(reg_a0: u32, reg_a1: u32) {
    unsafe {
        asm!("ecall", in("a0") reg_a0, in("a1") reg_a1);
    }
}
