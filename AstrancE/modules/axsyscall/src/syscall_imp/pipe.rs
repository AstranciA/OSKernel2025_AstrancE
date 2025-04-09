use crate::SyscallResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;

#[cfg(feature = "pipe")]

pub fn sys_pipe(fds: &mut [c_int]) -> SyscallResult {
    let ret = api::sys_pipe(fds) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    }else {
        SyscallResult::Success(ret)
    }
}