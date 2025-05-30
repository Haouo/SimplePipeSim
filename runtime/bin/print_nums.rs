#![no_std]
#![no_main]

use core::fmt::Write;
use runtime::basic_io::Stdout;

#[no_mangle]
fn main() {
    for i in 0..100 {
        let _ = writeln!(Stdout, "{}", i);
    }
}
