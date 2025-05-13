#![feature(naked_functions)]
#![no_std]
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate axlog;
use core::{
    arch::naked_asm,
    ffi::{c_int, c_void},
    mem::MaybeUninit,
    u64,
};

use axerrno::{LinuxError, LinuxResult};
use axhal::arch::{TrapFrame, UspaceContext};
use bitflags::*;
use linux_raw_sys::general::*;
use memory_addr::{VirtAddr, VirtAddrRange};
use syscalls::Sysno;

const NSIG: usize = 64;
/// signals
#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Signal {
    SIGBLOCK = SIG_BLOCK as usize,
    //SIGUNBLOCK = SIG_UNBLOCK,
    //SIGSETMASK = SIG_SETMASK,
    SIGHUP = SIGHUP as usize,
    SIGINT = SIGINT as usize,
    SIGQUIT = SIGQUIT as usize,
    SIGILL = SIGILL as usize,
    SIGTRAP = SIGTRAP as usize,
    //SIGABRT = SIGABRT,
    SIGIOT = SIGIOT as usize,
    SIGBUS = SIGBUS as usize,
    SIGFPE = SIGFPE as usize,
    SIGKILL = SIGKILL as usize,
    SIGUSR1 = SIGUSR1 as usize,
    SIGSEGV = SIGSEGV as usize,
    SIGUSR2 = SIGUSR2 as usize,
    SIGPIPE = SIGPIPE as usize,
    SIGALRM = SIGALRM as usize,
    SIGTERM = SIGTERM as usize,
    SIGSTKFLT = SIGSTKFLT as usize,
    SIGCHLD = SIGCHLD as usize,
    SIGCONT = SIGCONT as usize,
    SIGSTOP = SIGSTOP as usize,
    SIGTSTP = SIGTSTP as usize,
    SIGTTIN = SIGTTIN as usize,
    SIGTTOU = SIGTTOU as usize,
    SIGURG = SIGURG as usize,
    SIGXCPU = SIGXCPU as usize,
    SIGXFSZ = SIGXFSZ as usize,
    SIGVTALRM = SIGVTALRM as usize,
    SIGPROF = SIGPROF as usize,
    SIGWINCH = SIGWINCH as usize,
    SIGIO = SIGIO as usize,
    //SIGPOLL = SIGPOLL as usize,
    SIGPWR = SIGPWR as usize,
    //SIGSYS = SIGSYS as usize,
    SIGUNUSED = SIGUNUSED as usize,
}

//#[derive(Debug, Clone, Copy, PartialEq, Eq)]
//pub struct Signal(u32);
impl Signal {
    pub fn from_u32(n: u32) -> Option<Self> {
        if n as usize > NSIG {
            None
        } else {
            Some(unsafe { core::mem::transmute(n as usize) })
        }
    }
}
impl TryFrom<c_int> for Signal {
    type Error = SignalError;

    fn try_from(value: c_int) -> Result<Self, Self::Error> {
        if value < 0 || value as usize > NSIG {
            Err(SignalError::InvalidSignal)
        } else {
            Ok(unsafe { core::mem::transmute(value as usize) })
        }
    }
}
/*
 *impl Into<u32> for Signal {
 *    fn into(self) -> u32 {
 *        self as
 *    }
 *}
 *impl Into<u64> for Signal {
 *    fn into(self) -> u64 {
 *        self.0 as u64
 *    }
 *}
 */

bitflags! {
    #[derive(Clone, Copy, Default, Debug)]
    pub struct SignalSet :u64 {
        const EMPTY = 0;
        const SIGHUP     = 1 << SIGHUP;
        const SIGINT     = 1 << SIGINT;
        const SIGQUIT    = 1 << SIGQUIT;
        const SIGILL     = 1 << SIGILL;
        const SIGTRAP    = 1 << SIGTRAP;
        const SIGABRT    = 1 << SIGABRT;
        const SIGIOT     = 1 << SIGIOT;
        const SIGBUS     = 1 << SIGBUS;
        const SIGFPE     = 1 << SIGFPE;
        const SIGKILL    = 1 << SIGKILL;
        const SIGUSR1    = 1 << SIGUSR1;
        const SIGSEGV    = 1 << SIGSEGV;
        const SIGUSR2    = 1 << SIGUSR2;
        const SIGPIPE    = 1 << SIGPIPE;
        const SIGALRM    = 1 << SIGALRM;
        const SIGTERM    = 1 << SIGTERM;
        const SIGSTKFLT  = 1 << SIGSTKFLT;
        const SIGCHLD    = 1 << SIGCHLD;
        const SIGCONT    = 1 << SIGCONT;
        const SIGSTOP    = 1 << SIGSTOP;
        const SIGTSTP    = 1 << SIGTSTP;
        const SIGTTIN    = 1 << SIGTTIN;
        const SIGTTOU    = 1 << SIGTTOU;
        const SIGURG     = 1 << SIGURG;
        const SIGXCPU    = 1 << SIGXCPU;
        const SIGXFSZ    = 1 << SIGXFSZ;
        const SIGVTALRM  = 1 << SIGVTALRM;
        const SIGPROF    = 1 << SIGPROF;
        const SIGWINCH   = 1 << SIGWINCH;
        const SIGIO      = 1 << SIGIO;
        const SIGPOLL    = 1 << SIGPOLL;
        const SIGPWR     = 1 << SIGPWR;
        const SIGSYS     = 1 << SIGSYS;
        const SIGUNUSED  = 1 << SIGUNUSED ;
    }
}

