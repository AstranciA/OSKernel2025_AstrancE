//#![cfg_attr(feature = "axstd", no_std)]
//#![cfg_attr(feature = "axstd", no_main)]
#![feature(new_range_api)]
#![no_std]
#![no_main]


use core::arch::global_asm;

#[macro_use]
extern crate axstd;
#[macro_use]
extern crate axlog;
#[macro_use]
extern crate alloc;

mod config;
mod loader;
mod mm;
mod trap;
pub mod elf;

use axstd::println;
use mm::{init_mm, load_user_app};

global_asm!(include_str!("../link_apps.S"));

//#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
#[unsafe(no_mangle)]
fn main() {
    init_mm();
    debug!("Hello, world!");

    println!("Hello, world!");
    //load_user_app("hello").unwrap();

    println!("Hello, world!");
}
