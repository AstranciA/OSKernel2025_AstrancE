use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;

pub fn ax_socket(domain: c_int, socktype: c_int, protocol: c_int) -> Result<isize,isize>{
    let ret = api::sys_socket(domain, socktype, protocol) as isize;
    if ret < 0 {
        Err(ret)
    }else { 
        Ok(ret)
    }
}

