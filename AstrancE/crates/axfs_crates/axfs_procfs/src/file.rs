use alloc::sync::Arc;
use axfs_vfs::{impl_vfs_non_dir_default, VfsNodeAttr, VfsNodeAttrX, VfsNodeOps, VfsResult};
use spin::RwLock;

/// 动态文件生成器类型
pub type ProcFileGenerator = dyn Fn(u64, &mut [u8]) -> VfsResult<usize> + Send + Sync;

/// 静态内容文件
pub struct ProcFile {
    content: Arc<[u8]>,
}

impl ProcFile {
    pub fn new(content: &[u8]) -> Self {
        Self {
            content: Arc::from(content),
        }
    }
}

impl VfsNodeOps for ProcFile {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_file(self.content.len() as u64, 0))
    }

    fn get_attr_x(&self) -> VfsResult<axfs_vfs::VfsNodeAttrX> {
        Ok(VfsNodeAttrX::new_file(self.content.len() as u64, 0))
    }


    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let start = offset as usize;
        if start >= self.content.len() {
            return Ok(0);
        }
        let end = (start + buf.len()).min(self.content.len());
        buf[..end - start].copy_from_slice(&self.content[start..end]);
        Ok(end - start)
    }

    impl_vfs_non_dir_default! {}
}

/// 动态生成内容的文件
pub struct ProcDynamicFile {
    generator: RwLock<Arc<ProcFileGenerator>>,
}

impl ProcDynamicFile {
    pub fn new(generator: Arc<ProcFileGenerator>) -> Self {
        Self {
            generator: RwLock::new(generator),
        }
    }

    pub fn update_generator(&self, generator: Arc<ProcFileGenerator>) {
        *self.generator.write() = generator;
    }
}

impl VfsNodeOps for ProcDynamicFile {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_file(0, 0)) // 动态文件大小未知
    }

    fn get_attr_x(&self) -> VfsResult<VfsNodeAttrX> {
        Ok(VfsNodeAttrX::new_file(0, 0)) // 动态文件大小未知
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        (self.generator.read())(offset, buf)
    }

    impl_vfs_non_dir_default! {}
}
