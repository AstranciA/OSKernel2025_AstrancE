//! [ArceOS](https://github.com/arceos-org/arceos) filesystem module.
//!
//! It provides unified filesystem operations for various filesystems.
//!
//! # Cargo Features
//!
//! - `fatfs`: Use [FAT] as the main filesystem and mount it on `/`. This feature
//!    is **enabled** by default.
//! - `devfs`: Mount [`axfs_devfs::DeviceFileSystem`] on `/dev`. This feature is
//!    **enabled** by default.
//! - `ramfs`: Mount [`axfs_ramfs::RamFileSystem`] on `/tmp`. This feature is
//!    **enabled** by default.
//! - `myfs`: Allow users to define their custom filesystems to override the
//!    default. In this case, [`MyFileSystemIf`] is required to be implemented
//!    to create and initialize other filesystems. This feature is **disabled** by
//!    by default, but it will override other filesystem selection features if
//!    both are enabled.
//!
//! [FAT]: https://en.wikipedia.org/wiki/File_Allocation_Table
//! [`MyFileSystemIf`]: fops::MyFileSystemIf

#![cfg_attr(all(not(test), not(doc)), no_std)]
#![feature(doc_auto_cfg)]

#[macro_use]
extern crate log;
extern crate alloc;
use alloc::sync::Arc;
pub mod api;
mod blkdev;
mod dev;
pub mod fops;
mod fs;
mod mounts;
mod root;
pub use root::{CURRENT_DIR, CURRENT_DIR_PATH};

use crate::dev::Disk;
use axdriver::{AxDeviceContainer, prelude::*};
use axfs_vfs::{VfsNodeOps, VfsOps};

/// Initializes filesystems by block devices.
pub fn init_filesystems(mut blk_devs: AxDeviceContainer<AxBlockDevice>) {
    info!("Initialize filesystems...");
    let dev = blk_devs.take_one().expect("No block device found!");
    info!("  use block device 0: {:?}", dev.device_name());
    // root::init_rootfs(self::dev::Disk::new(dev));
    // let disk = Disk::new(dev,1,0);
    // let devfs = mounts::devfs();
    // devfs.add("ram1",Arc::new(disk.clone()));
    // let node = devfs.root_dir().lookup("ram1").unwrap();
    //let disk = Disk::new(node.get_dev(),1,0);
    root::init_rootfs(Disk::new(dev, 1, 0));
    info!("Initialize device filesystems...");
}
