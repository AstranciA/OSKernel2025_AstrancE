use core::fmt::Debug;

use memory_addr::{FrameTracker, PAGE_SIZE_4K, PhysAddr, VirtAddr};
use memory_set::MappingBackend;

use crate::backend::alloc::dealloc_frame;

/* TODO:
 *enum FrameTracker {
 *    4K()
 *    2M()
 *    1G()
 *}
 */
#[derive(Clone)]
pub struct FrameTrackerImpl {
    pub pa: PhysAddr,

    tracking: bool,
}

/// Implement of FrameTracker.
/// The methods should not be used in user momery space or pa will be invalid
impl FrameTracker for FrameTrackerImpl {
    const PAGE_SIZE: usize = PAGE_SIZE_4K;

    fn new(pa: PhysAddr) -> Self {
        //debug!("FrameTrackerImpl::new({:#x})", pa);
        Self { pa, tracking: true }
    }

    fn no_tracking(pa: PhysAddr) -> Self {
        //debug!("FrameTrackerImpl::new({:#x})", pa);
        Self {
            pa,
            tracking: false,
        }
    }

    /// Don't use this method. Frame should be allocated by Backend::map
    fn alloc_frame() -> Self {
        //Backend::Alloc { populate: true }.map(start, size, flags, page_table)
        panic!("frame should be allocated by Backend::map")
    }

    fn start(&self) -> PhysAddr {
        self.pa
    }

    fn dealloc_frame(&mut self) {
        if self.tracking {
            trace!("Dealloc frame {:?} by FrameTrackerImpl::drop", self.pa);
            dealloc_frame(self.pa);
        }
    }
}

impl Debug for FrameTrackerImpl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Frame {{ {:#x} }}", self.pa)
    }
}

impl Drop for FrameTrackerImpl {
    fn drop(&mut self) {
        self.dealloc_frame();
    }
}

pub type FrameTrackerRef = alloc::sync::Arc<FrameTrackerImpl>;
pub type FrameTrackerWeak = alloc::sync::Weak<FrameTrackerImpl>;
pub type FrameTrackerMap = alloc::collections::BTreeMap<VirtAddr, FrameTrackerRef>;
