use crate::SyscallResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;

#[cfg(feature = "fd")]
pub fn ae_close(fd: c_int) -> SyscallResult {
    let ret = api::sys_close(fd) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fd")]
pub fn ae_dup(old_fd: c_int) -> SyscallResult {
    let ret = api::sys_dup(old_fd) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}
#[cfg(feature = "fd")]

pub fn ae_dup2(old_fd: c_int, new_fd: c_int) -> SyscallResult {
    let ret = api::sys_dup2(old_fd, new_fd) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}
#[cfg(feature = "fd")]
pub fn ae_fcntl(fd: c_int, cmd: c_int, arg: usize) -> SyscallResult {
    let ret = api::sys_fcntl(fd, cmd, arg) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fd")]
pub fn ae_dup3(old_fd: c_int, new_fd: c_int) -> SyscallResult {
    let ret = api::sys_dup2(old_fd, new_fd) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}