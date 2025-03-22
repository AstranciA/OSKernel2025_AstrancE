//! syscall impl for AstrancE
// #![no_std]
#![cfg_attr(not(test), no_std)]
// #![cfg(test)]

mod test;

use syscalls::Sysno;
mod syscall_imp;
use core::ffi::*;
use arceos_posix_api::ctypes;
use syscall_imp::errno::LinuxError;

pub enum SyscallResult{ Success(isize), Error(LinuxError) }

impl From<SyscallResult> for isize {
    fn from(result: SyscallResult) -> isize {
        match result {
            SyscallResult::Success(val) => val as isize,
            SyscallResult::Error(e) => {
                -(e as isize)
            }
        }
    }
}
///SyscallResult 可直接into为有符号整数，其中错误值以负数返回，linuxError有
/// 方法as_str返回对应错误的具体文字描述


pub fn syscall_handler(sys_id: usize, args: [usize; 6]) -> SyscallResult {
    let sys_id = Sysno::from(sys_id as u32);//检查id与测例是否适配

    let ret = match sys_id {
        Sysno::write => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, size) };
            syscall_imp::io::ax_write(fd, buf)
            // call_with_args!(syscall_imp::io::ax_write,args)
        }
        //宏承接参数！！！！！！！！
        // sysmatch!(Sysno::read, handler, 3)
        // #[sysmatch(Sysno::read)]
        Sysno::read => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, size) };
                syscall_imp::io::ax_read(fd, buf)
        }
        // 文件操作相关系统调用
        #[cfg(all(feature = "fs", feature = "fd"))]
        Sysno::openat => {
            let dirfd = args[0];//类型检查与转化！
            let fname = args[1];
            let flages = args[2];
            let mode = args[3];
            todo!()
        }
        Sysno::close => {
            todo!()
        }
        Sysno::statfs => {
            todo!()
        }
        Sysno::fstat => {
            todo!()
        }
        Sysno::lseek => {
            todo!()
        }
        Sysno::mmap => {
            todo!()
        }
        Sysno::munmap => {
            todo!()
        }
        
        // 进程控制相关系统调用
        Sysno::exit => {
            let code = args[0] as c_int;
            syscall_imp::task::ax_exit(code)
        }
        Sysno::getpid => {
            syscall_imp::task::ax_getpid()
        }
        Sysno::clone => {
            todo!()
        }
        Sysno::execve => {
            todo!()
        }
        Sysno::wait4 => {
            todo!()
        }
        Sysno::sched_yield => {
            syscall_imp::task::ax_yield()
        }
        // 时间相关系统调用
        Sysno::clock_gettime => {
            let cls = args[0];
            let ts: *mut ctypes::timespec= args[1] as *mut ctypes::timespec;
            syscall_imp::time::ax_clock_gettime(cls as ctypes::clockid_t, ts)
        }
        Sysno::gettimeofday => {
            let ts: *mut ctypes::timeval= args[0] as *mut ctypes::timeval;
            syscall_imp::time::ax_get_time_of_day(ts)
        }
        Sysno::nanosleep => {
            let req : *const ctypes::timespec = args[0] as *const ctypes::timespec;
            let rem: *mut ctypes::timespec = args[1] as *mut ctypes::timespec;
            syscall_imp::time::ax_nanosleep(req, rem)
        }
        
        // 其他系统调用
        Sysno::brk => {
            todo!()
        }
        Sysno::uname => {
            todo!()
        }
        Sysno::getcwd => {
            todo!()
        }
        Sysno::dup => {
            todo!()
        }
        Sysno::dup3 => {
            todo!()
        }
        Sysno::chdir => {
            todo!()
        }
        Sysno::mkdirat => {
            todo!()
        }
        Sysno::getdents64 => {
            todo!()
        }

        //网络相关
        #[cfg(feature = "net")]
        Sysno::socket => {
            let domain = args[0] as c_int;
            let socktype = args[1] as c_int;
            let protocol = args[2] as c_int;
            syscall_imp::net::ax_socket(domain, socktype, protocol)
        }
        #[cfg(feature = "net")]
        Sysno::bind => {
            let fd = args[0] as c_int;
            let addr = args[1] as *const ctypes::sockaddr;
            let addrlen = args[2] as ctypes::socklen_t;
            syscall_imp::net::ax_bind(fd, addr, addrlen)
        }
        #[cfg(feature = "net")]
        // fd, addr, addrlen
        Sysno::connect => {
            let fd = args[0] as c_int;
            let addr = args[1] as *const ctypes::sockaddr;
            let addrlen = args[2] as ctypes::socklen_t;
            syscall_imp::net::ax_connect(fd, addr, addrlen)
        }
        #[cfg(feature = "net")]
        // fd, buf, len, flags, addr, addrlen
        Sysno::sendto => {
            let fd = args[0] as c_int;
            let buf = args[1] as *const c_void;
            let len = args[2] as usize;
            let flags = args[3] as c_int;
            let addr = args[4] as *const ctypes::sockaddr;
            let addrlen = args[5] as ctypes::socklen_t;
            syscall_imp::net::ax_sendto(fd, buf, len, flags, addr, addrlen)
        }
        #[cfg(feature = "net")]
        // fd, buf, len, flags
        Sysno::sendmsg => {
            let fd = args[0] as c_int;
            let buf = args[1] as *const c_void;
            let len = args[2] as usize;
            let flags = args[3] as c_int;
            syscall_imp::net::ax_send(fd, buf, len, flags)
        }
        #[cfg(feature = "net")]
        // fd, buf, len, flags, addr, addrlen
        Sysno::recvfrom => {
            let fd = args[0] as c_int;
            let buf = args[1] as *mut c_void;
            let len = args[2] as usize;
            let flags = args[3] as c_int;
            let addr = args[4] as *mut ctypes::sockaddr;
            let addrlen = args[5] as *mut ctypes::socklen_t;
            unsafe { syscall_imp::net::ax_recvfrom(fd, buf, len, flags, addr, addrlen) }
        }
        #[cfg(feature = "net")]
        // fd, buf, len, flags
        Sysno::recvmsg => {
            let fd = args[0] as c_int;
            let buf = args[1] as *mut c_void;
            let len = args[2] as usize;
            let flags = args[3] as c_int;
            syscall_imp::net::ax_recv(fd, buf, len, flags)
        }
        #[cfg(feature = "net")]
        // fd, backlog
        Sysno::listen => {
            let fd = args[0] as c_int;
            let backlog = args[1] as c_int;
            syscall_imp::net::ax_listen(fd, backlog)
        }
        #[cfg(feature = "net")]
        // fd, addr, addrlen
        Sysno::accept => {
            let fd = args[0] as c_int;
            let addr = args[1] as *mut ctypes::sockaddr;
            let addrlen = args[2] as *mut ctypes::socklen_t;
            unsafe { syscall_imp::net::ax_accept(fd, addr, addrlen) }
        }
        #[cfg(feature = "net")]
        // fd, how
        Sysno::shutdown => {
            let fd = args[0] as c_int;
            let how = args[1] as c_int;
            syscall_imp::net::ax_shutdown(fd, how)
        }
        #[cfg(feature = "net")]
        // fd, addr, addrlen
        Sysno::getsockname => {
            let fd = args[0] as c_int;
            let addr = args[1] as *mut ctypes::sockaddr;
            let addrlen = args[2] as *mut ctypes::socklen_t;
            unsafe { syscall_imp::net::ax_getsockname(fd, addr, addrlen) }
        }
        #[cfg(feature = "net")]
        // fd, addr, addrlen
        Sysno::getpeername => {
            let fd = args[0] as c_int;
            let addr = args[1] as *mut ctypes::sockaddr;
            let addrlen = args[2] as *mut ctypes::socklen_t;
            unsafe { syscall_imp::net::ax_getpeername(fd, addr, addrlen) }
        }

        _ => {
            SyscallResult::Error(LinuxError::ENOSYS)
        }
    };

    ret
}

