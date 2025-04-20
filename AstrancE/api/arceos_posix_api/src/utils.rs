#![allow(dead_code)]
#![allow(unused_macros)]

use axerrno::{LinuxError, LinuxResult};
use core::ffi::{CStr, c_char};

/// Convert a C string to a Rust string
pub fn char_ptr_to_str<'a>(str: *const c_char) -> LinuxResult<&'a str> {
    if str.is_null() {
        Err(LinuxError::EFAULT)
    } else {
        let str = str as *const _;
        unsafe { CStr::from_ptr(str) }
            .to_str()
            .map_err(|_| LinuxError::EINVAL)
    }
}

/// Convert a Rust string to a C string
pub unsafe fn str_to_cstr(s: &str, buf: *mut c_char) {
    let len = s.len();
    let dst = unsafe { core::slice::from_raw_parts_mut(buf, len + 1) };
    let ss = unsafe {
        core::slice::from_raw_parts(
            s.as_ptr() as *const c_char,
            s.len()
        )
    };
    dst[..len].copy_from_slice(ss);
    dst[len] = (b'\0') as c_char;
    error!("{:p},{:?}", buf,dst);
}

pub fn check_null_ptr<T>(ptr: *const T) -> LinuxResult {
    if ptr.is_null() {
        Err(LinuxError::EFAULT)
    } else {
        Ok(())
    }
}

pub fn check_null_mut_ptr<T>(ptr: *mut T) -> LinuxResult {
    if ptr.is_null() {
        Err(LinuxError::EFAULT)
    } else {
        Ok(())
    }
}
macro_rules! syscall_body {
    ($fn: ident, $($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(_) | Err(axerrno::LinuxError::EAGAIN) => debug!(concat!(stringify!($fn), " => {:?}"),  res),
            Err(_) => info!(concat!(stringify!($fn), " => {:?}"), res),
        }
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}
macro_rules! syscall_body_no_debug {
    ($($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}
