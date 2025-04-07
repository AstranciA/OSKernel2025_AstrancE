use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_void;
use crate::SyscallResult;
use crate::syscall_imp::mm;

pub fn ae_getpid() -> SyscallResult {
    let ret = api::sys_getpid() as isize;
    SyscallResult::Success(ret)
}

pub fn ae_exit(code:c_int) -> SyscallResult{
    api::sys_exit(code);
    SyscallResult::Success(2)
}

pub fn ae_yield() -> SyscallResult{
    SyscallResult::Success(api::sys_sched_yield() as isize)
}

pub fn ae_brk(addr:usize) -> SyscallResult{
    let ret = mm::brk::sys_brk(addr);
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    }else {
        SyscallResult::Success(ret)
    }
}