use arceos_posix_api::{self as api, ctypes};
use core::ffi::c_char;
use core::ffi::c_int;
use std::ffi::c_void;
pub fn ax_read(fd: usize,buf:&mut[u8]) -> Result<isize, isize> {
    let ret: isize;
    //检查读取的位置的权限安全性
    ret = api::sys_read(fd as i32, buf.as_mut_ptr() as *mut c_void, buf.len());
    if ret < 0 {
        Err(ret)
    }else {
        Ok(ret)
    }
}
pub fn ax_write(fd: usize,buf:&[u8]) -> Result<isize, isize> {
    let ret: isize;
    if fd == 1 || fd == 2{
        ret = api::sys_write(fd as i32, buf.as_ptr() as *mut c_void, buf.len());
    }else{
        todo!("检查写入的位置的权限安全性")
    }
    if ret < 0 {
        Err(ret)
    }else { 
        Ok(ret)
    }
}

