#![no_std]
#![no_main]

use core::fmt::Write;
use runtime;

#[no_mangle]
fn main() -> ! {
    write!(runtime::basic_io::Stdout, "Hello, World!");
    runtime::terminate();
}
