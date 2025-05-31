#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use core::fmt::Write;

use runtime::basic_io::Stdout;

#[no_mangle]
fn main() {
    let nums = vec![-100, 20, 124, 33, 123, 12, 01];
    for i in nums {
        let _ = writeln!(Stdout, "{}", i);
    }
}
