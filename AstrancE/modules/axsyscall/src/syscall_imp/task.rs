use crate::SyscallResult;
use crate::ToLinuxResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_void;

pub fn sys_getpid() -> SyscallResult {
    api::sys_getpid().to_linux_result()
}

pub fn sys_exit(code: c_int) -> SyscallResult {
    api::sys_exit(code);
    2.to_linux_result()
}

pub fn sys_yield() -> SyscallResult {
    api::sys_sched_yield().to_linux_result()
}
