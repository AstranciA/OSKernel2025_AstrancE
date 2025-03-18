use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;



pub fn ax_openat(dirfd: c_int,
                  filename: *const c_char,
                  flags: c_int,
                  mode: ctypes::mode_t,
) -> isize{
    //检查位置安全性
    api::sys_openat(dirfd,filename,flags,mode) as isize
}