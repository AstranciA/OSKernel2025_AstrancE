use crate::SyscallResult;
use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;

#[cfg(feature = "pipe")]

pub fn sys_pipe(fds: &mut [c_int]) -> SyscallResult {
    use crate::ToLinuxResult;

    api::sys_pipe(fds).to_linux_result()
}

