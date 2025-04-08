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
use core::clone;

use axerrno::AxResult;
use axhal::arch::TrapFrame;
use axhal::trap::{SYSCALL, register_trap_handler};
use axmm::kernel_aspace;
use axtask::current;
use ctypes::{CloneFlags, WaitStatus};
use syscalls::Sysno;
use task::wait_pid;

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

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> Option<isize> {
    let args = [
        tf.arg0(),
        tf.arg1(),
        tf.arg2(),
        tf.arg3(),
        tf.arg4(),
        tf.arg5(),
    ];

    let sys_id = Sysno::from(syscall_num as u32); //检查id与测例是否适配

    let ret = match sys_id {
        Sysno::clone => {
            let curr = current();
            let clone_flags = CloneFlags::from_bits(args[0] as u32);
            if clone_flags.is_none() {
                error!("Invalid clone flags: {}", args[0]);
                axtask::exit(-1); // FIXME: return error code
            }
            let clone_flags = clone_flags.unwrap();

            let child_task = task::clone_task(
                curr.as_task_ref().clone(),
                if (args[0] != 0) { Some(args[0]) } else { None },
                clone_flags,
                true,
            )
            .unwrap();
            axtask::spawn_task_by_ref(child_task.clone());
            Some(child_task.id().as_u64() as isize)
        }
        Sysno::wait4 => {
            let curr = current();
            let mut result = None;
            while let wait_result = wait_pid(
                curr.as_task_ref().clone(),
                args[0] as i32,
                args[1] as *mut i32,
            ) {
                let r = match wait_result {
                    Ok(pid) => {
                        result = Some(pid as isize);
                        break;
                    },
                    Err(WaitStatus::NotExist) => {
                        result = Some(0);
                        break;
                    },
                    Err(e) => {
                        debug!("wait4: {:?}, keep waiting...", e);
                    }
                };
            }
            result
        }
        _ => None,
    };
    ret
}
