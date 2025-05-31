#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;

use rt;

pub fn quick_sort(slice: &Vec<i32>, low: usize, high: usize) {
    if low < high {
        let pivot = sub_sort(slice, low, high);
        if pivot > 0 {
            quick_sort(slice, low, pivot - 1)
        }
        quick_sort(slice, pivot + 1, high);
    }
}

pub fn sub_sort(slice: &Vec<i32>, low: usize, high: usize) -> usize {
    let pivot = high;
    let mut i = low as isize - 1;

    for j in low..high {
        // println!("J now is: {}", j);
        if slice[j] < slice[pivot] {
            i += 1;
            slice.swap(i as usize, j);
        }
    }
    slice.swap((i + 1) as usize, pivot);
    (i + 1) as usize
}

#[no_mangle]
fn main() {
    let nums: Vec<i32> = vec![-12, 10, 100, 0, 55, -155, 22, 40, 101];
    quick_sort(&nums, 0, nums.len() - 1);
}
