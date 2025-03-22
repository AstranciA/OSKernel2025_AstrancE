#![no_std]
#![no_main]

#[macro_use]
extern crate axstd;

#[unsafe(no_mangle)]
fn _simple_2() -> usize {
    fib(12)
}

fn fib(n: usize) -> usize {
    if n <= 1 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}
