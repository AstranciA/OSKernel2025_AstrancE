//! Device filesystem used by [ArceOS](https://github.com/arceos-org/arceos).
//!
//! The implementation is based on [`axfs_vfs`].

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::collections::BTreeMap;
use spin::RwLock;

mod dir;
mod null;
mod zero;
// mod sda;
#[cfg(test)]
mod tests;

pub use self::dir::DirNode;
pub use self::null::NullDev;
pub use self::zero::ZeroDev;

use alloc::sync::Arc;
use axfs_vfs::{VfsNodeOps, VfsNodeRef, VfsOps, VfsResult};
use spin::once::Once;


/// A device filesystem that implements [`axfs_vfs::VfsOps`].
pub struct DeviceFileSystem {
    parent: Once<VfsNodeRef>,
    root: Arc<DirNode>,
}

impl DeviceFileSystem {
    /// Create a new instance.
    pub fn new() -> Self {
        Self {
            parent: Once::new(),
            root: DirNode::new(None),
        }
    }

    /// Create a subdirectory at the root directory.
    pub fn mkdir(&self, name: &'static str) -> Arc<DirNode> {
        self.root.mkdir(name)
    }

    /// Add a node to the root directory.
    ///
    /// The node must implement [`axfs_vfs::VfsNodeOps`], and be wrapped in [`Arc`].
    pub fn add(&self, name: &'static str, node: Arc<dyn VfsNodeOps>) { self.root.add(name, node);}
    
    // Register a device file by name (e.g., "vda2") and insert into dev_map.
    // pub fn register_device_by_name(&self, name: &'static str, major: u32, minor: u32, node: Arc<dyn VfsOps>) -> VfsResult {
    //     let dev_id = make_dev(major, minor);
    //     self.mkdir(name);
    //     self.root.add(name, node.clone());
    //     self.dev_map.write().insert(dev_id, node);
    //     Ok(())
    // }
    // pub fn get_device_by_id(&self, major: u32, minor: u32) -> Arc<dyn VfsOps> {
    //     let dev_t= make_dev(major, minor);
    //     self.dev_map.read().get(&dev_t).cloned()
    // }
}

impl VfsOps for DeviceFileSystem {
    fn mount(&self, _path: &str, mount_point: VfsNodeRef) -> VfsResult {
        if let Some(parent) = mount_point.parent() {
            self.root.set_parent(Some(self.parent.call_once(|| parent)));
        } else {
            self.root.set_parent(None);
        }
        Ok(())
    }

    fn root_dir(&self) -> VfsNodeRef {
        self.root.clone()
    }
}

impl Default for DeviceFileSystem {
    fn default() -> Self {
        Self::new()
    }
}


