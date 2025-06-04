use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    ctypes::TimeStat,
    elf::OwnedElfFile,
    mm::{load_elf_to_mem, map_trampoline},
    task::add_thread_to_table,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use arceos_posix_api::{FD_TABLE, ctypes::*};
use axerrno::{AxError, AxResult, LinuxError, LinuxResult};
use axfs::{
    CURRENT_DIR, CURRENT_DIR_PATH,
    api::{current_dir, set_current_dir},
};
use axhal::arch::UspaceContext;
use axio::Read;
use axmm::{AddrSpace, kernel_aspace};
use axns::AxNamespace;
use axprocess::Pid;
use axsignal::{Signal, SignalContext};
use axsync::Mutex;
use axtask::{AxTaskRef, TaskExtRef, WaitQueue, current};
use core::ffi::c_int;
use memory_addr::VirtAddrRange;
use spin::RwLock;

use crate::{
    copy_from_kernel,
    ctypes::{CloneFlags, WaitStatus},
    elf::ELFInfo,
    loader::load_elf_from_disk,
    mm::map_elf_sections,
    task::TaskExt,
    utils::get_pwd_from_envs,
};

use super::{read_trapframe_from_kstack, spawn_user_task, spawn_user_task_inner};

/// Extended data for [`Process`].
pub struct ProcessData {
    /// The executable path
    pub exe_path: RwLock<String>,
    /// The virtual memory address space.
    pub aspace: Arc<Mutex<AddrSpace>>,
    /// The resource namespace
    pub ns: AxNamespace,

    /// The child exit wait queue
    pub child_exit_wq: WaitQueue,
    /// The exit signal of the thread
    pub exit_signal: Option<Signal>,

    /// The process signal manager
    pub signal: Arc<Mutex<SignalContext>>,
    pub signal_stack: Box<[u8; 4096]>,
}
impl ProcessData {
    /// Create a new [`ProcessData`].
    pub fn new(
        exe_path: String,
        aspace: Arc<Mutex<AddrSpace>>,
        signal: Arc<Mutex<SignalContext>>,
        exit_signal: Option<Signal>,
    ) -> Self {
        let signal_stack = Box::new([0u8; 4096]);
        let signal__ = signal.clone();
        let mut signal_ = signal__.lock();

        signal_.set_current_stack(axsignal::SignalStackType::Primary);
        signal_.set_stack(
            axsignal::SignalStackType::Primary,
            VirtAddrRange::from_start_size((signal_stack.as_ptr() as usize).into(), 4096),
        );
        Self {
            exe_path: RwLock::new(exe_path),
            aspace,
            ns: AxNamespace::new_thread_local(),
            child_exit_wq: WaitQueue::new(),
            exit_signal,
            signal,
            signal_stack,
        }
    }
    /// Initialize the namespace for the new task.
    pub(crate) fn ns_init_new(&self) {
        let ns = &self.ns;
        FD_TABLE.deref_from(ns).init_new(FD_TABLE.copy_inner());
        CURRENT_DIR
            .deref_from(ns)
            .init_new(CURRENT_DIR.copy_inner());
        CURRENT_DIR_PATH
            .deref_from(ns)
            .init_new(CURRENT_DIR_PATH.copy_inner());
    }
    /// Linux manual: A "clone" child is one which delivers no signal, or a
    /// signal other than SIGCHLD to its parent upon termination.
    pub fn is_clone_child(&self) -> bool {
        self.exit_signal != Signal::from_u32(SIGCHLD)
    }

    pub fn signal(&self) -> &Arc<Mutex<SignalContext>> {
        &self.signal
    }

    pub fn send_signal(&self, sig: Signal) {
        self.signal.lock().send_signal(sig.into());
    }
}
impl Drop for ProcessData {
    fn drop(&mut self) {
        if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
            // See [`crate::new_user_aspace`]

            debug!("Drop ProcessData");

            let kernel = kernel_aspace().lock();

            self.aspace
                .lock()
                .clear_mappings(VirtAddrRange::from_start_size(kernel.base(), kernel.size()));
        }
    }
}
/// Extended data for [`Thread`].
pub struct ThreadData {
    /// The clear thread tid field
    ///
    /// See <https://manpages.debian.org/unstable/manpages-dev/set_tid_address.2.en.html#clear_child_tid>
    ///
    /// When the thread exits, the kernel clears the word at this address if it is not NULL.
    pub clear_child_tid: AtomicUsize,
    // The thread-level signal manager
    //pub signal: ThreadSignalManager<RawMutex, WaitQueueWrapper>,
}

