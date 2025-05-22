use axdriver::prelude::*;
use axfs_vfs::{VfsNodeAttr, VfsNodeOps, VfsResult};
const BLOCK_SIZE: usize = 512;
/// A disk device with a cursor.
pub struct Disk {
    block_id: u64,
    offset: usize,
    dev: AxBlockDevice, //Ramdisk
    dev_t: (u8, u8),
}

impl Disk {
    /// Create a new disk.
    pub fn new(dev:AxBlockDevice, major: u8, minor: u8) -> Self {
        assert_eq!(BLOCK_SIZE, dev.block_size());
        Self {
            block_id: 0,
            offset: 0,
            dev,
            dev_t: (major, minor),
        }
    }

    /// Get the size of the disk.
    pub fn size(&self) -> u64 {
        self.dev.num_blocks() * BLOCK_SIZE as u64
    }
    
    // ///Clone disk for filesystem
    // pub fn get_dev(&self) -> AxBlockDevice {
    //     let dev = self.dev.deep_clone().expect("Clone failed");
    // 
    //     // 尝试从 trait object 恢复为具体类型
    //     dev.as_any()
    //         .downcast_ref::<AxBlockDevice>()
    //         .expect("Not a RamDisk")
    //         .clone()
    // }
    ///Get dev index
    pub fn dev_t(&self) -> (u8, u8) {self.dev_t}

    /// Get the position of the cursor.
    pub fn position(&self) -> u64 {
        self.block_id * BLOCK_SIZE as u64 + self.offset as u64
    }

    /// Set the position of the cursor.
    pub fn set_position(&mut self, pos: u64) {
        self.block_id = pos / BLOCK_SIZE as u64;
        self.offset = pos as usize % BLOCK_SIZE;
    }

    /// Read within one block, returns the number of bytes read.
    pub fn read_one(&mut self, buf: &mut [u8]) -> DevResult<usize> {
        let read_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            // whole block
            let mut data = [0u8; BLOCK_SIZE];
            self.dev.read_block(self.block_id, &mut data)?;
            buf[0..BLOCK_SIZE].copy_from_slice(&data);
            // self.dev
            //     .read_block(self.block_id, &mut buf[0..BLOCK_SIZE])?;
            self.block_id += 1;
            BLOCK_SIZE
        } else {
            // partial block
            let mut data = [0u8; BLOCK_SIZE];
            let start = self.offset;
            let count = buf.len().min(BLOCK_SIZE - self.offset);

            self.dev.read_block(self.block_id, &mut data)?;
            buf[..count].copy_from_slice(&data[start..start + count]);

            self.offset += count;
            if self.offset >= BLOCK_SIZE {
                self.block_id += 1;
                self.offset -= BLOCK_SIZE;
            }
            count
        };
        Ok(read_size)
    }

    /// Write within one block, returns the number of bytes written.
    pub fn write_one(&mut self, buf: &[u8]) -> DevResult<usize> {
        let write_size = if self.offset == 0 && buf.len() >= BLOCK_SIZE {
            // whole block
            self.dev.write_block(self.block_id, &buf[0..BLOCK_SIZE])?;
            self.block_id += 1;
            BLOCK_SIZE
        } else {
            // partial block
            let mut data = [0u8; BLOCK_SIZE];
            let start = self.offset;
            let count = buf.len().min(BLOCK_SIZE - self.offset);

            self.dev.read_block(self.block_id, &mut data)?;
            data[start..start + count].copy_from_slice(&buf[..count]);
            self.dev.write_block(self.block_id, &data)?;

            self.offset += count;
            if self.offset >= BLOCK_SIZE {
                self.block_id += 1;
                self.offset -= BLOCK_SIZE;
            }
            count
        };
        Ok(write_size)
    }

    /// Read a single block starting from the specified offset.
    #[allow(unused)]
    pub fn read_offset(&mut self, offset: usize) -> [u8; BLOCK_SIZE] {
        let block_id = offset / BLOCK_SIZE;
        let mut block_data = [0u8; BLOCK_SIZE];
        self.dev
            .read_block(block_id as u64, &mut block_data)
            .unwrap();
        block_data
    }

    /// Write single block starting from the specified offset.
    #[allow(unused)]
    pub fn write_offset(&mut self, offset: usize, buf: &[u8]) -> DevResult<usize> {
        assert!(
            buf.len() == BLOCK_SIZE,
            "Buffer length must be equal to BLOCK_SIZE"
        );
        assert!(offset % BLOCK_SIZE == 0);
        let block_id = offset / BLOCK_SIZE;
        self.dev.write_block(block_id as u64, buf).unwrap();
        Ok(buf.len())
    }
}

// impl VfsNodeOps for Disk{
//     fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
//         //TODO:dev num
//         Ok(VfsNodeAttr::new_file(4096, 1))
//     }
//     fn remove(&self, _path: &str) -> VfsResult {
//         todo!()
//     }
// 
// }
// 
// impl Clone for Disk {
//     fn clone(&self) -> Self {
//         Disk {
//             block_id: self.block_id,
//             offset: self.offset,
//             dev: self.get_dev(), // 确保 dev 实现了 Clone
//             dev_t: self.dev_t,
//         }
//     }
// }

unsafe impl Send for Disk {}
unsafe impl Sync for Disk {}
