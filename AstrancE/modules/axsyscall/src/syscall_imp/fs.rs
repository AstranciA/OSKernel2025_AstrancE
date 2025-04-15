use crate::{syscall_result, SyscallResult};
use arceos_posix_api::char_ptr_to_str;
use arceos_posix_api::{self as api, ctypes};
use axfs::api::{create_dir, set_current_dir};
use core::ffi::c_char;
use core::ffi::c_int;

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

pub fn sys_lseek(fd: c_int, offset: ctypes::off_t, whence: c_int) -> SyscallResult {
    let ret = api::sys_lseek(fd, offset, whence) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub unsafe fn sys_stat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult {
    let ret = api::sys_stat(path, buf) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub unsafe fn sys_fstat(fd: c_int, buf: *mut ctypes::stat) -> SyscallResult {
    let ret = unsafe { api::sys_fstat(fd, buf) } as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub unsafe fn sys_lstat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult {
    let ret = api::sys_lstat(path, buf) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub fn sys_getcwd(buf: *mut c_char, size: usize) -> SyscallResult {
    let ret = api::sys_getcwd(buf, size) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub fn sys_rename(old: *const c_char, new: *const c_char) -> SyscallResult {
    let ret = api::sys_rename(old, new) as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    } else {
        SyscallResult::Success(ret)
    }
}

pub fn sys_mkdirat(dir_fd: usize, dir_path: *const c_char, mode: usize) -> SyscallResult {
    syscall_result!(arceos_posix_api::sys_mkdirat(dir_fd.try_into().unwrap(), dir_path, mode.try_into().unwrap()))
    /*
     *    let ret = unsafe { char_ptr_to_str(dir_path) }.map(|dir_path|
     *        {
     *
     *
     *        create_dir(dir_path)
     *        }
     *
     *    );
     */
    /*
     *match ret {
     *    Ok(_) => SyscallResult::Success(0),
     *    Err(e) => SyscallResult::Error(e.into()),
     *}
     */
}

pub fn sys_chdir(path: *const c_char) -> SyscallResult {
    let ret = unsafe { char_ptr_to_str(path) }.map(|chdir_path| set_current_dir(&chdir_path));
    match ret {
        Ok(_) => SyscallResult::Success(0),
        Err(e) => SyscallResult::Error(e.into()),
    }
}
