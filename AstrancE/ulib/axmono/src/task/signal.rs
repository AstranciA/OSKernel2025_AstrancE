use core::{ffi::c_int, time::Duration};

//use arceos_posix_api::ctypes::{self, *};
use axerrno::{LinuxError, LinuxResult, ax_err};
use axhal::time::monotonic_time;
use axsignal::*;
use axtask::{TaskExtRef, current, yield_now};
use linux_raw_sys::general::*;

use crate::ptr::{PtrWrapper, UserPtr};

use super::{
    PROCESS_TABLE, ProcessData, THREAD_TABLE, time::TimeStat, time_stat_from_old_task,
    time_stat_to_new_task, yield_with_time_stat,
};
pub(crate) fn sys_sigaction(
    signum: c_int,
    act: *const sigaction,
    old_act: *mut sigaction,
) -> LinuxResult<isize> {
    error!("sigacton");
    let sig: Signal = signum.try_into()?;
    let curr = current();
    let mut sigctx = curr.task_ext().process_data().signal.lock();
    if !act.is_null() {
        let act = SigAction::try_from(unsafe { *act }).inspect_err(|e| {
            warn!("{e:?}");
        })?;
        let old = sigctx.set_action(sig, act);

        // 设置旧动作（如果有）
        unsafe { old_act.as_mut().map(|ptr| unsafe { *ptr = old.into() }) };
    } else {
        // 只获取旧动作（如果有）
        unsafe {
            let old = sigctx.get_action(sig);
            old_act
                .as_mut()
                .map(|ptr| unsafe { *ptr = sigctx.get_action(sig).into() });
        };
    }

    Ok(0)
}

pub(crate) fn sys_sigprocmask(
    how: c_int,
    set: *const sigset_t,
    oldset: *mut sigset_t,
) -> LinuxResult<isize> {
    let curr = current();
    let mut sigctx = curr.task_ext().process_data().signal.lock();

    if !set.is_null() {
        warn!("{:x?}", unsafe{*set});
        let set: SignalSet = unsafe { *set }.into();
        warn!("{set:?}");

        let old = match how as u32 {
            SIG_BLOCK => sigctx.block(set),
            SIG_UNBLOCK => sigctx.unblock(set),
            SIG_SETMASK => sigctx.set_mask(set),
            _ => return Err(LinuxError::EINVAL),
        };
        unsafe {
            oldset
                .as_mut()
                .map(|ptr| unsafe { *ptr }.sig[0] = old.bits())
        };
    }

    Ok(0)
}

pub(crate) fn sys_kill(pid: c_int, sig: c_int) -> LinuxResult<isize> {
    let sig = Signal::from_u32(sig as _).ok_or(LinuxError::EINVAL)?;
    warn!("{pid:?}, {sig:?}");
    panic!();
    if pid > 0 {
        let process = PROCESS_TABLE
            .read()
            .get(&(pid as _))
            .ok_or(LinuxError::ESRCH)?;
        let data: &ProcessData = process.data().ok_or_else(|| {
            error!("Process {} has no data", pid);
            LinuxError::EFAULT
        })?;
        data.send_signal(sig);
    } else {
        warn!("Not supported yet: pid: {:?}", pid);
        return Err(LinuxError::EINVAL);
    }
    Ok(0)
}

pub(crate) fn sys_sigtimedwait(
    sigset: *const sigset_t,
    info: *mut siginfo_t,
    timeout: *const timespec,
) -> LinuxResult<isize> {
    let sigset: SignalSet = unsafe { *(sigset.as_ref().ok_or(LinuxError::EFAULT)?) }.into();
    let curr = current();
    let start_time = monotonic_time();

    // 检查是否有超时设置
    let has_timeout = !timeout.is_null();
    let timeout_duration = if has_timeout {
        let ts = unsafe { timeout.as_ref().ok_or(LinuxError::EFAULT)? };
        if ts.tv_sec == 0 && ts.tv_nsec == 0 {
            // 立即返回的特殊情况
            return curr
                .task_ext()
                .process_data()
                .signal
                .lock()
                .take_pending_in(sigset)
                .ok_or(LinuxError::EAGAIN)
                .map(|sig| sig as isize);
        }
        Some(Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32))
    } else {
        None
    };

    // 主等待循环
    loop {
        // 检查是否有待处理的信号
        if let Some(sig) = curr
            .task_ext()
            .process_data()
            .signal
            .lock()
            .take_pending_in(sigset)
        {
            return Ok(sig as isize);
        }

        // 检查超时
        if let Some(duration) = timeout_duration {
            let elapsed = monotonic_time() - start_time;
            if elapsed >= duration {
                return Err(LinuxError::EAGAIN);
            }
        }

        // 让出CPU
        yield_with_time_stat();
    }
}
