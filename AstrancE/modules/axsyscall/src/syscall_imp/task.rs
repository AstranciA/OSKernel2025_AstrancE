use arceos_posix_api::{self as api, ctypes::mode_t};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;
use arceos_posix_api::ctypes;

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