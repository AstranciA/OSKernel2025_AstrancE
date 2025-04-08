/*!
AstrancE lib for building a monolithic kernel
This library mainly provides a set of functions for userspace applications to interact with the kernel.
Although AstrancE is designed to be a unikernel, some may want to use user mode codes.
*/
#![no_std]
extern crate alloc;
#[macro_use]
extern crate axlog;

pub mod ctypes;
use axerrno::AxResult;
use axmm::kernel_aspace;

pub mod elf;
pub mod loader;

#[cfg(feature = "process")]
pub mod task;

#[cfg(feature = "mm")]
pub mod mm;

#[cfg(any(feature = "mm", feature = "process"))]
/// If the target architecture requires it, the kernel portion of the address
/// space will be copied to the user address space.
/// TODO: unsafe. using trampoline instead
pub fn copy_from_kernel(aspace: &mut axmm::AddrSpace) -> AxResult {
    use axmm::kernel_aspace;

    if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
        // ARMv8 (aarch64) and LoongArch64 use separate page tables for user space
        // (aarch64: TTBR0_EL1, LoongArch64: PGDL), so there is no need to copy the
        // kernel portion to the user page table.
        aspace.copy_mappings_from(&kernel_aspace().lock(), false)?;
    }

    Ok(())
}
