use core::{
    error,
    ffi::{CStr, c_char, c_void},
};

use crate::task::ThreadData;
use crate::{
    ctypes::{CloneFlags, WaitStatus},
    mm::mmap::MmapIOImpl,
    task::{self, time_stat_from_user_to_kernel, time_stat_output},
};
use alloc::{string::String, sync::Arc, vec::Vec};
use arceos_posix_api::{
    self as api, char_ptr_to_str, ctypes::*, get_file_like, str_vec_ptr_to_str, sys_read,
};
use axerrno::{AxError, LinuxError};
use axfs::{CURRENT_DIR, api::set_current_dir, fops::Directory};
use axhal::trap::{PRE_TRAP, register_trap_handler};
use axhal::{arch::TrapFrame, time::nanos_to_ticks};
use axmm::{MmapFlags, MmapPerm};
use axsyscall::{ToLinuxResult, apply, syscall_handler_def};
use axtask::{CurrentTask, TaskExtMut, TaskExtRef, current};
use core::ffi::c_int;
use linux_raw_sys::general as linux;
use memory_addr::MemoryAddr;
use syscalls::Sysno;

mod mm;

syscall_handler_def!(
        exit => [code,..] {
            crate::task::sys_exit((code & 0xff) as i32)
        }
        exit_group => [code,..]{
            task::exit::sys_exit_group((code & 0xff) as i32)
        }
        clone => [flags, sp, ..] {
            let clone_flags = CloneFlags::from_bits_retain(flags as u32);

            let child_task = task::clone_task(
                if (sp != 0) { Some(sp) } else { None },
                clone_flags,
                true,
            )?;
            Ok(child_task.task_ext().thread.process().pid() as isize)
        }
        wait4 => [pid, wstatus, options, reusage, ..] {
            let curr = current();
            crate::sys_waitpid(
                pid as i32,
                wstatus.into(),
                options as u32
            )
        }
        execve => [pathname, argv, envp, ..] {
            let pathname = char_ptr_to_str(pathname as *const c_char)?;
            let argv: Vec<String> = str_vec_ptr_to_str(argv as *const *const c_char)?.into_iter().map(|s| String::from(s)).collect();
            let envp: Vec<String> = str_vec_ptr_to_str(envp as *const *const c_char)?.into_iter().map(|s| String::from(s)).collect();

            let err = task::exec_current(
                pathname,
                &argv.as_slice(),
                &envp.as_slice()
            ).expect_err("successful execve should not reach here");
            Err(err.into())
        }
        brk => [brk, ..] {
            apply!(mm::sys_brk, brk)
        }
        set_tid_address => args{
                let tidptr = args[0];
                let tid: usize = current().task_ext().thread.tid() as _;
                current().task_ext().thread_data().set_clear_child_tid(tidptr);
                Ok(tid as isize)
        }
        mmap => args {
            let curr = current();
            let mut aspace = curr.task_ext().process_data().aspace.lock();
            let perm = MmapPerm::from_bits(args[2]).ok_or(LinuxError::EINVAL)?;
            let flags = MmapFlags::from_bits(args[3]).ok_or(LinuxError::EINVAL)?;
            let fd = args[4];
            let offset = args[5];
            if let Ok(va) = aspace.mmap(
                args[0].into(),
                args[1],
                perm,
                flags,
                Arc::new(MmapIOImpl {
                    fd: fd as c_int,
                    file_offset: offset.try_into().unwrap(),
                    flags
                }),
                false,
            ) {
                return Ok(va.as_usize() as isize);
            }
            Err(LinuxError::EPERM)
        }
        munmap => args {
            let curr = current();
            let mut aspace = curr.task_ext().process_data().aspace.lock();
            let start = args[0].into();
            let size = args[1].align_up_4k();
            if aspace.munmap(start, size).is_ok() {
                Ok(0)
            } else {
                // TODO
                Err(LinuxError::EPERM)
            }
        }
        mprotect => [addr, size, prot, ..] {
            apply!(mm::sys_mprotect, addr, size, prot)
        }
        getpid => _ {
            Ok(current().task_ext().thread.process().pid() as _)
        }
        gettid => _ {
            Ok(current().task_ext().thread.tid() as _)
        }
        getppid => _ {
            current().task_ext().thread.process().parent().map(|p|p.pid() as _).ok_or(LinuxError::EINVAL)
        }
        geteuid => _{
            //TODO: returns the effective user ID of the calling process
            Ok(0)
        }
        getegid => _{
            //TODO:returns the effective group ID of the calling process
            Ok(0)
        }
        // FIXME: cutime cstimes
        times => args {
            let curr_task = current();
            let (utime_ns, stime_ns) = curr_task.task_ext().time_stat_output();
            let utime = nanos_to_ticks(utime_ns.try_into().map_err(|_| AxError::BadState)?);
            let stime = nanos_to_ticks(stime_ns.try_into().map_err(|_| AxError::BadState)?);
            let tms = api::ctypes::tms {
                tms_utime: utime.try_into().unwrap(),
                tms_stime: stime.try_into().unwrap(),
                tms_cutime: utime.try_into().unwrap(),
                tms_cstime: stime.try_into().unwrap(),
            };
            unsafe {
                *(args[0] as *mut api::ctypes::tms) = tms;
            }
            Ok(0)
            //unsafe { core::slice::from_raw_parts_mut(args[0] as *mut api::ctypes::tms, 1).copy_from_slice(tms); }
        }
        rt_sigaction => [signum, act, oldact, ..] {
            task::signal::sys_sigaction(signum.try_into().map_err(|_| LinuxError::EINVAL)?, act as _, oldact as _)
        }
        rt_sigprocmask => [how, set, oldset, ..] {
            task::signal::sys_sigprocmask(how.try_into().map_err(|_| LinuxError::EINVAL)?, set as _, oldset as _)
        }
        rt_sigtimedwait => [set, info, timeout, ..] {
            task::signal::sys_sigtimedwait(set as _, info as _, timeout as _).map(|sig| sig as isize)
        }
        rt_sigreturn => _ {
            task::signal::sys_sigreturn()
        }
        kill => [pid, sig, ..] {
            task::signal::sys_kill(pid as _, sig as _)
        }
);
