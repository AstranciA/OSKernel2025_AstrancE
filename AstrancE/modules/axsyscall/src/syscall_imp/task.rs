use arceos_posix_api::{self as api, ctypes::mode_t};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;
use arceos_posix_api::ctypes;

pub fn getpid() -> Result<isize, isize> {
    let ret = api::sys_getpid() as isize;
    Ok(ret)
}

pub fn exit(code: c_int) -> Result<isize, isize>{
    api::sys_exit(code);
    Ok(2)
}