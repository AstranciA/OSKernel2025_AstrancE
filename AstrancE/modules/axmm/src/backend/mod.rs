//! Memory mapping backends.

use ::alloc::sync::Arc;
use axhal::paging::{MappingFlags, PageTable};
use frame::{FrameTrackerImpl, FrameTrackerMap};
use memory_addr::{FrameTracker, Page, VirtAddr};
use memory_set::{MappingBackend, MemorySet};

use crate::{AddrSpace, aspace::mmap::MmapIO};

pub(super) mod alloc;
pub mod frame;
mod linear;

/// A unified enum type for different memory mapping backends.
///
/// Currently, two backends are implemented:
///
/// - **Linear**: used for linear mappings. The target physical frames are
///   contiguous and their addresses should be known when creating the mapping.
/// - **Allocation**: used in general, or for lazy mappings. The target physical
///   frames are obtained from the global allocator.
#[derive(Clone)]
pub enum Backend {
    /// Linear mapping backend.
    ///
    /// The offset between the virtual address and the physical address is
    /// constant, which is specified by `pa_va_offset`. For example, the virtual
    /// address `vaddr` is mapped to the physical address `vaddr - pa_va_offset`.
    Linear {
        /// `vaddr - paddr`.
        pa_va_offset: usize,
    },
    /// Allocation mapping backend.
    ///
    /// If `populate` is `true`, all physical frames are allocated when the
    /// mapping is created, and no page faults are triggered during the memory
    /// access. Otherwise, the physical frames are allocated on demand (by
    /// handling page faults).
    Alloc {
        va_type: VmAreaType,
        /// Whether to populate the physical frames when creating the mapping.
        populate: bool,
    },
}

impl MappingBackend for Backend {
    type Addr = VirtAddr;
    type Flags = MappingFlags;
    type PageTable = PageTable;
    type FrameTrackerImpl = FrameTrackerImpl;
    type FrameTrackerRef = Arc<FrameTrackerImpl>;

    fn unmap(&self, start: VirtAddr, size: usize, pt: &mut PageTable) -> bool {
        match self {
            Self::Linear { pa_va_offset } => Self::unmap_linear(start, size, pt, *pa_va_offset),
            Self::Alloc { populate, va_type } => {
                Self::unmap_alloc(start, size, pt, va_type.clone(), *populate)
            }
        }
    }

    fn protect(
        &self,
        start: Self::Addr,
        size: usize,
        new_flags: Self::Flags,
        page_table: &mut Self::PageTable,
    ) -> bool {
        page_table
            .protect_region(start, size, new_flags, true)
            .map(|tlb| tlb.ignore())
            .is_ok()
    }

    fn map(
        &self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
    ) -> Result<FrameTrackerMap, ()> {
        let frame_refs = match self {
            Self::Linear { pa_va_offset } => {
                Self::map_linear(start, size, flags, pt, *pa_va_offset)
            }
            Self::Alloc { populate, va_type } => {
                Self::map_alloc(start, size, flags, pt, va_type.clone(), *populate)
            }
        };
        frame_refs
    }
}

impl Backend {
    pub(crate) fn handle_page_fault(
        &self,
        vaddr: VirtAddr,
        orig_flags: MappingFlags,
        aspace: &mut AddrSpace,
        //areas: &mut MemorySet<Backend>,
        //page_table: &mut PageTable,
        //page_table: &mut PageTable,
    ) -> bool {
        match self {
            Self::Linear { .. } => false, // Linear mappings should not trigger page faults.
            Self::Alloc { populate, va_type } => {
                Self::handle_page_fault_alloc(vaddr, va_type.clone(), orig_flags, aspace, *populate)
            }
        }
    }
}

#[derive(Clone)]
pub enum VmAreaType {
    Normal,
    Elf,
    Heap,
    Stack,
    Mmap(Arc<dyn MmapIO>),
}
