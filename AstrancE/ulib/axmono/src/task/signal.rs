use arceos_posix_api::ctypes::*;
use axhal::arch::{TrapFrame, UspaceContext};
use axtask::{current, AxTaskRef, TaskExtMut, TaskExtRef};
use bitflags::*;
use syscalls::Sysno;

const NSIG: i32 = 32;
/// signals
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Signal {
    SIGBLOCK = SIG_BLOCK,
    SIGUNBLOCK = SIG_UNBLOCK,
    SIGSETMASK = SIG_SETMASK,
    SIGHUP = SIGHUP,
    SIGINT = SIGINT,
    SIGQUIT = SIGQUIT,
    SIGILL = SIGILL,
    SIGTRAP = SIGTRAP,
    SIGABRT = SIGABRT,
    SIGIOT = SIGIOT,
    SIGBUS = SIGBUS,
    SIGFPE = SIGFPE,
    SIGKILL = SIGKILL,
    SIGUSR1 = SIGUSR1,
    SIGSEGV = SIGSEGV,
    SIGUSR2 = SIGUSR2,
    SIGPIPE = SIGPIPE,
    SIGALRM = SIGALRM,
    SIGTERM = SIGTERM,
    SIGSTKFLT = SIGSTKFLT,
    SIGCHLD = SIGCHLD,
    SIGCONT = SIGCONT,
    SIGSTOP = SIGSTOP,
    SIGTSTP = SIGTSTP,
    SIGTTIN = SIGTTIN,
    SIGTTOU = SIGTTOU,
    SIGURG = SIGURG,
    SIGXCPU = SIGXCPU,
    SIGXFSZ = SIGXFSZ,
    SIGVTALRM = SIGVTALRM,
    SIGPROF = SIGPROF,
    SIGWINCH = SIGWINCH,
    SIGIO = SIGIO,
    SIGPOLL = SIGPOLL,
    SIGPWR = SIGPWR,
    SIGSYS = SIGSYS,
    SIGUNUSED = SIGUNUSED,
}

impl Signal {
    pub fn from_u32(n: u32) -> Option<Self> {
        if n > NSIG {
            None
        } else {
            Some(unsafe { core::mem::transmute(n) })
        }
    }
}

bitflags! {
    pub struct SignalSet :u32 {
        const SIG_BLOCK = 1 << Signal::SIGBLOCK as usize;
    }
}

impl SignalSet {
    pub fn get_one(&self) -> Option<Signal> {
        let sig = self.bits().trailing_zeros();
        Signal::from_u32(sig)
    }
}

impl From<Signal> for SignalSet {
    fn from(sig: Signal) -> Self {
        Self::from_bits_retain(1 << sig as usize)
    }
}

pub enum SigHandler {
    Default,
    Ignore,
    Handler(unsafe extern "C" fn(i32)),
}

// 信号动作配置
pub struct SigAction {
    pub handler: SigHandler,
    pub mask: SignalSet,
    pub flags: i32,
}

#[naked]
#[no_mangle]
unsafe extern "C" fn sigreturn_trampoline() {
    // 内联汇编确保无函数前导/后导代码
    asm!(
        "li a7, {sysno}",
        "ecall",
        sysno = const Sysno::rt_sigreturn as usize,
        options(noreturn)
    );
}

/// 信号返回trampoline
global_asm!(
    r#"
    .global sigreturn_trampoline
    sigreturn_trampoline:
        li a7, {sys_sigreturn}
        ecall
    "#,
    sys_sigreturn = const Sysno::rt_sigreturn,
);

// 进程信号上下文
#[derive(Default)]
pub struct SignalContext {
    pub handlers: [SigAction; NSIG as usize], // 信号处理表
    pub blocked: SignalSet,                   // 被阻塞的信号
    pub pending: SignalSet,                   // 待处理信号
}

impl SignalContext {
    /// 向进程发送信号
    pub fn send_signal(&mut self, sig: Signal) {
        let mask = 1 << (sig as u8 - 1);

        // 如果信号未被阻塞，则加入待处理队列
        if !self.pending.contains(sig) {
            self.pending = self.pending.union(sig);
        }
    }

    /// 检查是否有待处理信号
    pub fn has_pending(&self) -> bool {
        self.pending == 0
    }
}

pub fn handle_pending_signals() {
    let current_task = current();
    let tast_ext = current_task.task_ext_mut();
    let mut ctx: SignalContext = tast_ext.sigctx;
    let curr_sp = tast_ext.uctx.get_sp();

    while ctx.has_pending() {
        // 找到最高优先级的待处理信号
        let sig = ctx.pending.get_one().unwrap();
        let sig_action = &ctx.handlers[sig as usize];

        match sig_action.handler {
            SigHandler::Default => handle_default_signal(sig, regs),
            SigHandler::Ignore => {} // 直接忽略
            SigHandler::Handler(handler) => {
                // 设置信号处理栈帧
                // WARN: 在syscall rt_sigreturn中清除信号。
                unsafe { enter_signal_handler(&mut ctx, curr_sp, handler, sig) };
            }
        }

        // 清除已处理的信号
        ctx.pending.remove(sig);
    }
}

unsafe fn enter_signal_handler(
    sigctx: &mut SignalContext,
    ustack_top: usize,
    sig_action: SigAction,
    handler: unsafe extern "C" fn(i32),
    sig: i32,
) -> ! {
    let curr = current();
    let sigctx = curr.task_ext().sigctx;
    // 设置用户处理函数上下文，栈接着原来的用户栈
    // 信号编号作为第一个参数
    let uctx = UspaceContext::new(handler as usize, ustack_top, sig as usize);

    // 跳转到处理函数
    uctx.set_ip(handler as usize);
    uctx.sepc = handler as usize;

    // 设置返回地址为信号返回trampoline
    regs.ra = sigreturn_trampoline as usize;

    // 设置信号屏蔽字
    let old_mask = current_task().signal_ctx.blocked;
    sigctx.blocked |= sig_action.mask;
    frame.saved_mask = old_mask;
    unsafe { uctx.enter_uspace(task.get_sig_stack_top()) };
}

fn enter_signal_handler(tf: &mut TrapFrame) {}
