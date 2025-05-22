// use core::any::Any;
// use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
// use axfs_vfs::{VfsError, VfsResult};
// use alloc::sync::{Arc, Weak};
// use spin::Mutex;
// use axdriver::prelude::*;
// 
// pub struct Blkdev {
//     dev: AxBlockDevice,
//     dev_t: (u32, u32),
//     mount_n: usize,
// }
// 
// impl Blkdev {
//     pub fn new(dev: AxBlockDevice, major: u32, minor: u32) -> Self {
//         Self{
//             dev: dev,
//             dev_t: (major, minor),
//             mount_n: 0,
//         }
//     }
//     pub fn inc_mount_n(&mut self) {
//         self.mount_n += 1;
//     }
//     pub fn get_dev(self) -> AxBlockDevice {
//        self.dev.clone()
//     }
//     pub fn dev_t(&self) -> (u32, u32) {self.dev_t}
//     pub fn mount_n(&self) ->usize {self.mount_n}
// 
// }
// 
// impl VfsNodeOps for Blkdev {
//     fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
//         //TODO:dev num
//         Ok(VfsNodeAttr::new_file(4096, 1))
//     }
// 
//     fn remove(&self, _path: &str) -> VfsResult {
//         todo!()
//     }
// 
//     fn as_any(&self) -> &dyn Any {
//         todo!()
//     }
// }
