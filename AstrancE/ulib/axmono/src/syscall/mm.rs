use axerrno::{LinuxError, LinuxResult};
use axtask::{TaskExtRef, current};
use memory_addr::va;
use page_table_entry::MappingFlags;

pub(crate) fn sys_brk(new_heap_top: usize) -> LinuxResult<isize> {
    let current_task = current();
    let mut aspace = current_task.task_ext().process_data().aspace.lock();
    let old_top = aspace.heap().top();
    if (new_heap_top != 0) {
        // TODO: Validate heap address
        aspace.set_heap_top(va!(new_heap_top));
    }
    // FIXME: return old_top or new_top????
    //Ok(old_top.as_usize() as isize)
    Ok(aspace.heap().top().as_usize() as isize)
}

pub(crate) fn sys_mprotect(addr: usize, size: usize, prot: usize) -> LinuxResult<isize> {
    let curr = current();
    let mut aspace = curr.task_ext().process_data().aspace.lock();
    let prot = MappingFlags::from_bits(prot).ok_or(LinuxError::EINVAL)?;
    debug!(
        "mprotect: addr={:#x}, size={:#x}, prot={:?}",
        addr, size, prot
    );
    // TODO: Validate address and size
    aspace.protect(addr.into(), size, prot)?;
    Ok(0)
}
