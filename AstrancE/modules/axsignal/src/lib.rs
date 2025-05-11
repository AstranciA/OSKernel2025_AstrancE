#![feature(naked_functions)]
#![no_std]
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate axlog;
use core::{
    arch::{asm, global_asm, naked_asm},
    ffi::{c_int, c_void},
    mem::MaybeUninit,
};

use arceos_posix_api::ctypes::{self, *};
use axerrno::{LinuxError, LinuxResult};
use axhal::arch::{TrapFrame, UspaceContext};
use bitflags::*;
use memory_addr::{VirtAddr, VirtAddrRange};
use syscalls::Sysno;

const NSIG: usize = 32;
/// signals
//#[repr(u32)]
//#[derive(Debug, Copy, Clone, PartialEq, Eq)]
//pub enum Signal {
//SIGBLOCK = SIG_BLOCK,
//SIGUNBLOCK = SIG_UNBLOCK,
//SIGSETMASK = SIG_SETMASK,
//SIGHUP = SIGHUP,
//SIGINT = SIGINT,
//SIGQUIT = SIGQUIT,
//SIGILL = SIGILL,
//SIGTRAP = SIGTRAP,
//SIGABRT = SIGABRT,
//SIGIOT = SIGIOT,
//SIGBUS = SIGBUS,
//SIGFPE = SIGFPE,
//SIGKILL = SIGKILL,
//SIGUSR1 = SIGUSR1,
//SIGSEGV = SIGSEGV,
//SIGUSR2 = SIGUSR2,
//SIGPIPE = SIGPIPE,
//SIGALRM = SIGALRM,
//SIGTERM = SIGTERM,
//SIGSTKFLT = SIGSTKFLT,
//SIGCHLD = SIGCHLD,
//SIGCONT = SIGCONT,
//SIGSTOP = SIGSTOP,
//SIGTSTP = SIGTSTP,
//SIGTTIN = SIGTTIN,
//SIGTTOU = SIGTTOU,
//SIGURG = SIGURG,
//SIGXCPU = SIGXCPU,
//SIGXFSZ = SIGXFSZ,
//SIGVTALRM = SIGVTALRM,
//SIGPROF = SIGPROF,
//SIGWINCH = SIGWINCH,
//SIGIO = SIGIO,
////SIGPOLL = SIGPOLL,
//SIGPWR = SIGPWR,
//SIGSYS = SIGSYS,
//SIGUNUSED = SIGUNUSED,
//}
//
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signal(u32);
impl Signal {
    pub fn from_u32(n: u32) -> Option<Self> {
        if n as usize > NSIG {
            None
        } else {
            Some(unsafe { core::mem::transmute(n) })
        }
    }
}
impl TryFrom<c_int> for Signal {
    type Error = SignalError;

    fn try_from(value: c_int) -> Result<Self, Self::Error> {
        if value < 0 || value as usize > NSIG {
            Err(SignalError::InvalidSignal)
        } else {
            Ok(Signal(value as u32))
        }
    }
}
impl From<u32> for Signal {
    fn from(n: u32) -> Self {
        Self(n)
    }
}
impl Into<u32> for Signal {
    fn into(self) -> u32 {
        self.0
    }
}
impl Into<u64> for Signal {
    fn into(self) -> u64 {
        self.0 as u64
    }
}

bitflags! {
    #[derive(Clone, Copy, Default, Debug)]
    pub struct SignalSet :u64 {
        const EMPTY = 0;
        const SIGHUP     = 1 << SIGHUP as usize;
        const SIGINT     = 1 << SIGINT as usize;
        const SIGQUIT    = 1 << SIGQUIT as usize;
        const SIGILL     = 1 << SIGILL as usize;
        const SIGTRAP    = 1 << SIGTRAP as usize;
        const SIGABRT    = 1 << SIGABRT as usize;
        const SIGIOT     = 1 << SIGIOT as usize;
        const SIGBUS     = 1 << SIGBUS as usize;
        const SIGFPE     = 1 << SIGFPE as usize;
        const SIGKILL    = 1 << SIGKILL as usize;
        const SIGUSR1    = 1 << SIGUSR1 as usize;
        const SIGSEGV    = 1 << SIGSEGV as usize;
        const SIGUSR2    = 1 << SIGUSR2 as usize;
        const SIGPIPE    = 1 << SIGPIPE as usize;
        const SIGALRM    = 1 << SIGALRM as usize;
        const SIGTERM    = 1 << SIGTERM as usize;
        const SIGSTKFLT  = 1 << SIGSTKFLT as usize;
        const SIGCHLD    = 1 << SIGCHLD as usize;
        const SIGCONT    = 1 << SIGCONT as usize;
        const SIGSTOP    = 1 << SIGSTOP as usize;
        const SIGTSTP    = 1 << SIGTSTP as usize;
        const SIGTTIN    = 1 << SIGTTIN as usize;
        const SIGTTOU    = 1 << SIGTTOU as usize;
        const SIGURG     = 1 << SIGURG as usize;
        const SIGXCPU    = 1 << SIGXCPU as usize;
        const SIGXFSZ    = 1 << SIGXFSZ as usize;
        const SIGVTALRM  = 1 << SIGVTALRM as usize;
        const SIGPROF    = 1 << SIGPROF as usize;
        const SIGWINCH   = 1 << SIGWINCH as usize;
        const SIGIO      = 1 << SIGIO as usize;
        const SIGPOLL    = 1 << SIGPOLL as usize;
        const SIGPWR     = 1 << SIGPWR as usize;
        const SIGSYS     = 1 << SIGSYS as usize;
        const SIGUNUSED  = 1 << SIGUNUSED as usize;
    }
}

