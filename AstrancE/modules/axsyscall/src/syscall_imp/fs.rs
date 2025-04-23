use crate::{SyscallResult, ToLinuxResult};
use arceos_posix_api::{self as api, char_ptr_to_str, ctypes};
use axfs::api::set_current_dir;
use core::ffi::{c_char, c_int};

#[inline]
pub fn sys_openat(
    dirfd: c_int,
    filename: *const c_char,
    flags: c_int,
    mode: ctypes::mode_t,
) -> SyscallResult {
    api::sys_openat(dirfd, filename, flags, mode).to_linux_result()
}

#[inline]
pub fn sys_lseek(fd: c_int, offset: ctypes::off_t, whence: c_int) -> SyscallResult {
    (api::sys_lseek(fd, offset, whence) as isize).to_linux_result()
}

#[inline]
pub unsafe fn sys_stat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult {
    api::sys_stat(path, buf).to_linux_result()
}

#[inline]
pub unsafe fn sys_fstat(fd: c_int, buf: *mut ctypes::stat) -> SyscallResult {
    unsafe { api::sys_fstat(fd, buf) }.to_linux_result()
}

#[inline]
pub unsafe fn sys_fstatat(
    dir_fd: c_int,
    filename: *const c_char,
    buf: *mut ctypes::stat,
    flags: c_int,
) -> SyscallResult {
    unsafe { api::sys_fstatat(dir_fd, filename, buf, flags).map(|r| r as isize) }
}

#[inline]
pub unsafe fn sys_lstat(path: *const c_char, buf: *mut ctypes::stat) -> SyscallResult {
    api::sys_lstat(path, buf).to_linux_result()
}

#[inline]
pub fn sys_getcwd(buf: *mut c_char, size: usize) -> SyscallResult {
    (api::sys_getcwd(buf, size) as isize).to_linux_result()
}

#[inline]
pub fn sys_rename(old: *const c_char, new: *const c_char) -> SyscallResult {
    api::sys_rename(old, new).to_linux_result()
}

#[inline]
pub fn sys_mkdirat(dir_fd: usize, dir_path: *const c_char, mode: usize) -> SyscallResult {
    arceos_posix_api::sys_mkdirat(
        dir_fd as c_int,
        dir_path,
        mode.try_into().unwrap(),
    )
    .to_linux_result()
}

#[inline]
pub fn sys_chdir(path: *const c_char) -> SyscallResult {
    let ret = char_ptr_to_str(path).map(|chdir_path| set_current_dir(&chdir_path));
    match ret {
        Ok(_) => SyscallResult::Ok(0),
        Err(e) => SyscallResult::Err(e.into()),
    }
}

#[inline]
pub fn sys_getdents(fd: c_int, dirp: *mut ctypes::dirent, count: c_int) -> SyscallResult {
    unsafe { api::sys_getdents(fd, dirp, count) }
}

#[inline]
pub fn sys_unlink(path: *const c_char) -> SyscallResult {
    api::sys_unlink(path)
}

#[inline]
pub fn sys_unlinkat(dir_fd: c_int, path: *const c_char) -> SyscallResult {
    api::sys_unlinkat(dir_fd, path)
}
