use arceos_posix_api::{self as api, ctypes::mode_t};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;
use arceos_posix_api::ctypes;
pub fn sys_read(fd: usize,buf:&mut[u8]) -> isize {
    todo!("检查读取的位置的权限安全性");
    api::sys_read(fd as i32,buf.as_mut_ptr() as *mut c_void,buf.len())
}
pub fn sys_write(fd: usize,buf:&[u8]) -> isize {
    if fd == 1 || fd == 2{
        api::sys_write(fd as i32,buf.as_ptr() as *mut c_void,buf.len())
    }else{
        todo!("检查写入的位置的权限安全性")
    }
}

