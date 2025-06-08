#![no_std]
#![no_main]

extern crate alloc;
use alloc::{vec, vec::Vec};
use core::fmt::Write;

use runtime;
use runtime::basic_io::Stdout;

fn generate_large_matrix(size: usize) -> Vec<Vec<i32>> {
    let mut matrix = Vec::with_capacity(size);
    let mut counter = 1;
    for _ in 0..size {
        let mut row = Vec::with_capacity(size);
        for _ in 0..size {
            row.push(counter);
            counter += 1;
        }
        matrix.push(row);
    }
    matrix
}

fn matrix_multiply(a: &Vec<Vec<i32>>, b: &Vec<Vec<i32>>) -> Option<Vec<Vec<i32>>> {
    if a.is_empty() || b.is_empty() || a[0].len() != b.len() {
        return None;
    }

    let rows_a = a.len();
    let cols_a = a[0].len();
    let cols_b = b[0].len();

    let mut result = vec![vec![0; cols_b]; rows_a];

    for i in 0..rows_a {
        for j in 0..cols_b {
            for k in 0..cols_a {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }

    Some(result)
}

fn print_matrix(matrix: &Vec<Vec<i32>>, name: &str) {
    writeln!(Stdout, "{}:", name).unwrap();
    for row in matrix {
        write!(Stdout, "[").unwrap();
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                write!(Stdout, ", ").unwrap();
            }
            write!(Stdout, "{:3}", val).unwrap();
        }
        writeln!(Stdout, "]").unwrap();
    }
    writeln!(Stdout).unwrap();
}

#[no_mangle]
fn main() {
    // let matrix_a = vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]];
    // let matrix_b = vec![vec![9, 8, 7], vec![6, 5, 4], vec![3, 2, 1]];
    let matrix_a = generate_large_matrix(15);
    let matrix_b = generate_large_matrix(15);

    print_matrix(&matrix_a, "Matrix A");
    print_matrix(&matrix_b, "Matrix B");

    match matrix_multiply(&matrix_a, &matrix_b) {
        Some(result) => {
            print_matrix(&result, "A × B Result");
        }
        None => {
            let _ = writeln!(Stdout, "Matmul fail!!!").unwrap();
        }
    }
}
