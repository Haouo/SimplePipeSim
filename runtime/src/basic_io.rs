use core::arch::asm;
use core::fmt::Write;

/// only for internal function call
#[no_mangle]
extern "C" fn __internal_syscall(reg_a0: u32, reg_a1: u32) {
    unsafe {
        asm!("ecall","nop", "nop", "nop", "nop", in("a0") reg_a0, in("a1") reg_a1);
    }
}

macro_rules! syscall {
    ($a:expr) => {
        __internal_syscall($a, 0)
    };
    ($a:expr, $b:expr) => {
        __internal_syscall($a, $b)
    };
}

#[no_mangle]
fn platform_outb(single_char: char) {
    syscall!(1, single_char as u32);
}

#[no_mangle]
pub fn exit_success() -> ! {
    syscall!(0);
    unreachable!();
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
