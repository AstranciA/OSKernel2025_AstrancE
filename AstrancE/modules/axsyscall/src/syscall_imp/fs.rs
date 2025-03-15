use arceos_posix_api::{self as api, ctypes::mode_t};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;
use arceos_posix_api::ctypes;



pub fn sys_openat(dirfd: c_int,
                  filename: *const c_char,
                  flags: c_int,
                  mode: ctypes::mode_t,
) -> isize{
    //检查位置安全性
    api::sys_openat(dirfd,filename,flags,mode) as isize
}