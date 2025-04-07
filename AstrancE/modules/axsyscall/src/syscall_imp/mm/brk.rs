use axtask::{TaskExtRef, current};
use axconfig;

pub fn sys_brk(addr: usize) -> isize {
    // 执行系统调用主体逻辑
    let res = (|| -> axerrno::LinuxResult<_> {
        let current_task = current();
        let mut return_val: isize = current_task.task_ext().get_heap_top() as isize;
        let heap_bottom = current_task.task_ext().get_heap_bottom() as usize;

        if addr != 0 && addr >= heap_bottom && addr <= heap_bottom + axconfig::plat::USER_HEAP_SIZE {
            current_task.task_ext().set_heap_top(addr as u64);
            return_val = addr as isize;
        }

        Ok(return_val)
    })();

    // 处理日志记录
    match res {
        Ok(_) | Err(axerrno::LinuxError::EAGAIN) => {
            debug!("sys_brk => {:?}", res);
        }
        Err(_) => {
            info!("sys_brk => {:?}", res);
        }
    }

    // 处理返回值
    match res {
        Ok(v) => v as _,
        Err(e) => {
            -e.code() as _
        }
    }
}
