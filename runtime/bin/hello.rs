#![no_std]
#![no_main]

use core::fmt::Write;
use runtime;

#[no_mangle]
fn main() {
    let _ = writeln!(runtime::basic_io::Stdout, "Hello, World!").is_ok();
}