impl SignalSet {
    pub fn get_one(&self) -> Option<Signal> {
        Signal::from_u32(self.bits().trailing_zeros())
    }
}

impl From<Signal> for SignalSet {
    fn from(sig: Signal) -> Self {
        Self::from_bits_retain(1 << sig.0)
    }
}
bitflags! {
    /// 信号处理标志位，匹配POSIX标准和Linux扩展
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct SigFlags: u32 {
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
    Action(unsafe extern "C" fn(c_int, *mut siginfo_t, *mut c_void)),
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

impl TryFrom<sigaction> for SigAction {
    type Error = SignalError;

    fn try_from(act: sigaction) -> Result<Self, Self::Error> {
        warn!("flag: {:x?}", act.sa_flags);
        /*
         *let flags = if let Some(flags) = SigFlags::from_bits(act.sa_flags.try_into().unwrap()) {
         *    flags
         *} else {
         *    return Err(SignalError::InvalidFlags);
         *};
         */
        let flags = SigFlags::from_bits_truncate(act.sa_flags as u32);

        let mask = SignalSet::from_bits(act.sa_mask.__bits[0]).expect("Unimplemeted signal");

        let handler = if flags.contains(SigFlags::SIG_INFO) {
            SigHandler::Handler(unsafe {
                act.__sa_handler
                    .sa_handler
                    .ok_or(SignalError::InvalidAction)?
            })
        } else {
            SigHandler::Action(unsafe {
                act.__sa_handler
                    .sa_sigaction
                    .ok_or(SignalError::InvalidAction)?
            })
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
                    act.__sa_handler.sa_handler = Some(f);
                }
                SigHandler::Action(f) => {
                    act.__sa_handler.sa_sigaction = Some(f);
                }
                SigHandler::Default => {
                    act.__sa_handler.sa_handler = Some(tmp);
                }
                SigHandler::Ignore => {
                    act.__sa_handler.sa_handler = Some(core::mem::transmute(SIG_IGN));
                }
            }
        }

        // 2. 设置信号掩码（RISC-V使用单个u64）
        act.sa_mask.__bits[0] = self.mask.bits();

        // 3. 设置标志位
        act.sa_flags = self.flags.bits() as i32;

        // 4. RISC-V不需要显式restorer，但保持ABI兼容
        act.sa_restorer = None;

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
    pub stack: SignalStackManager,
    pub actions: [SigAction; NSIG], // 信号处理表
    pub blocked: SignalSet,         // 被阻塞的信号
    pub pending: SignalSet,         // 待处理信号
}

impl Default for SignalContext {
    fn default() -> Self {
        let mut default = Self {
            stack: Default::default(),
            actions: Default::default(),
            blocked: Default::default(),
            pending: Default::default(),
        };
        default.set_action(
            Signal(SIGINT),
            SigAction {
                handler: SigHandler::Ignore,
                mask: SignalSet::empty(),
                flags: SigFlags::NO_DEFER,
            },
        );
        default.set_action(
            Signal(SIGSEGV),
            SigAction {
                handler: SigHandler::Default,
                mask: SignalSet::empty(),
                flags: SigFlags::NO_DEFER,
            },
        );
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
        self.actions[sig.0 as usize]
    }
    /// 设置信号处理动作，返回之前的动作
    pub fn set_action(&mut self, sig: Signal, act: SigAction) -> SigAction {
        warn!("set action: {act:?}");
        let old_act = self.actions[sig.0 as usize];
        self.actions[sig.0 as usize] = act;
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

    pub fn get_mask(&self) -> SignalSet {
        self.blocked
    }

    pub fn block(&mut self, mask: SignalSet) -> SignalSet {
        let old = self.blocked;
        self.blocked = self.blocked.union(mask);
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
        } = sigctx.actions[sig.0 as usize];

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
                        sig.0 as usize,
                    );
                    // 设置返回地址为信号返回trampoline
                    uctx.0.set_ra(sigreturn_trampoline as usize);

                    // 设置信号屏蔽字
                    let old_mask = sigctx.blocked;
                    sigctx.blocked |= mask;
                    handler(sig.0.try_into().unwrap());
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
                        sig.0 as usize,
                    );
                    // 设置返回地址为信号返回trampoline
                    uctx.0.set_ra(sigreturn_trampoline as usize);
                    // TODO: place siginfo_t in sig stack top
                    //uctx.set_args([sig.0 as usize, ]);

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
