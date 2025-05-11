use core::ffi::c_int;

use arceos_posix_api::ctypes::{self, *};
use axerrno::{LinuxError, LinuxResult};
use axsignal::*;
use axtask::{current, TaskExtRef};
pub(crate) fn sys_sigaction(
    signum: c_int,
    act: *const sigaction,
    old_act: *mut sigaction,
) -> LinuxResult<isize> {
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

    unsafe {
        oldset
            .as_mut()
            .map(|oldset| oldset.__bits[0] = sigctx.get_mask().bits());
    }

    if !set.is_null() {
        let set = SignalSet::from_bits_truncate(unsafe { (*set).__bits[0] });
        match how as u32 {
            SIG_BLOCK => {
                sigctx.blocked = sigctx.blocked.union(set);
            }
            SIG_UNBLOCK => {
                sigctx.blocked = sigctx.blocked.difference(set);
            }
            SIG_SETMASK => {
                sigctx.blocked = set;
            }
            _ => return Err(LinuxError::EINVAL),
        }
    }

    Ok(0)
}

pub(crate) fn sys_kill(pid: pid_t, sig: c_int) {
}