impl ThreadData {
    /// Create a new [`ThreadData`].
    #[allow(clippy::new_without_default)]
    pub fn new(proc: &ProcessData) -> Self {
        Self {
            clear_child_tid: AtomicUsize::new(0),
            //signal: ThreadSignalManager::new(proc.signal.clone()),
        }
    }

    /// Get the clear child tid field.
    pub fn clear_child_tid(&self) -> usize {
        self.clear_child_tid.load(Ordering::Relaxed)
    }

    /// Set the clear child tid field.
    pub fn set_clear_child_tid(&self, clear_child_tid: usize) {
        self.clear_child_tid
            .store(clear_child_tid, Ordering::Relaxed);
    }
}

/// From starry-next
/*
 *pub fn wait_pid(task: AxTaskRef, pid: i32, exit_code_ptr: *mut i32) -> Result<u64, WaitStatus> {
 *    let mut exit_task_id: usize = 0;
 *    let mut answer_id: u64 = 0;
 *    let mut answer_status = WaitStatus::NotExist;
 *
 *    for (index, child) in task.task_ext().children.lock().iter().enumerate() {
 *        //warn!("check child: {}", child.id_name());
 *        if pid <= 0 {
 *            if pid == 0 {
 *                axlog::warn!("Don't support for process group.");
 *            }
 *
 *            answer_status = WaitStatus::Running;
 *            if child.state() == axtask::TaskState::Exited {
 *                let exit_code = child.exit_code();
 *                answer_status = WaitStatus::Exited;
 *                debug!(
 *                    "wait pid _{}_ with code _{}_",
 *                    child.id().as_u64(),
 *                    exit_code
 *                );
 *                exit_task_id = index;
 *                if !exit_code_ptr.is_null() {
 *                    unsafe {
 *                        *exit_code_ptr = exit_code << 8;
 *                    }
 *                }
 *                answer_id = child.id().as_u64();
 *                break;
 *            }
 *        } else if child.id().as_u64() == pid as u64 {
 *            if let Some(exit_code) = child.join() {
 *                answer_status = WaitStatus::Exited;
 *                info!(
 *                    "wait pid _{}_ with code _{:?}_",
 *                    child.id().as_u64(),
 *                    exit_code
 *                );
 *                exit_task_id = index;
 *                if !exit_code_ptr.is_null() {
 *                    unsafe {
 *                        *exit_code_ptr = exit_code << 8;
 *                    }
 *                }
 *                answer_id = child.id().as_u64();
 *            } else {
 *                answer_status = WaitStatus::Running;
 *            }
 *            break;
 *        }
 *    }
 *
 *    if answer_status == WaitStatus::Running {
 *        axtask::yield_now();
 *    }
 *
 *    if answer_status == WaitStatus::Exited {
 *        task.task_ext().children.lock().remove(exit_task_id);
 *        return Ok(answer_id);
 *    }
 *    Err(answer_status)
 *}
 */

/// fork current task
/// **Return**
/// - `Ok(new_task_ref)` if fork successfully
pub fn fork(from_umode: bool) -> LinuxResult<AxTaskRef> {
    clone_task(None, CloneFlags::empty(), from_umode)
}

