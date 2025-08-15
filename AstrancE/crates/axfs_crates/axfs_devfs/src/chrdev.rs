use alloc::sync::Arc;
use spin::Mutex;
use axfs_vfs::{VfsNodeOps, FileAttr, FileType, VfsResult};

pub struct CharDeviceNode {
    name: &'static str,
    buffer: Mutex<Vec<u8>>,
}

impl CharDeviceNode {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            buffer: Mutex::new(Vec::new()),
        }
    }
}

impl VfsNodeOps for CharDeviceNode {
    fn get_attr(&self) -> VfsResult<FileAttr> {
        Ok(FileAttr {
            file_type: FileType::CharDevice,
            size: self.buffer.lock().len() as u64,
        })
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        let data = self.buffer.lock();
        let len = buf.len().min(data.len().saturating_sub(offset));
        buf[..len].copy_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize> {
        let mut data = self.buffer.lock();
        if offset > data.len() {
            data.resize(offset, 0);
        }
        if offset + buf.len() > data.len() {
            data.resize(offset + buf.len(), 0);
        }
        data[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(buf.len())
    }

    fn name(&self) -> &str {
        self.name
    }

    // 你可以根据需要补充 open、release、truncate 等方法
}
