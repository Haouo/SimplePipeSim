#![no_std]
#![no_main]

use core::fmt::Write;
use runtime::basic_io::Stdout;
use runtime::terminate;

fn main() -> ! {
    for i in 0..100 {
        writeln!(Stdout, "{}", i);
    }
    terminate();
}
