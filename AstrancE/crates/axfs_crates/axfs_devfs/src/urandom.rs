use core::{
    ffi::{c_int, c_long},
    sync::atomic::{AtomicU64, Ordering::*},
};

use axfs_vfs::{VfsNodeAttr, VfsNodeAttrX, VfsNodeOps, VfsNodePerm, VfsNodeType, VfsResult};
use rand::{rngs::SmallRng, RngCore, SeedableRng};

/// A zero device behaves like `/dev/zero`.
///
/// It always returns a chunk of `\0` bytes when read, and all writes are discarded.
pub struct URandomDev;

static SEED: AtomicU64 = AtomicU64::new(0xa2ce_a2ce);

/// 返回 32 位随机整数
pub fn rand() -> c_int {
    let new_seed = SEED.load(SeqCst).wrapping_mul(6364136223846793005) + 1;
    SEED.store(new_seed, SeqCst);
    (new_seed >> 33) as c_int
}

/// 返回 64 位随机整数
pub fn random() -> c_long {
    let new_seed = SEED.load(SeqCst).wrapping_mul(6364136223846793005) + 1;
    SEED.store(new_seed, SeqCst);
    new_seed as c_long
}

pub fn fill_random(buf: &mut [u8]) -> usize {
    let seed = random() as u64;
    let mut rng = SmallRng::seed_from_u64(seed);
    rng.fill_bytes(buf);
    buf.len()
}

impl VfsNodeOps for URandomDev {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new(
            0,
            VfsNodePerm::default_file(),
            VfsNodeType::CharDevice,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ))
    }

    fn get_attr_x(&self) -> VfsResult<VfsNodeAttrX> {
        Ok(VfsNodeAttrX::new(
            0,
            0,
            0,
            0,
            0,
            0,
            VfsNodePerm::default_file(),
            VfsNodeType::CharDevice,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ))
    }
    fn read_at(&self, _offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        fill_random(buf);
        Ok(buf.len())
    }

    axfs_vfs::impl_vfs_non_dir_default! {}
}