impl SignalSet {
    /// get lowest signal in the set
    /// will return None if the set is empty (trailing_zeros == NSIG)
    pub fn get_one(&self) -> Option<Signal> {
        Signal::from_u32(self.bits().trailing_zeros())
    }

    /// get lowest signal in the set that is in the filter set
    /// will return None if no signal in the set is in the filter set
    pub fn get_one_in(&self, filter: SignalSet) -> Option<Signal> {
        Signal::from_u32(self.intersection(filter).bits().trailing_zeros())
    }

    /// take the lowest signal in the set and remove it from the set
    pub fn take_one(&mut self) -> Option<Signal> {
        if let Some(sig) = self.get_one() {
            self.remove(sig.into());
            Some(sig)
        } else {
            None
        }
    }

    /// take the lowest signal in the set that is in the filter set and remove it from the set
    pub fn take_one_in(&mut self, filter: SignalSet) -> Option<Signal> {
        if let Some(sig) = self.get_one_in(filter) {
            self.remove(sig.into());
            Some(sig)
        } else {
            None
        }
    }
}

impl From<Signal> for SignalSet {
    fn from(sig: Signal) -> Self {
        Self::from_bits_retain(1 << sig as usize)
    }
}

#[cfg(any(target_arch = "riscv64", target_arch = "loongarch64"))]
impl From<sigset_t> for SignalSet {
    fn from(value: sigset_t) -> Self {
        Self::from_bits_retain(value.sig[0])
    }
}

