use alloc::sync::Arc;
use axerrno::{AxError, AxResult, ax_err};
use axhal::{
    mem::MemoryAddr,
    paging::{MappingFlags, PageSize},
};
use bitflags::bitflags;
use memory_addr::{VirtAddr, addr_range, va};
use memory_set::MemoryArea;

use crate::{
    AddrSpace, Backend,
    backend::{VmAreaType, alloc::alloc_frame},
    mapping_err_to_ax_err,
};

const MMAP_END: VirtAddr = va!(0x4000_0000);

// From phoenix
bitflags! {
    // Defined in <bits/mman-linux.h>
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapFlags: usize {
        // Sharing types (must choose one and only one of these).
        /// Share changes.
        const MAP_SHARED = 0x01;
        /// Changes are private.
        const MAP_PRIVATE = 0x02;
        /// Share changes and validate
        const MAP_SHARED_VALIDATE = 0x03;
        const MAP_TYPE_MASK = 0x03;

        // Other flags
        /// Interpret addr exactly.
        const MAP_FIXED = 0x10;
        /// Don't use a file.
        const MAP_ANONYMOUS = 0x20;
        /// Don't check for reservations.
        const MAP_NORESERVE = 0x04000;
    }
}

bitflags! {
    // Defined in <bits/mman-linux.h>
    // NOTE: Zero bit flag is discouraged. See https://docs.rs/bitflags/latest/bitflags/#zero-bit-flags
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapPerm: usize {
        /// Page can be read.
        const PROT_READ = 0x1;
        /// Page can be written.
        const PROT_WRITE = 0x2;
        /// Page can be executed.
        const PROT_EXEC = 0x4;
    }
}

impl From<MmapPerm> for MappingFlags {
    fn from(prot: MmapPerm) -> Self {
        let mut ret = Self::USER;
        if prot.contains(MmapPerm::PROT_READ) {
            ret |= Self::READ;
        }
        if prot.contains(MmapPerm::PROT_WRITE) {
            ret |= Self::WRITE;
        }
        if prot.contains(MmapPerm::PROT_EXEC) {
            ret |= Self::EXECUTE;
        }
        ret
    }
}

pub trait MmapIO: Send + Sync {
    fn read(&self, offset: usize, buf: &mut [u8]);
    fn write(&self, offset: usize, data: &[u8]);
    fn flags(&self) -> MmapFlags;
}

impl AddrSpace {
    pub fn mmap(
        &mut self,
        start: VirtAddr,
        size: usize,
        perm: MmapPerm,
        flags: MmapFlags,
        /*
         *file: Arc<MmapFile>,
         *offset: usize,
         */
        mmap_io: Arc<dyn MmapIO>,
        populate: bool,
    ) -> AxResult<VirtAddr> {
        let start = if flags.contains(MmapFlags::MAP_FIXED) {
            self.validate_region(start, size)
                .expect("Invalid mmap address or size");
            start
        } else {
            #[cfg(feature = "heap")]
            {
                // should below heap
                let heap_start = self
                    .heap
                    .as_ref()
                    .map(|h| h.base())
                    .unwrap_or(MMAP_END)
                    .into();
                self.find_free_area(
                    0x2000_0000.into(),
                    size,
                    addr_range!(self.base().as_usize()..heap_start),
                )
                .expect("Cannot find free area for mmap")
            }
            #[cfg(not(feature = "heap"))]
            {
                self.find_free_area(
                    0x2000_0000.into(),
                    size,
                    addr_range!(self.base().as_usize()..MMAP_END.into()),
                )
                .expect("Cannot find free area for mmap")
            }
        };

        debug!("mmap at: [{:#x}, {:#x})", start, start + size);

        let mut map_flags: MappingFlags = perm.into();
        map_flags = map_flags | MappingFlags::DEVICE;

        // #[cfg(feature = "COW")]
        // // TODO: Why check flags here?
        // if flags.contains(MmapFlags::MAP_PRIVATE) {
        //     map_flags = (map_flags - MappingFlags::WRITE) | MappingFlags::COW;
        // }

        let area = MemoryArea::new_mmap(
            start,
            size.align_up_4k(),
            None,
            map_flags,
            Backend::new(populate, VmAreaType::Mmap(mmap_io.clone())),
        );

        if populate {
            todo!("populate from file");
        }
        self.areas
            .map(area, &mut self.pt, false, Some(map_flags))
            .map_err(mapping_err_to_ax_err)?;
        Ok(start)
    }