pub fn clone_task(
    stack: Option<usize>,
    flags: CloneFlags,
    from_umode: bool,
    /*
     *_ptid: usize,
     *_tls: usize,
     *_ctid: usize,
     */
) -> LinuxResult<AxTaskRef> {
    debug!("clone_task with flags: {:?}", flags);
    let curr = current();
    let current_task_ext = curr.task_ext();
    const FLAG_MASK: u32 = 0xff;
    let exit_signal = Signal::from_u32(flags.bits() & FLAG_MASK);
    // new task with same ip and sp of current task
    let mut trap_frame = read_trapframe_from_kstack(curr.get_kernel_stack_top().unwrap());

    let mut current_aspace = current_task_ext.process_data().aspace.lock();

    if from_umode {
        trap_frame.set_ret_code(0);
        trap_frame.step_ip();
    }

    // TODO: clone stack since it's always changed.
    // stack is copied meanwhilst addr space is copied
    //trap_frame.set_user_sp(stack);
    if let Some(stack) = stack {
        trap_frame.set_user_sp(stack);
    }

    let new_uctx = UspaceContext::from(&trap_frame);
    let current_pwd = current_dir()?;
    let mut new_task = spawn_user_task_inner(curr.name(), new_uctx, current_pwd);
    let tid = new_task.id().as_u64() as Pid;
    debug!("new process data");
    let process = if flags.contains(CloneFlags::THREAD) {
        new_task
            .ctx_mut()
            .set_page_table_root(current_aspace.page_table_root());

        curr.task_ext().thread.process()
    } else {
        let parent = if flags.contains(CloneFlags::PARENT) {
            curr.task_ext()
                .thread
                .process()
                .parent()
                .ok_or(LinuxError::EINVAL)?
        } else {
            curr.task_ext().thread.process().clone()
        };
        let builder = parent.fork(tid);
        let aspace = if flags.contains(CloneFlags::VM) {
            curr.task_ext().process_data().aspace.clone()
        } else {
            #[cfg(feature = "COW")]
            let mut aspace = current_aspace.clone_on_write()?;
            #[cfg(not(feature = "COW"))]
            let mut aspace = current_aspace.clone_or_err()?;
            copy_from_kernel(&mut aspace)?;
            Arc::new(Mutex::new(aspace))
        };
        new_task
            .ctx_mut()
            .set_page_table_root(aspace.lock().page_table_root());

        let signal = if flags.contains(CloneFlags::SIGHAND) {
            parent
                .data::<ProcessData>()
                .map_or_else(Arc::default, |it| it.signal.clone())
        } else {
            Arc::default()
        };
        let process_data = ProcessData::new(
            curr.task_ext().process_data().exe_path.read().clone(),
            aspace,
            signal,
            exit_signal,
        );

        if flags.contains(CloneFlags::FILES) {
            FD_TABLE
                .deref_from(&process_data.ns)
                .init_shared(FD_TABLE.share());
        } else {
            FD_TABLE
                .deref_from(&process_data.ns)
                .init_new(FD_TABLE.copy_inner());
        }

        if flags.contains(CloneFlags::FS) {
            CURRENT_DIR
                .deref_from(&process_data.ns)
                .init_shared(CURRENT_DIR.share());
            CURRENT_DIR_PATH
                .deref_from(&process_data.ns)
                .init_shared(CURRENT_DIR_PATH.share());
        } else {
            CURRENT_DIR
                .deref_from(&process_data.ns)
                .init_new(CURRENT_DIR.copy_inner());
            CURRENT_DIR_PATH
                .deref_from(&process_data.ns)
                .init_new(CURRENT_DIR_PATH.copy_inner());
        }
        &builder.data(process_data).build()
    };

    let thread_data = ThreadData::new(process.data().unwrap());
    /* TODO: child_tid
     *if flags.contains(CloneFlags::CHILD_CLEARTID) {
     *    thread_data.set_clear_child_tid(child_tid);
     *}
     */

    let thread = process.new_thread(tid).data(thread_data).build();
    add_thread_to_table(&thread);
    new_task.init_task_ext(TaskExt::new(thread));
    Ok(axtask::spawn_task(new_task))
}