bitflags! {
    /// 信号处理标志位，匹配POSIX标准和Linux扩展
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct SigFlags: usize {
        /// 子进程停止时不发送SIGCHLD (SA_NOCLDSTOP)
        const NO_CHILD_STOP = 0x0000_0001;
        /// 子进程退出时不成为僵尸进程 (SA_NOCLDWAIT)
        const NO_CHILD_WAIT = 0x0000_0002;
        /// 使用三参数信号处理函数 (提供`siginfo_t`和上下文) (SA_SIGINFO)
        const SIG_INFO = 0x0000_0004;
        /// 被信号中断的系统调用自动重启 (SA_RESTART)
        const RESTART = 0x0000_0010;
        /// 使用备用信号栈 (通过`sigaltstack`设置) (SA_ONSTACK)
        const ON_STACK = 0x0800_0000;
        /// 不自动阻塞当前信号 (SA_NODEFER)
        const NO_DEFER = 0x4000_0000;
        /// 执行处理程序后重置为默认处理 (SA_RESETHAND)
        const RESET_HAND = 0x8000_0000;
        /// 不自动重启系统调用 (历史遗留标志) (SA_INTERRUPT)
        const INTERRUPT = 0x2000_0000;
        /// 接收信号时通知ptrace (Linux特有) (SA_SIGINFO)
        const PT_TRACE = 0x0000_0020;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SigHandler {
    Default,
    Ignore,
    Handler(unsafe extern "C" fn(c_int)),
    //actually Action(unsafe extern "C" fn(c_int, *mut siginfo_t, *mut c_void)),
    // this is for capabilites, since the fn won't be called directly
    Action(unsafe extern "C" fn(c_int)),
}

impl Default for SigHandler {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Default, Clone, Copy, Debug)]
/// 信号动作配置
pub struct SigAction {
    pub handler: SigHandler,
    pub mask: SignalSet,
    pub flags: SigFlags,
}

#[cfg(any(target_arch = "riscv64", target_arch = "loongarch64"))]
impl TryFrom<sigaction> for SigAction {
    type Error = SignalError;

    fn try_from(act: sigaction) -> Result<Self, Self::Error> {
        /*
         *let flags = if let Some(flags) = SigFlags::from_bits(act.sa_flags.try_into().unwrap()) {
         *    flags
         *} else {
         *    return Err(SignalError::InvalidFlags);
         *};
         */
        let flags = SigFlags::from_bits_retain(act.sa_flags as usize);

        let mask = act.sa_mask.into();

        let handler = if flags.contains(SigFlags::SIG_INFO) {
            SigHandler::Handler(unsafe { act.sa_handler.ok_or(SignalError::InvalidAction)? })
        } else {
            SigHandler::Action(unsafe { act.sa_handler.ok_or(SignalError::InvalidAction)? })
        };

        Ok(Self {
            handler,
            mask,
            flags,
        })
    }
}

impl Into<sigaction> for SigAction {
    fn into(self) -> sigaction {
        // 初始化一个全零的sigaction（避免未初始化字段）
        let mut act: sigaction = unsafe { MaybeUninit::zeroed().assume_init() };

        // 1. 设置处理函数联合体
        unsafe {
            match self.handler {
                SigHandler::Handler(f) => {
                    act.sa_handler = Some(f);
                }
                SigHandler::Action(f) => {
                    act.sa_handler = Some(f);
                }
                SigHandler::Default => {
                    act.sa_handler = Some(tmp);
                }
                SigHandler::Ignore => {
                    act.sa_handler = Some(core::mem::transmute(SIG_IGN));
                }
            }
        }

        // 2. 设置信号掩码（RISC-V使用单个u64）
        act.sa_mask = unsafe { core::mem::transmute(self.mask.bits()) };

        // 3. 设置标志位
        act.sa_flags = self.flags.bits() as u64;

        // 4. RISC-V不需要显式restorer，但保持ABI兼容
        // act.sa_restorer = None;

        act
    }
}

pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

unsafe extern "C" fn tmp(a: i32) {}

#[naked]
#[unsafe(no_mangle)]
unsafe extern "C" fn sigreturn_trampoline() {
    // 内联汇编确保无函数前导/后导代码
    naked_asm!(
        "li a7, {sysno}",
        "ecall",
        sysno = const Sysno::rt_sigreturn as usize,
    );
}

// 进程信号上下文
pub struct SignalContext {
    stack: SignalStackManager,
    actions: [SigAction; NSIG], // 信号处理表
    blocked: SignalSet,         // 被阻塞的信号
    pending: SignalSet,         // 待处理信号
}

impl Default for SignalContext {
    fn default() -> Self {
        let mut default = Self {
            stack: Default::default(),
            actions: [Default::default(); NSIG],
            blocked: Default::default(),
            pending: Default::default(),
        };
        default.set_action(Signal::SIGINT, SigAction {
            handler: SigHandler::Ignore,
            mask: SignalSet::empty(),
            flags: SigFlags::NO_DEFER,
        });
        default.set_action(Signal::SIGSEGV, SigAction {
            handler: SigHandler::Default,
            mask: SignalSet::empty(),
            flags: SigFlags::NO_DEFER,
        });
        default
    }
}

impl SignalContext {
    /// 向进程发送信号
    pub fn send_signal(&mut self, sig: SignalSet) {
        // 如果信号未被阻塞，则加入待处理队列
        if self.blocked.intersection(sig).is_empty() {
            self.pending = self.pending.union(sig);
        }
    }

    /// 检查是否有待处理信号
    pub fn has_pending(&self) -> bool {
        self.pending.is_empty()
    }

    /// 获取信号处理动作，返回之前的动作
    pub fn get_action(&mut self, sig: Signal) -> SigAction {
        self.actions[sig as usize]
    }
    /// 设置信号处理动作，返回之前的动作
    pub fn set_action(&mut self, sig: Signal, act: SigAction) -> SigAction {
        warn!("set action: {act:?}");
        let old_act = self.actions[sig as usize];
        self.actions[sig as usize] = act;
        old_act
    }

    pub fn set_level(&mut self, ty: SignalStackType) -> SignalResult<VirtAddrRange> {
        self.stack.switch_to(ty)
    }

    pub fn current_stack(&self) -> SignalResult<VirtAddrRange> {
        if let Some(range) = self.stack.current_stack() {
            Ok(range)
        } else {
            Err(SignalError::NoStack)
        }
    }

    pub fn get_blocked(&self) -> SignalSet {
        self.blocked
    }

    pub fn take_pending_in(&mut self, filter: SignalSet) -> Option<Signal> {
        self.pending.take_one_in(filter)
    }

    pub fn block(&mut self, mask: SignalSet) -> SignalSet {
        let old = self.blocked;
        self.blocked = self.blocked.union(mask);
        old
    }

    pub fn unblock(&mut self, mask: SignalSet) -> SignalSet {
        let old = self.blocked;
        self.blocked = self.blocked.difference(mask);
        old
    }

    pub fn set_mask(&mut self, mask: SignalSet) -> SignalSet {
        let old = self.blocked;
        self.blocked = mask;
        old
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SignalError {
    NoStack,          // 信号栈不可用
    SigNotImpl,       // 信号未实现
    InvalidAction,    // 无效的处理动作
    InvalidFlags,     // 无效的标志位组合
    InvalidSignal,    // 无效的信号编号
    PermissionDenied, // 权限不足(如设置SIGKILL)
}
impl From<SignalError> for LinuxError {
    fn from(err: SignalError) -> LinuxError {
        match err {
            SignalError::NoStack => LinuxError::ENOMEM, // 无内存用于信号栈
            SignalError::SigNotImpl => LinuxError::ENOSYS, // 功能未实现
            SignalError::InvalidAction => LinuxError::EINVAL, // 无效参数
            SignalError::InvalidFlags => LinuxError::EINVAL, // 无效标志
            SignalError::InvalidSignal => LinuxError::EINVAL, // 无效信号号
            SignalError::PermissionDenied => LinuxError::EPERM,
        }
    }
}

type SignalResult<T> = Result<T, SignalError>;
#[derive(Clone, Copy)]
pub enum SignalStackType {
    Primary = 0,   // 主线程栈
    Alternate = 1, // 用户指定的备用栈
    Emergency = 2, // 内核紧急栈（只用于同步信号）
}

pub struct SignalStackManager {
    stacks: [Option<VirtAddrRange>; 3],
    current: SignalStackType,
}

impl SignalStackManager {
    pub fn current_stack(&self) -> Option<VirtAddrRange> {
        self.stacks[self.current as usize]
    }
    pub fn set_stack(&mut self, ty: SignalStackType, range: VirtAddrRange) {
        self.stacks[ty as usize] = Some(range);
    }
}

impl Default for SignalStackManager {
    fn default() -> Self {
        Self {
            stacks: [None, None, None],
            current: SignalStackType::Primary,
        }
    }
}

impl SignalStackManager {
    fn switch_to(&mut self, ty: SignalStackType) -> SignalResult<VirtAddrRange> {
        let range = self.stacks[ty as usize].ok_or(SignalError::NoStack)?;
        self.current = ty;
        Ok(range)
    }
}

pub fn handle_pending_signals(sigctx: &mut SignalContext) {
    while sigctx.has_pending() {
        // 找到最高优先级的待处理信号
        let sig = sigctx.pending.get_one().unwrap();
        let SigAction {
            handler,
            mask,
            flags,
        } = sigctx.actions[sig as usize];

        match handler {
            SigHandler::Default => handle_default_signal(sig, &mut *sigctx),
            SigHandler::Ignore => {} // 直接忽略
            SigHandler::Handler(handler) => {
                // 设置信号处理栈帧
                // WARN: 在syscall rt_sigreturn中清除信号。
                unsafe {
                    let mut uctx = UspaceContext::new(
                        handler as usize,
                        sigctx.current_stack().expect("Sig stack not set").start,
                        sig as usize,
                    );
                    // 设置返回地址为信号返回trampoline
                    uctx.0.set_ra(sigreturn_trampoline as usize);

                    // 设置信号屏蔽字
                    let old_mask = sigctx.blocked;
                    sigctx.blocked |= mask;
                    handler(sig as i32);
                    //unsafe { uctx.enter_uspace(current_task.kernel_stack_top().unwrap()) };
                };
            }
            SigHandler::Action(handler) => {
                // 设置信号处理栈帧
                // WARN: 在syscall rt_sigreturn中清除信号。
                unsafe {
                    let mut uctx = UspaceContext::new(
                        handler as usize,
                        sigctx.current_stack().expect("Sig stack not set").start,
                        sig as usize,
                    );
                    // 设置返回地址为信号返回trampoline
                    uctx.0.set_ra(sigreturn_trampoline as usize);
                    // TODO: place siginfo_t in sig stack top
                    //uctx.set_args([sig as usize, ]);

                    // 设置信号屏蔽字
                    let old_mask = sigctx.blocked;
                    sigctx.blocked |= mask;
                    //TODO: handler();
                    //unsafe { uctx.enter_uspace(current_task.kernel_stack_top().unwrap()) };
                };
            }
        }

        // 清除已处理的信号
        sigctx.pending.remove(sig.into());
    }
}

fn handle_default_signal(sig: Signal, ctx: &mut SignalContext) {
    todo!()
}
