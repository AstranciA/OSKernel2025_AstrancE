use arceos_posix_api::UtsName;
use axtask::current;

use crate::SyscallResult;

pub fn sys_uname(buf: *mut u8) -> SyscallResult {
    if arceos_posix_api::sys_uname(buf as *mut UtsName) == 0 {
        SyscallResult::Success(0)
    } else {
        SyscallResult::Error(super::errno::LinuxError::EINVAL)
    }
}
