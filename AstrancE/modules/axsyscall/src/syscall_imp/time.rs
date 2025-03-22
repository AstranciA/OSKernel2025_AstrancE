use arceos_posix_api::{self as api, ctypes};


pub fn ax_clock_gettime(clk: ctypes::clockid_t, ts: *mut ctypes::timespec) -> Result<isize, isize> {
    let ret = unsafe { api::sys_clock_gettime(clk, ts) } as isize;
    if ret != 0 {
        Err(ret)
    }else { 
        Ok(ret)
    }
}

pub fn ax_nanosleep(req: *const ctypes::timespec, rem: *mut ctypes::timespec) -> Result<isize, isize> {
    let ret = unsafe { api::sys_nanosleep(req, rem) } as isize;
    if ret < 0 {
        Err(ret)
    }else{
        Ok(ret)
    }
}

pub fn ax_get_time_of_day(ts: *mut ctypes::timeval) -> Result<isize, isize> {
    let ret = unsafe { api::sys_get_time_of_day(ts) } as isize;
    if ret < 0 {
        Err(ret)
    }else { 
        Ok(ret)
    }
}