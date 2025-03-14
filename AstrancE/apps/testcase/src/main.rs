//#![cfg_attr(feature = "axstd", no_std)]
//#![cfg_attr(feature = "axstd", no_main)]
#![no_std]
#![no_main]

mod loader;

use core::arch::global_asm;

#[macro_use]
extern crate axstd;
extern crate axalloc;

use loader::load_file;

global_asm!(include_str!("../link_apps.S"));

//#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
#[unsafe(no_mangle)]
fn main() {
    load_file();
    println!("Hello, world!");
}
