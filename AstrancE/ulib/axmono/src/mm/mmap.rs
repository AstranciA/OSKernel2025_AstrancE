use core::ffi::{c_int, c_void};

use alloc::vec::{self, Vec};
use arceos_posix_api::{ctypes, get_file_like, sys_lseek, sys_read};
use axfs::{
    api::read,
    fops::{File, OpenOptions},
};
use axmm::MmapIO;

pub struct MmapIOImpl {
    pub file_offset: ctypes::off_t,
    pub fd: c_int,
}

impl MmapIO for MmapIOImpl {
    fn read(&self, start: usize, buf: &mut [u8]) {
        let fd = self.fd;
        let start: ctypes::off_t = start.try_into().unwrap();
        let prev = sys_lseek(fd, 0, 1);
        let curr = sys_lseek(fd, start + self.file_offset, 0);
        sys_read(fd, buf.as_mut_ptr() as *mut c_void, buf.len());
        // recover cursor
        sys_lseek(fd, prev, 0);
    }

    fn write(&self, offset: usize, data: &[u8]) {
        todo!()
    }
}