enum ExecType {
    Elf,
    Shebang(&'static str),
    Shell,
}

/// execve
/// mainly from starry
/// **Return**
/// - `Ok(handler)` if exec successfully, call handler to enter task.
/// - `Err(AxError)` if exec failed
///
pub fn exec_current(program_name: &str, args: &[String], envs: &[String]) -> AxResult<!> {
    warn!(
        "exec: {} with args {:?}, envs {:?}",
        program_name, args, envs
    );
    let mut args_ = vec![];
    let (oldpwd, pwd) = get_pwd_from_envs(envs);
    let mut program_path = if let Some(ref pwd) = pwd {
        pwd.clone() + "/" + program_name
    } else {
        program_name.to_string()
    };

    // 读取文件头部以检测类型
    let mut buffer: [u8; 64] = [0; 64];
    let mut file = axfs::api::File::open(program_path.as_str())?;
    file.read(&mut buffer)?;

    // 确定执行类型（ELF 或 Shell 脚本）
    let exec_type = if buffer.len() >= 4 && buffer[..4] == *b"\x7fELF" {
        ExecType::Elf
    } else if buffer[..2] == [b'#', b'!'] {
        ExecType::Shell
    } else {
        ExecType::Shell
    };

    // 加载 ELF 文件
    let elf_file: OwnedElfFile = match exec_type {
        ExecType::Elf => load_elf_from_disk(&program_path)
            .inspect_err(|err| debug!("load_elf_from_disk failed: {:?}", err))?,
        ExecType::Shell => {
            program_path = "/usr/bin/busybox".to_string();
            args_.push(program_path.clone());
            args_.push("ash".to_string());
            load_elf_from_disk(program_path.as_str()).unwrap()
        }
        _ => {
            unimplemented!()
        }
    };
    args_.extend_from_slice(args);

    let args_: &[String] = args_.as_slice();
    let current_task = current();

    // 检查地址空间是否被多个任务共享
    if Arc::strong_count(&current_task.task_ext().process_data().aspace) != 1 {
        warn!("Address space is shared by multiple tasks, exec is not supported.");
        return Err(AxError::Unsupported);
    }

    // 释放旧的用户地址空间映射
    let mut aspace = current_task.task_ext().process_data().aspace.lock();
    aspace.unmap_user_areas()?;
    axhal::arch::flush_tlb(None);

    // 使用之前定义的 load_elf_to_mem 函数加载 ELF 文件到内存
    let (entry_point, user_stack_base, thread_pointer) =
        load_elf_to_mem(elf_file,&mut aspace, Some(args_), Some(envs))?;

    axhal::arch::flush_tlb(None);

    unsafe { current_task.task_ext().process_data().aspace.force_unlock() };

    // 设置当前任务名称和目录
    current_task.set_name(&program_path);
    if let Some(pwd) = pwd {
        set_current_dir(pwd.as_str())?;
    }

    debug!(
        "exec: enter uspace, entry: {:?}, stack: {:?}",
        entry_point, user_stack_base,
    );

    // 设置用户上下文并进入用户空间
    let mut uctx = UspaceContext::new(entry_point.as_usize(), user_stack_base, 0);
    if let Some(tp) = thread_pointer {
        uctx.set_tp(tp.as_usize());
    }
    unsafe {
        uctx.enter_uspace(
            current_task
                .kernel_stack_top()
                .expect("No kernel stack top"),
        )
    }
}

/*
 *pub fn exec_current(program_name: &str, args: &[String], envs: &[String]) -> AxResult<!> {
 *    warn!(
 *        "exec: {} with args {:?}, envs {:?}",
 *        program_name, args, envs
 *    );
 *    let mut args_ = vec![];
 *    let (oldpwd, pwd) = get_pwd_from_envs(envs);
 *    let mut program_path = if let Some(ref pwd) = pwd {
 *        pwd.clone() + "/" + program_name
 *    } else {
 *        program_name.to_string()
 *    };
 *    // try reading shebang
 *    let mut buffer: [u8; 64] = [0; 64];
 *    let mut file = axfs::api::File::open(program_path.as_str())?;
 *    file.read(&mut buffer)?;
 *
 *    // FIXME: parse shebang
 *    let exec_type = if buffer.len() >= 4 && buffer[..4] == *b"\x7fELF" {
 *        ExecType::Elf
 *    } else if buffer[..2] == [b'#', b'!'] {
 *        // FIXME: read real shabang
 *        ExecType::Shell
 *    } else {
 *        ExecType::Shell
 *    };
 *
 *    let elf_file: OwnedElfFile = match exec_type {
 *        ExecType::Elf => load_elf_from_disk(&program_path)
 *            .inspect_err(|err| debug!("load_elf_from_disk failed: {:?}", err))?,
 *        ExecType::Shell => {
 *            // try reading shebang
 *            //debug!("execve:{:?} starts with shebang #!...", program_name);
 *            program_path = "/usr/bin/busybox".parse().unwrap(); // busybox
 *
 *            args_.push(program_path.clone().into());
 *            args_.push("ash".into());
 *
 *            load_elf_from_disk(program_path.as_str()).unwrap()
 *        }
 *        _ => {
 *            unimplemented!()
 *        }
 *    };
 *    args_.extend_from_slice(args);
 *
 *    let args_: &[String] = args_.as_slice();
 *    let current_task = current();
 *
 *    if Arc::strong_count(&current_task.task_ext().process_data().aspace) != 1 {
 *        warn!("Address space is shared by multiple tasks, exec is not supported.");
 *        return Err(AxError::Unsupported);
 *    }
 *    let mut aspace = current_task.task_ext().process_data().aspace.lock();
 *    let elf_info = ELFInfo::new(elf_file, aspace.base())?;
 *    aspace.unmap_user_areas()?;
 *    axhal::arch::flush_tlb(None);
 *
 *    //TODO: clone envs??
 *    let (entry_point, user_stack_base, thread_pointer) =
 *        map_elf_sections(elf_info, &mut aspace, Some(args_), Some(envs))?;
 *
 *    unsafe { current_task.task_ext().process_data().aspace.force_unlock() };
 *
 *    current_task.set_name(&program_path);
 *    if let Some(pwd) = pwd {
 *        set_current_dir(pwd.as_str())?;
 *    }
 *
 *    debug!(
 *        "exec: enter uspace, entry: {:?}, stack: {:?}",
 *        entry_point, user_stack_base,
 *    );
 *
 *    let mut uctx = UspaceContext::new(entry_point.as_usize(), user_stack_base, 0);
 *    if let Some(tp) = thread_pointer {
 *        uctx.set_tp(tp.as_usize());
 *    }
 *    unsafe {
 *        uctx.enter_uspace(
 *            current_task
 *                .kernel_stack_top()
 *                .expect("No kernel stack top"),
 *        )
 *    }
 *}
 */
