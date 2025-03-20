#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;
use axsyscall::syscall_handler;

const s:  &str = "Hello, wgfdfcszorld!";
#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("Hello, world!");
    let o = syscall_handler(64,[1, s.as_ptr() as usize,s.len(),0,0,0]);
}
