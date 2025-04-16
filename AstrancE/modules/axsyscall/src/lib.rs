//! syscall impl for AstrancE
// #![no_std]
#![cfg_attr(not(test), no_std)]
#![feature(stmt_expr_attributes)]
// #![cfg(test)]

mod test;
#[macro_use]
use core::fmt::Debug;
extern crate axlog;
use axerrno::{AxError, LinuxError, LinuxResult};
use syscall_imp::{fs::sys_chdir, sys::sys_uname};
use syscalls::Sysno;
mod syscall_imp;
use arceos_posix_api::{FD_TABLE, char_ptr_to_str, ctypes};
use axfs::{
    CURRENT_DIR,
    api::{create_dir, current_dir, set_current_dir},
    fops::Directory,
};
use core::ffi::*;
pub mod result;
pub use result::{SyscallResult, ToLinuxResult};

#[macro_export]
macro_rules! syscall_handler_def {
    ($($(#[$attr:meta])* $sys:ident => $args:tt $body:expr $(,)?)*) => {
        #[axhal::trap::register_trap_handler(axhal::trap::SYSCALL)]
        pub fn handle_syscall(tf: &axhal::arch::TrapFrame, syscall_num: usize) -> Option<isize> {
            use syscalls::Sysno;
            use $crate::result::{SyscallResult, LinuxResultToIsize};
            let args = [tf.arg0(), tf.arg1(), tf.arg2(), tf.arg3(), tf.arg4(), tf.arg5()];
            let sys_id = Sysno::from(syscall_num as u32);

            let result:Option<SyscallResult> = match sys_id {
                $(
                    $(#[$attr])*
                    Sysno::$sys => {
                        axlog::debug!("handle syscall: {}({:?})", stringify!($sys), args);
                        // TODO: remove #![feature(stmt_expr_attributes)]
                        Some((
                            #[inline(always)]
                            || -> SyscallResult {
                                let $args = args;
                                $body
                        })())
                    }
                ),*,
                _ => {
                        axlog::debug!("handle syscall: {}({:?})", stringify!(none), args);
                        None
                }
            };
            result.map(|r| r.as_isize())
        }
    };
}

/*
 *macro_rules! get_args {
 *    ($($arg:ident),* $(,)?) => {
 *        let [$($arg),* ..] = args;
 *    };
 *}
 */

syscall_handler_def!(
        write => [fd,buf_ptr,size,..] {
            let buf = unsafe { core::slice::from_raw_parts(buf_ptr as _, size) };
            syscall_imp::io::sys_write(fd, buf)
        }

        read => [fd, buf_ptr, size, ..] {
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, size) };
            syscall_imp::io::sys_read(fd, buf)
        }

        writev => [fd, iov, iocnt, ..] {
            syscall_imp::io::sys_writev(fd as _, iov as _, iocnt as _)
        }
        // 文件操作相关系统调用
        #[cfg(all(feature = "fs", feature = "fd"))]
        openat => [dirfd, fname, flags, mode, ..] {
            syscall_imp::fs::sys_openat(dirfd as _, fname as _, flags as _, mode as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        close => [fd, ..] {
            syscall_imp::fd::sys_close(fd as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        statfs => args {
            todo!()
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        fstat => [fd, buf, ..] {
            unsafe { syscall_imp::fs::sys_fstat(fd as _, buf as _) }
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        lseek => [fd, offset, whence, ..] {
            syscall_imp::fs::sys_lseek(fd as _, offset as _, whence as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        getcwd => [buf, size, ..] {
            syscall_imp::fs::sys_getcwd(buf as _, size as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        renameat => [old, new, ..] {
            syscall_imp::fs::sys_rename(old as _, new as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        dup => [old_fd, ..] {
            syscall_imp::fd::sys_dup(old_fd as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        dup3 => [old_fd, new_fd, ..] {
            syscall_imp::fd::sys_dup3(old_fd as _, new_fd as _)
        }

        #[cfg(all(feature = "fs", feature = "fd"))]
        fcntl => [fd, cmd, arg, ..] {
            syscall_imp::fd::sys_fcntl(fd as _, cmd as _, arg as _)
        }
        #[cfg(feature = "pipe")]
        pipe2 => [fds, ..] {
            let fds = unsafe { core::slice::from_raw_parts_mut(fds as *mut c_int, 2) };
            syscall_imp::pipe::sys_pipe(fds)
        }

        // 进程控制相关系统调用
        exit => [code,..] {
            syscall_imp::task::sys_exit(code as _)
        }
        getpid => args syscall_imp::task::sys_getpid()
        sched_yield => args syscall_imp::task::sys_yield()
        // 时间相关系统调用
        clock_gettime => args {
            let cls = args[0];
            let ts: *mut ctypes::timespec = args[1] as *mut ctypes::timespec;
            syscall_imp::time::sys_clock_gettime(cls as ctypes::clockid_t, ts)
        }
        clock_gettime64 => args {
            let cls = args[0];
            let ts: *mut ctypes::timespec = args[1] as *mut ctypes::timespec;
            syscall_imp::time::sys_clock_gettime(cls as ctypes::clockid_t, ts)
        }
        gettimeofday => args {
            let ts: *mut ctypes::timeval = args[0] as *mut ctypes::timeval;
            syscall_imp::time::sys_get_time_of_day(ts)
        }
        nanosleep => args {
            let req: *const ctypes::timespec = args[0] as *const ctypes::timespec;
            let rem: *mut ctypes::timespec = args[1] as *mut ctypes::timespec;
            syscall_imp::time::sys_nanosleep(req, rem)
        }
        clock_nanosleep_time64 => args {
            // TODO: handle clock_id and flags
            let _clock_id = args[0];
            let _flags = args[1];
            let req: *const ctypes::timespec = args[2] as *const ctypes::timespec;
            let rem: *mut ctypes::timespec = args[3] as *mut ctypes::timespec;
            syscall_imp::time::sys_nanosleep(req, rem)
        }
        // 其他系统调用
        uname => args sys_uname(args[0] as _),

        #[cfg(all(feature = "fs", feature = "fd"))]
        chdir => args sys_chdir(args[0] as _),
        // TODO: handle dir_fd and prem
        #[cfg(all(feature = "fs", feature = "fd"))]
        mkdirat => [dir_fd, path, perm, ..] {
            syscall_imp::fs::sys_mkdirat(dir_fd, path as _, perm)
        }
        getdents64 => args {
            todo!()
        }

        //网络相关
        #[cfg(feature = "net")]
        socket => [domain, socktype, protocol, ..] {
            syscall_imp::net::sys_socket(domain as _, socktype as _, protocol as _)
        }
        #[cfg(feature = "net")]
        bind => [fd, addr, addrlen, ..] {
            syscall_imp::net::sys_bind(fd as _, addr as _, addrlen as _)
        }
        #[cfg(feature = "net")]
        // fd, addr, addrlen
        connect => [fd, addr, addrlen, ..] {
            syscall_imp::net::sys_connect(fd as _, addr as _, addrlen as _)
        }
        #[cfg(feature = "net")]
        // fd, buf, len, flags, addr, addrlen
        sendto => [fd, buf, len, flags, addr, addrlen, ..] {
            syscall_imp::net::sys_sendto(
                fd as _,
                buf as _,
                len as _,
                flags as _,
                addr as _,
                addrlen as _,
            )
        }

        #[cfg(feature = "net")]
        // fd, buf, len, flags
        sendmsg => [fd, buf, len, flags, ..] {
            syscall_imp::net::sys_send(fd as _, buf as _, len as _, flags as _)
        }

        #[cfg(feature = "net")]
        // fd, buf, len, flags, addr, addrlen
        recvfrom => [fd, buf, len, flags, addr, addrlen, ..] {
            unsafe {
                syscall_imp::net::sys_recvfrom(
                    fd as _,
                    buf as _,
                    len as _,
                    flags as _,
                    addr as _,
                    addrlen as _,
                )
            }
        }

        #[cfg(feature = "net")]
        // fd, buf, len, flags
        recvmsg => [fd, buf, len, flags, ..] {
            syscall_imp::net::sys_recv(fd as _, buf as _, len as _, flags as _)
        }

        #[cfg(feature = "net")]
        // fd, backlog
        listen => [fd, backlog, ..] {
            syscall_imp::net::sys_listen(fd as _, backlog as _)
        }

        #[cfg(feature = "net")]
        // fd, addr, addrlen
        accept => [fd, addr, addrlen, ..] {
            unsafe { syscall_imp::net::sys_accept(fd as _, addr as _, addrlen as _) }
        }

        #[cfg(feature = "net")]
        // fd, how
        shutdown => [fd, how, ..] {
            syscall_imp::net::sys_shutdown(fd as _, how as _)
        }

        #[cfg(feature = "net")]
        // fd, addr, addrlen
        getsockname => [fd, addr, addrlen, ..] {
            unsafe { syscall_imp::net::sys_getsockname(fd as _, addr as _, addrlen as _) }
        }

        #[cfg(feature = "net")]
        // fd, addr, addrlen
        getpeername => [fd, addr, addrlen, ..] {
            unsafe { syscall_imp::net::sys_getpeername(fd as _, addr as _, addrlen as _) }
        }

);
