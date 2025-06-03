use axprocess::Pid;
use axsignal::{Signal, SignalSet};
//use axsignal::{SignalInfo, Signo};
use crate::task::ProcessData;
use axtask::{TaskExtRef, current};
use linux_raw_sys::general::SI_KERNEL;

use crate::ptr::{PtrWrapper, UserPtr};

pub fn do_exit(exit_code: i32, group_exit: bool) -> ! {
    let curr = current();
    let curr_ext = curr.task_ext();

    let thread = &curr_ext.thread;
    info!("{:?} exit with code: {}", thread, exit_code);

    let clear_child_tid = UserPtr::<Pid>::from(curr_ext.thread_data().clear_child_tid());
    if let Ok(clear_tid) = clear_child_tid.get() {
        unsafe { clear_tid.write(0) };
        // TODO: wake up threads, which are blocked by futex, and waiting for the address pointed by clear_child_tid
    }

    let process = thread.process();
    if thread.exit(exit_code) {
        process.exit();
        if let Some(parent) = process.parent() {
            /*
             *if let Some(signo) = process.data::<ProcessData>().and_then(|it| it.exit_signal) {
             *    let _ = send_signal_process(&parent, SignalInfo::new(signo, SI_KERNEL as _));
             *}
             */
            if let Some(data) = parent.data::<ProcessData>() {
                trace!("send SIGCHLD to parent {:?}", parent.pid());
                data.signal.lock().send_signal(SignalSet::SIGCHLD);
                data.child_exit_wq.notify_all(false);
            }
        }

        process.exit();
        // TODO: clear namespace resources
    }
    if group_exit && !process.is_group_exited() {
        process.group_exit();
        //let sig = SignalInfo::new(Signo::SIGKILL, SI_KERNEL as _);
        for thr in process.threads() {
            //let _ = send_signal_thread(&thr, sig.clone());
            // TODO: thread local sigctx
            //thr.data::<ThreadData>().and_then(||)
        }
    }
    axtask::exit(exit_code)
}

pub fn sys_exit(exit_code: i32) -> ! {
    do_exit(exit_code << 8, false)
}

pub fn sys_exit_group(exit_code: i32) -> ! {
    do_exit(exit_code << 8, true)
}
