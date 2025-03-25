use arceos_posix_api::{self as api, ctypes};
use crate::SyscallResult;

pub fn ae_clock_gettime(clk: ctypes::clockid_t, ts: *mut ctypes::timespec) -> SyscallResult {
    let ret = unsafe { api::sys_clock_gettime(clk, ts) } as isize;
    if ret != 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    }else {
        SyscallResult::Success(ret)
    }
}

pub fn ae_nanosleep(req: *const ctypes::timespec, rem: *mut ctypes::timespec) -> SyscallResult {
    let ret = unsafe { api::sys_nanosleep(req, rem) } as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    }else {
        SyscallResult::Success(ret)
    }
}

pub fn ae_get_time_of_day(ts: *mut ctypes::timeval) -> SyscallResult {
    let ret = unsafe { api::sys_get_time_of_day(ts) } as isize;
    if ret < 0 {
        SyscallResult::Error((-ret).try_into().unwrap())
    }else {
        SyscallResult::Success(ret)
    }
}