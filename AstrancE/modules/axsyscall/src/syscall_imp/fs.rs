use crate::SyscallResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;

#[cfg(feature = "fs")]
pub fn sys_openat(
    dirfd: c_int,
    filename: *const c_char,
    flags: c_int,
    mode: ctypes::mode_t,
) -> SyscallResult {
    let ret = api::sys_openat(dirfd, filename, flags, mode) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}
#[cfg(feature = "fs")]
pub fn sys_lseek(fd: c_int, offset: ctypes::off_t, whence: c_int) -> SyscallResult {
    let ret = api::sys_lseek(fd,offset,whence) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fs")]
pub unsafe fn sys_stat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult{
    let ret = api::sys_stat(path,buf) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fs")]
pub unsafe fn sys_fstat(fd: c_int, buf: *mut ctypes::stat) -> SyscallResult{
    let ret = unsafe { api::sys_fstat(fd, buf) } as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fs")]
pub unsafe fn sys_lstat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult {
    let ret = api::sys_lstat(path,buf) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fs")]
pub fn sys_getcwd(buf: *mut c_char, size: usize) -> SyscallResult {
    let ret = api::sys_getcwd(buf,size) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

#[cfg(feature = "fs")]
pub fn sys_rename(old: *const c_char, new: *const c_char) -> SyscallResult {
    let ret = api::sys_rename(old,new) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}
