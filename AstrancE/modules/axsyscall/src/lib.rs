//! syscall impl for AstrancE
mod test;

use syscalls::Sysno;
/// sysno参考[参考文件](file:///root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/syscall_imp-0.6.18/src/arch/riscv64.rs)
// 声明 axsyscalls 模块
// 声明 syscall_imp 模块（对应 syscall_imp 目录）
pub mod syscall_imp;
pub use syscall_imp::*;
use crate::io::*;
use crate::fs::*;
use crate::task::*;
use core::ffi::c_int;
use arceos_posix_api::ctypes;
use crate::time::*;

pub fn syscall_handler(sys_id: usize, args: [usize; 6]) -> Result<isize,isize> {
    let sys_id = Sysno::from(sys_id as u32);//检查id与测例是否适配

    let ret = match sys_id {
        Sysno::write => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, size) };
            if size == 0 {
                return Err(-1);
            } else {
                ax_write(fd, buf)
            }
        }
        Sysno::read => {
            let fd = args[0];
            let buf_ptr = args[1];
            let size = args[2];
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, size) };
            if size == 0 {
                return Err(-1);
            } else {
                ax_read(fd, buf)
            }
        }
        // 文件操作相关系统调用
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
            ax_exit(code)
        }
        Sysno::getpid => {
            ax_getpid()
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
            ax_yield()
        }
        // 时间相关系统调用
        Sysno::clock_gettime => {
            let cls = args[0];
            let ts: *mut ctypes::timespec= args[1] as *mut ctypes::timespec;
            ax_clock_gettime(cls as ctypes::clockid_t, ts)
        }
        Sysno::gettimeofday => {
            let ts: *mut ctypes::timeval= args[0] as *mut ctypes::timeval;
            ax_get_time_of_day(ts)
        }
        Sysno::nanosleep => {
            let req : *const ctypes::timespec = args[0] as *const ctypes::timespec;
            let rem: *mut ctypes::timespec = args[1] as *mut ctypes::timespec;
            ax_nanosleep(req, rem)
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
        _ => {
            Err(-1) // Return error code for unsupported syscall_imp
        }
    };

    ret
}
