#![no_std]
#![no_main]

#[macro_use]
extern crate axstd;

#[unsafe(no_mangle)]
fn _simple_1() -> i32 {
    1
}
