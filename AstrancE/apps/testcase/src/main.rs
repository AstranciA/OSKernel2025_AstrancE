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
mod ctypes;
pub mod elf;
mod loader;
mod mm;
mod task;
mod trap;

use alloc::sync::Arc;
use axerrno::AxResult;
use axhal::arch::TrapFrame;
use axhal::trap::{SYSCALL, register_trap_handler};
use axhal::{arch::UspaceContext, mem::VirtAddr};
use axmm::{kernel_aspace, AddrSpace};
use axstd::println;
use axsync::Mutex;
use mm::load_user_app;

global_asm!(include_str!("../link_apps.S"));

//#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
#[unsafe(no_mangle)]
fn main() {
    println!("Hello, world!");
    let (entry_vaddr, ustack_top, uspace) = load_user_app("hello").unwrap();
    debug!(
        "app_entry: {:?}, app_stack: {:?}, app_aspace: {:?}",
        entry_vaddr, ustack_top, uspace
    );
    let uctx = UspaceContext::new(entry_vaddr.into(), ustack_top, 2333);
    println!("User task created");
    let user_task = task::spawn_user_task(Arc::new(Mutex::new(uspace)), uctx);
    println!("User task spawned" );
    let exit_code = user_task.join();
    info!("User task exited with code: {:?}", exit_code);
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    debug!("syscall_handler: {:?}", syscall_num);
    todo!();
}

/// If the target architecture requires it, the kernel portion of the address
/// space will be copied to the user address space.
fn copy_from_kernel(aspace: &mut AddrSpace) -> AxResult {
    if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
        // ARMv8 (aarch64) and LoongArch64 use separate page tables for user space
        // (aarch64: TTBR0_EL1, LoongArch64: PGDL), so there is no need to copy the
        // kernel portion to the user page table.
        aspace.copy_mappings_from(&kernel_aspace().lock())?;
    }


    Ok(())
}

