pub mod mmap;
use alloc::string::String;
use axerrno::AxResult;
use axfs::api::write;
use axhal::trap::{PAGE_FAULT, register_trap_handler};
use axhal::{
    mem::{VirtAddr, virt_to_phys},
    paging::MappingFlags,
};
use axmm::AddrSpace;
use axtask::{TaskExtRef, current};
use xmas_elf::ElfFile;

use crate::{
    copy_from_kernel,
    elf::{ELFInfo, OwnedElfFile},
};

pub fn new_user_aspace_empty() -> AxResult<AddrSpace> {
    AddrSpace::new_empty(
        VirtAddr::from_usize(axconfig::plat::USER_SPACE_BASE),
        axconfig::plat::USER_SPACE_SIZE,
    )
}

/// load app to memory
/// # Returns
/// - The first return value is the entry point of the user app.
/// - The second return value is the top of the user stack.
/// - Third: thread pointer.
/// - The last return value is the address space of the user app.
pub fn load_elf_to_mem(
    elf_file: OwnedElfFile,
    args: Option<&[String]>,
    envs: Option<&[String]>,
) -> AxResult<(VirtAddr, VirtAddr, Option<VirtAddr>, AddrSpace)> {
    let mut uspace = new_user_aspace_empty()
        .and_then(|mut it| {
            copy_from_kernel(&mut it)?;
            Ok(it)
        })
        .expect("Failed ot create user address space");
    let elf_info = ELFInfo::new(elf_file, uspace.base());
    let (entry, ustack_pointer, tp) = map_elf_sections(elf_info, &mut uspace, args, envs)?;
    Ok((entry, ustack_pointer, tp, uspace))
}

/**
init stack with args, envs, argc and argv.
Returns:
- The entry point of the user app.
- The initial stack pointer
- The thread pointer
[argc | argv | env ]
^
sp
*/
pub fn map_elf_sections(
    mut elf_info: ELFInfo,
    uspace: &mut AddrSpace,
    args: Option<&[String]>,
    envs: Option<&[String]>,
) -> Result<(VirtAddr, VirtAddr, Option<VirtAddr>), axerrno::AxError> {
    let mut tp: Option<VirtAddr> = None;
    for segement in elf_info.segments.iter() {
        match segement.type_ {
            xmas_elf::program::Type::Load => {
                let segement_end = segement.start_va + segement.size;
                trace!(
                    "Mapping ELF segment: [{:#x?}, {:#x?}) -> [{:#x?}, {:#x?}), flags: {:#x?}",
                    segement.start_va + segement.offset,
                    segement_end + segement.offset,
                    segement.start_va,
                    segement_end,
                    segement.flags
                );

                uspace.map_alloc(segement.start_va, segement.size, segement.flags, true)?;

                if segement.data.is_empty() {
                    continue;
                }
                //uspace.populate_area(segement.start_va, segement.size);

                uspace.write(segement.start_va + segement.offset, segement.data)?;
                uspace.fill_zero(
                    segement.start_va + segement.offset + segement.data.len(),
                    segement.size - segement.data.len(),
                );
                // TDOO: flush the I-cache
            }
            xmas_elf::program::Type::Tls => {
                tp = Some(segement.start_va + segement.offset);
            }
            _ => {
                panic!("Unsupported segment type");
            }
        }
    }

    // heap
    #[cfg(feature = "heap")]
    uspace.init_heap(
        axconfig::plat::USER_HEAP_BASE.into(),
        axconfig::plat::USER_HEAP_SIZE,
    );

    // The user stack is divided into two parts:
    // `ustack_start` -> `ustack_pointer`: It is the stack space that users actually read and write.
    // `ustack_pointer` -> `ustack_end`: It is the space that contains the arguments, environment variables and auxv passed to the app.
    //  When the app starts running, the stack pointer points to `ustack_pointer`.
    let ustack_end = VirtAddr::from_usize(axconfig::plat::USER_STACK_TOP);
    let ustack_size = axconfig::plat::USER_STACK_SIZE;
    let ustack_start = ustack_end - ustack_size;
    debug!(
        "Mapping user stack: {:#x?}..{:#x?}",
        ustack_start, ustack_end
    );
    // FIXME: Add more arguments and environment variables
    let stack_data = kernel_elf_parser::app_stack_region(
        args.unwrap_or_default(),
        envs.unwrap_or_default(),
        elf_info.auxv.as_mut_slice(),
        ustack_start,
        ustack_size,
    );
    uspace.map_alloc(
        ustack_start,
        ustack_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
    )?;

    uspace.write(ustack_end - stack_data.len(), stack_data.as_slice())?;
    let sp_offset = stack_data.len();

    // map trapoline
    map_trapoline(uspace);

    Ok((elf_info.entry, ustack_end - sp_offset, tp))
}

unsafe extern "C" {
    fn _strampoline();
    fn _etrampoline();
}

pub(crate) fn map_trapoline(aspace: &mut AddrSpace) {
    aspace
        .map_linear(
            axconfig::plat::USER_TRAMPOLINE_BASE.into(),
            virt_to_phys((_strampoline as usize).into()),
            _etrampoline as usize - _strampoline as usize,
            MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
        )
        .unwrap();
}

pub(crate) unsafe fn trampoline_vaddr(fn_: usize) -> usize {
    assert!(
        fn_ >= _strampoline as usize && fn_ < _etrampoline as usize,
        "Invalid trampoline address"
    );

    fn_ - _strampoline as usize + axconfig::plat::USER_TRAMPOLINE_BASE
}

#[percpu::def_percpu]
static mut ACCESSING_USER_MEM: bool = false;

/// Enables scoped access into user memory, allowing page faults to occur inside
/// kernel.
pub fn access_user_memory<R>(f: impl FnOnce() -> R) -> R {
    ACCESSING_USER_MEM.with_current(|v| {
        *v = true;
        let result = f();
        *v = false;
        result
    })
}

/// Check if the current thread is accessing user memory.
pub fn is_accessing_user_memory() -> bool {
    ACCESSING_USER_MEM.read_current()
}
#[register_trap_handler(PAGE_FAULT)]
fn handle_page_fault(vaddr: VirtAddr, access_flags: MappingFlags, is_user: bool) -> bool {
    debug!(
        "Page fault at {:#x?}, flags: {:#x?}, is_user: {:?}",
        vaddr, access_flags, is_user
    );
    let current = current();
    let mut aspace = current.task_ext().process_data().aspace.lock();
    let result = aspace.handle_page_fault(vaddr, access_flags);
    debug!("Page fault result: {result:?}");
    result
}
