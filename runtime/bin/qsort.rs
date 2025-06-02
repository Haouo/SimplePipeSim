#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;

use runtime;
use runtime::basic_io::Stdout;

fn quick_sort<T>(vec: &mut Vec<T>)
where
    T: Ord + Clone,
{
    if vec.len() <= 1 {
        return;
    }
    quick_sort_range(vec, 0, vec.len() - 1);
}

fn quick_sort_range<T>(vec: &mut Vec<T>, low: usize, high: usize)
where
    T: Ord + Clone,
{
    if low < high {
        let pivot_index = partition(vec, low, high);

        // 遞迴排序左半部分
        if pivot_index > 0 {
            quick_sort_range(vec, low, pivot_index - 1);
        }

        // 遞迴排序右半部分
        if pivot_index < high {
            quick_sort_range(vec, pivot_index + 1, high);
        }
    }
}

fn partition<T>(vec: &mut Vec<T>, low: usize, high: usize) -> usize
where
    T: Ord + Clone,
{
    // 選擇最後一個元素作為 pivot
    let pivot = vec[high].clone();
    let mut i = low;

    for j in low..high {
        if vec[j] <= pivot {
            vec.swap(i, j);
            i += 1;
        }
    }

    vec.swap(i, high);
    i
}

#[no_mangle]
fn main() {
    let mut numbers = vec![64, 34, 25, 12, 22, 11, 90];
    let _ = writeln!(Stdout, "Before: {:?}", numbers);
    quick_sort(&mut numbers);
    let _ = writeln!(Stdout, "After: {:?}", numbers);

    let mut words = vec!["banana", "apple", "cherry", "date"];
    let _ = writeln!(Stdout, "\nBefore: {:?}", words);
    quick_sort(&mut words);
    let _ = writeln!(Stdout, "After: {:?}", words);
}
