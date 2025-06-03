#![no_std]
#![no_main]

extern crate alloc;
use core::fmt::Write;

use runtime;
use runtime::basic_io::Stdout;

fn factorial(n: u32) -> u32 {
    if n == 0 || n == 1 {
        n
    } else {
        n * factorial(n - 1)
    }
}

#[no_mangle]
fn main() -> core::fmt::Result {
    writeln!(Stdout, "The result of factorial(20) is {}", factorial(20))?;
    Ok(())
}
