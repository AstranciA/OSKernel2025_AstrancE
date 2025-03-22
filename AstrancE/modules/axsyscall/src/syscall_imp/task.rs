use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_void;

pub fn ax_getpid() -> Result<isize, isize> {
    let ret = api::sys_getpid() as isize;
    Ok(ret)
}

pub fn ax_exit(code:c_int) -> Result<isize, isize>{
    api::sys_exit(code);
    Ok(2)
}

pub fn ax_yield() -> Result<isize, isize>{
    Ok(api::sys_sched_yield() as isize)
}