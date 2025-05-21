use axerrno::LinuxResult;
use axtask::{TaskExtRef, current};
use memory_addr::va;

pub(crate) fn sys_brk(new_heap_top: usize) -> LinuxResult<isize> {
    let current_task = current();
    let old_top = current_task.task_ext().heap_top();
    if (new_heap_top != 0) {
        // TODO: Validate heap address
        current_task.task_ext().set_heap_top(va!(new_heap_top));
    }
    // FIXME: return old_top or new_top????
    Ok(old_top.as_usize() as isize)
    //Ok(current_task.task_ext().heap_top().as_usize() as isize)
}
