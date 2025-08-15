//! RAM filesystem used by [ArceOS](https://github.com/arceos-org/arceos).
//!
//! The implementation is based on [`axfs_vfs`].

#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod dir;
mod file;

#[cfg(test)]
mod tests;

pub use dir::*;
pub use file::*;
use alloc::sync::Arc;
use axfs_vfs::{VfsNodeRef, VfsOps, VfsResult};
use spin::once::Once;

/// A RAM filesystem that implements [`axfs_vfs::VfsOps`].
pub struct ProcFileSystem {
    parent: Once<VfsNodeRef>,
    root: Arc<ProcDir>,
}

impl ProcFileSystem {
    /// Create a new instance.
    pub fn new() -> Self {
        Self {
            parent: Once::new(),
            root: ProcDir::new(None),
        }
    }

    /// Returns the root directory node in [`Arc<DirNode>`](DirNode).
    pub fn root_dir_node(&self) -> Arc<ProcDir> {
        self.root.clone()
    }
}

impl VfsOps for ProcFileSystem {
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

impl Default for ProcFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
