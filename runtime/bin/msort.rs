#![no_std]
#![no_main]

extern crate alloc;
use alloc::{vec, vec::Vec};
use core::fmt::Write;

use runtime;
use runtime::basic_io::Stdout;

fn merge_sort<T>(vec: &mut Vec<T>)
where
    T: Ord + Clone,
{
    if vec.len() <= 1 {
        return;
    }
    merge_sort_range(vec, 0, vec.len());
}

fn merge_sort_range<T>(vec: &mut Vec<T>, start: usize, end: usize)
where
    T: Ord + Clone,
{
    if end - start <= 1 {
        return;
    }

    let mid = start + (end - start) / 2;
    merge_sort_range(vec, start, mid);
    merge_sort_range(vec, mid, end);
    merge(vec, start, mid, end);
}

fn merge<T>(vec: &mut Vec<T>, start: usize, mid: usize, end: usize)
where
    T: Ord + Clone,
{
    let left = vec[start..mid].to_vec();
    let right = vec[mid..end].to_vec();

    let mut i = 0;
    let mut j = 0;
    let mut k = start;

    while i < left.len() && j < right.len() {
        if left[i] <= right[j] {
            vec[k] = left[i].clone();
            i += 1;
        } else {
            vec[k] = right[j].clone();
            j += 1;
        }
        k += 1;
    }

    while i < left.len() {
        vec[k] = left[i].clone();
        i += 1;
        k += 1;
    }

    while j < right.len() {
        vec[k] = right[j].clone();
        j += 1;
        k += 1;
    }
}

#[no_mangle]
fn main() {
    let mut numbers = vec![64, 34, 25, 12, 22, 11, 90, 88, 76, 50, 42];
    let _ = writeln!(Stdout, "Before sorting: {:?}", numbers).unwrap();
    merge_sort(&mut numbers);
    let _ = writeln!(Stdout, "After sorting: {:?}", numbers).unwrap();
}
