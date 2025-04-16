use crate::SyscallResult;
use crate::ToLinuxResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;

#[cfg(feature = "fd")]
pub fn sys_close(fd: c_int) -> SyscallResult {
    api::sys_close(fd).to_linux_result()
}

#[cfg(feature = "fd")]
pub fn sys_dup(old_fd: c_int) -> SyscallResult {
    api::sys_dup(old_fd).to_linux_result()
}
#[cfg(feature = "fd")]

pub fn sys_dup2(old_fd: c_int, new_fd: c_int) -> SyscallResult {
    api::sys_dup2(old_fd, new_fd).to_linux_result()
}
#[cfg(feature = "fd")]
pub fn sys_fcntl(fd: c_int, cmd: c_int, arg: usize) -> SyscallResult {
    api::sys_fcntl(fd, cmd, arg).to_linux_result()
}

#[cfg(feature = "fd")]
pub fn sys_dup3(old_fd: c_int, new_fd: c_int) -> SyscallResult {
    api::sys_dup2(old_fd, new_fd).to_linux_result()
}