    pub fn map_mmap(
        &mut self,
        mmio: Arc<dyn MmapIO>,
        vaddr: VirtAddr,
        size: PageSize,
        orig_flags: MappingFlags,
    ) -> AxResult {
        debug_assert!(vaddr.is_aligned_4k());
        let flags = orig_flags | MappingFlags::READ | MappingFlags::USER;
        // #[cfg(feature = "COW")]
        // MappingFlags::mark_cow(&mut flags);

        if let Some(frame) = alloc_frame(true) {
            if let Err(_) = self.page_table()
                // READ | WRITE for copying data from file later.
                .map(
                    vaddr,
                    frame.pa,
                    size,
                    MappingFlags::READ | MappingFlags::WRITE,
                )
                .map(|tlb| tlb.flush())
            {
                return ax_err!(BadAddress);
            }

            let area = match self.areas.find_mut(vaddr) {
                Some(area) => area,
                None => return ax_err!(BadAddress),
            };
            debug!(
                "pa: {:?}, start: {:?}, end: {:?}, flags: {:?}",
                frame.pa,
                area.start(),
                area.end(),
                flags
            );
            area.insert_frame(vaddr, frame.clone());

            let dst = unsafe { core::slice::from_raw_parts_mut(vaddr.as_mut_ptr(), size.into()) };
            mmio.read(vaddr - area.start(), dst);

            self.page_table()
                // WRITE for copying data from file later.
                .remap(vaddr, frame.pa, flags)
                .map(|(_, tlb)| tlb.flush())
                .unwrap();

            Ok(())
        } else {
            Err(AxError::NoMemory)
        }
    }
    pub fn munmap(&mut self, start: VirtAddr, size: usize) -> AxResult {
        // TODO: is it correct?
        let size = size.align_up_4k();
        let end = start + size;
        let area = match self.areas.find_mut(start) {
            Some(area) => area,
            None => return Ok(()),
        };
        if area.end() < end {
            error!(
                "[{:#x}, {:#x}) out of range [{:#x}, {:#x})",
                start,
                end,
                area.start(),
                area.end()
            );
            return ax_err!(BadAddress, "munmap end out of range");
        }

        let _mmap_io = if let Backend::Alloc { va_type, populate } = area.backend() {
            if let VmAreaType::Mmap(mmap_io) = va_type {
                Ok(mmap_io)
            } else {
                Err(AxError::InvalidInput)
            }
        } else {
            Err(AxError::InvalidInput)
        }?;

        area.unmap_frames(start, size, &mut self.pt).unwrap();
        let is_empty = area.frames_len() == 0;
        let area_start = area.start();
        if is_empty {
            self.unmap_area(area_start);
        }
        /*
         *        if (start == area.start()) {
         *            // [unmap_area | back]
         *            if let Some(back) = area.split(end) {
         *                self.areas.insert(back);
         *            };
         *            area.unmap_area(&mut self.pt);
         *        } else {
         *            // split area into three parts
         *            // [front | unmap_area | back]
         *            let mut unmap_area = match area.split(start) {
         *                Some(area) => area,
         *                None => return Ok(()),
         *            };
         *
         *            if let Some(back) = unmap_area.split(end) {
         *                self.areas.insert(back);
         *            };
         *        };
         */

        Ok(())
    }
}
