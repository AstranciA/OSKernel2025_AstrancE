use crate::{
    copy_from_kernel,
    ctypes::{CloneFlags, TimeStat, WaitStatus},
    elf::ELFInfo,
    loader::load_elf_from_disk,
    mm::map_elf_sections,
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use axio;
use arceos_posix_api::FD_TABLE;
use axerrno::{AxError, AxResult};
use axfs::api::{current_dir, read, set_current_dir};
use axfs::{CURRENT_DIR, CURRENT_DIR_PATH};
use axhal::trap::{POST_TRAP, PRE_TRAP, register_trap_handler};
#[cfg(feature = "sig")]
use signal::SignalContext;
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU64, Ordering},
};
use axio::Read;
use xmas_elf::symbol_table::Type::File;
use memory_addr::{VirtAddr, VirtAddrRange};

use axhal::{
    arch::{TrapFrame, UspaceContext},
    time::{monotonic_time_nanos, NANOS_PER_MICROS, NANOS_PER_SEC},
};
use axmm::heap::HeapSpace;
use axmm::{AddrSpace, kernel_aspace};
use axns::{AxNamespace, AxNamespaceIf};
use axsync::Mutex;
use axtask::{current, AxTaskRef, TaskExtRef, TaskInner};
use crate::mm::load_elf_to_mem;

#[cfg(feature = "sig")]
mod signal;

pub fn new_user_aspace_empty() -> AxResult<AddrSpace> {
    AddrSpace::new_empty(
        VirtAddr::from_usize(axconfig::plat::USER_SPACE_BASE),
        axconfig::plat::USER_SPACE_SIZE,
    )
}

/// Task extended data for the monolithic kernel.
pub struct TaskExt {
    /// The process ID.
    pub proc_id: usize,
    /// The parent process ID.
    pub parent_id: AtomicU64,
    /// children process
    pub children: Mutex<Vec<AxTaskRef>>,
    /// The clear thread tid field
    ///
    /// See <https://manpages.debian.org/unstable/manpages-dev/set_tid_address.2.en.html#clear_child_tid>
    ///
    /// When the thread exits, the kernel clears the word at this address if it is not NULL.
    clear_child_tid: AtomicU64,
    /// The user space context.
    pub uctx: UspaceContext,
    /// The virtual memory address space.
    pub aspace: Arc<Mutex<AddrSpace>>,
    /// The resource namespace
    pub ns: AxNamespace,
    /// The time statistics
    pub time: UnsafeCell<TimeStat>,
    #[cfg(feature = "sig")]
    pub sigctx: SignalContext,
}

#[allow(unused)]
impl TaskExt {
    pub fn new(proc_id: usize, uctx: UspaceContext, aspace: Arc<Mutex<AddrSpace>>) -> Self {
        Self {
            proc_id,
            parent_id: AtomicU64::new(1),
            children: Mutex::new(Vec::new()),
            uctx,
            clear_child_tid: AtomicU64::new(0),
            aspace,
            ns: AxNamespace::new_thread_local(false),
            time: TimeStat::new().into(),
            #[cfg(feature = "sig")]
            sigctx: SignalContext::default()
        }
    }

    pub(crate) fn clear_child_tid(&self) -> u64 {
        self.clear_child_tid
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn set_clear_child_tid(&self, clear_child_tid: u64) {
        self.clear_child_tid
            .store(clear_child_tid, core::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn get_parent(&self) -> u64 {
        self.parent_id.load(Ordering::Acquire)
    }

    pub(crate) fn set_parent(&self, parent_id: u64) {
        self.parent_id.store(parent_id, Ordering::Release);
    }

    /// Initialize the namespace for the new task.
    /// TODO: from parent task
    pub(crate) fn ns_init_new(&self) {
        FD_TABLE.create(&self.ns).init_new(FD_TABLE.copy_inner());
        CURRENT_DIR
            .create(&self.ns)
            .init_new(CURRENT_DIR.copy_inner());
        CURRENT_DIR_PATH
            .create(&self.ns)
            .init_new(CURRENT_DIR_PATH.copy_inner());
    }

    pub(crate) fn time_stat_from_kernel_to_user(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_into_user_mode(current_tick);
        }
    }

    pub(crate) fn time_stat_from_user_to_kernel(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_into_kernel_mode(current_tick);
        }
    }

    pub(crate) fn time_stat_when_switch_from(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_from_old_task(current_tick);
        }
    }

    pub(crate) fn time_stat_when_switch_to(&self, current_tick: usize) {
        let time = self.time.get();
        unsafe {
            (*time).switch_to_new_task(current_tick);
        }
    }

    pub(crate) fn time_stat_output(&self) -> (usize, usize) {
        let time = self.time.get();
        unsafe { (*time).output() }
    }

    pub(crate) fn set_heap_top(&self, top: VirtAddr) -> VirtAddr {
        self.aspace.lock().set_heap_top(top)
    }

    pub(crate) fn set_heap_size(&self, size: usize) -> VirtAddr {
        self.aspace.lock().set_heap_size(size)
    }

    pub(crate) fn heap_size(&self) -> usize {
        self.aspace.lock().heap().size()
    }

    pub(crate) fn heap_top(&self) -> VirtAddr {
        self.aspace.lock().heap().top()
    }
}

impl Drop for TaskExt {
    fn drop(&mut self) {
        if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
            // See [`crate::new_user_aspace`]

            debug!("Drop TaskExt: {}", self.proc_id);

            let kernel = kernel_aspace().lock();

            self.aspace
                .lock()
                .clear_mappings(VirtAddrRange::from_start_size(kernel.base(), kernel.size()));
        }
    }
}

struct AxNamespaceImpl;

#[crate_interface::impl_interface]
impl AxNamespaceIf for AxNamespaceImpl {
    #[inline(never)]
    fn current_namespace_base() -> *mut u8 {
        let current = axtask::current();
        /*
         *println!("current_namespace_base: {:p}", unsafe {
         *    current.task_ext_ptr()
         *});
         */
        // Safety: We only check whether the task extended data is null and do not access it.
        if unsafe { current.task_ext_ptr() }.is_null() {
            return axns::AxNamespace::global().base();
        }
        current.task_ext().ns.base()
    }
}

axtask::def_task_ext!(TaskExt);

pub fn spawn_user_task_inner(
    app_name: &str,
    aspace: Arc<Mutex<AddrSpace>>,
    uctx: UspaceContext,
    pwd:String,
) -> TaskInner {
    let mut task = TaskInner::new(
        move || {
            // TODO: no current
            let curr = axtask::current();
            let kstack_top = curr.kernel_stack_top().unwrap();
            error!("tp:{:?}", curr.task_ext().uctx.0.regs.tp);
            trace!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                curr.task_ext().uctx.get_ip(),
                curr.task_ext().uctx.get_sp(),
                kstack_top,
            );
            // FIXME:
            set_current_dir(pwd.as_str()).unwrap();
            unsafe { curr.task_ext().uctx.enter_uspace(kstack_top) };
        },
        app_name.into(),
        axconfig::plat::KERNEL_STACK_SIZE,
    );
    task.ctx_mut()
        .set_page_table_root(aspace.lock().page_table_root());

    task.init_task_ext(TaskExt::new(task.id().as_u64() as usize, uctx, aspace));

    // TODO:
    task.task_ext().ns_init_new();
    task
}

pub fn spawn_user_task(
    app_name: &str,
    aspace: Arc<Mutex<AddrSpace>>,
    uctx: UspaceContext,
    pwd:String,
) -> AxTaskRef {
    spawn_user_task_inner(app_name, aspace, uctx,pwd).into_arc()
    /*
     *let task_inner = spawn_user_task_inner(app_name, aspace, uctx);
     *axtask::spawn_task(task_inner)
     */
}

/// Unable to work for cloned task since task will overwrite trap_frame from uctx
pub fn write_trapframe_to_kstack(kstack_top: usize, trap_frame: &TrapFrame) {
    let trap_frame_size = core::mem::size_of::<TrapFrame>();
    let trap_frame_ptr = (kstack_top - trap_frame_size) as *mut TrapFrame;
    unsafe {
        *trap_frame_ptr = *trap_frame;
    }
}

pub fn read_trapframe_from_kstack(kstack_top: usize) -> TrapFrame {
    let trap_frame_size = core::mem::size_of::<TrapFrame>();
    let trap_frame_ptr = (kstack_top - trap_frame_size) as *mut TrapFrame;
    unsafe { *trap_frame_ptr }
}

/// From starry-next
pub fn wait_pid(task: AxTaskRef, pid: i32, exit_code_ptr: *mut i32) -> Result<u64, WaitStatus> {
    let mut exit_task_id: usize = 0;
    let mut answer_id: u64 = 0;
    let mut answer_status = WaitStatus::NotExist;

    for (index, child) in task.task_ext().children.lock().iter().enumerate() {
        if pid <= 0 {
            if pid == 0 {
                axlog::warn!("Don't support for process group.");
            }

            answer_status = WaitStatus::Running;
            if child.state() == axtask::TaskState::Exited {
                let exit_code = child.exit_code();
                answer_status = WaitStatus::Exited;
                debug!(
                    "wait pid _{}_ with code _{}_",
                    child.id().as_u64(),
                    exit_code
                );
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = exit_code << 8;
                    }
                }
                answer_id = child.id().as_u64();
                break;
            }
        } else if child.id().as_u64() == pid as u64 {
            if let Some(exit_code) = child.join() {
                answer_status = WaitStatus::Exited;
                info!(
                    "wait pid _{}_ with code _{:?}_",
                    child.id().as_u64(),
                    exit_code
                );
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = exit_code << 8;
                    }
                }
                answer_id = child.id().as_u64();
            } else {
                answer_status = WaitStatus::Running;
            }
            break;
        }
    }

    if answer_status == WaitStatus::Running {
        axtask::yield_now();
    }

    if answer_status == WaitStatus::Exited {
        task.task_ext().children.lock().remove(exit_task_id);
        return Ok(answer_id);
    }
    Err(answer_status)
}

/// fork current task
/// **Return**
/// - `Ok(new_task_ref)` if fork successfully
pub fn fork(from_umode: bool) -> AxResult<AxTaskRef> {
    clone_task(None, CloneFlags::FORK, from_umode)
}

pub fn clone_task(
    stack: Option<usize>,
    clone_flags: CloneFlags,
    from_umode: bool,
    /*
     *_ptid: usize,
     *_tls: usize,
     *_ctid: usize,
     */
) -> AxResult<AxTaskRef> {
    axconfig::plat::KERNEL_STACK_SIZE;
    // TODO: support all flags
    let current_task = current();
    let current_task_ext = current_task.task_ext();
    // new task with same ip and sp of current task
    let mut trap_frame = read_trapframe_from_kstack(current_task.get_kernel_stack_top().unwrap());

    let mut current_aspace = current_task_ext.aspace.lock();
    let mut new_aspace;
    #[cfg(feature = "COW")]
    {
        new_aspace = current_aspace.clone_on_write()?;
    }
    #[cfg(not(feature = "COW"))]
    {
        new_aspace = current_aspace.clone_or_err()?;
    }

    copy_from_kernel(&mut new_aspace);

    //let new_uctx = current_task_ext.uctx.0;

    if from_umode {
        trap_frame.set_ret_code(0);
        trap_frame.inc_sepc();
    }

    // TODO: clone stack since it's always changed.
    // stack is copied meanwhilst addr space is copied
    //trap_frame.set_user_sp(stack);
    if let Some(stack) = stack {
        trap_frame.set_user_sp(stack);
    }

    //write_trapframe_to_kstack(new_task_ref.kernel_stack_top().unwrap().into(), &trap_frame);
    //write_trapframe_to_kstack(new_task_ref.kernel_stack_top().unwrap().into(), &TrapFrame::default());
    //new_uctx.0 = trap_frame;
    let new_uctx = UspaceContext::from(&trap_frame);
    //panic!();
    let current_d = current_dir()?;
    let new_task_ref = spawn_user_task(
        current_task.name(),
        Arc::new(Mutex::new(new_aspace)),
        new_uctx,
        current_d
    );

    // TODO: children task management
    current_task_ext.children.lock().push(new_task_ref.clone());

    Ok(new_task_ref)
}

/// execve
/// mainly from starry
/// **Return**
/// - `Ok(handler)` if exec successfully, call handler to enter task.
/// - `Err(AxError)` if exec failed
pub fn exec_current(program_name: &str, args: &[String], envs: &[String]) -> AxResult<> {
    warn!(
        "exec: {} with args {:?}, envs {:?}",
        program_name, args, envs
    );

    let program_path = program_name.to_string();
    let mut buffer: [u8; 64] = [0; 64];
    let mut file = axfs::api::File::open(program_path.as_str())?;
    file.read(&mut buffer)?;
    if buffer[..2] == [b'#',b'!']{
        debug!("execve:{:?} starts with {:?}", program_name,&buffer[..2] as &[u8]);
        let app_path = "/musl/busybox";
        let (entry_vaddr, user_stack_base, uspace) = load_elf_to_mem(
            load_elf_from_disk(app_path).unwrap(),
            Some(&[app_path.into(),"ash".into(),program_path.into()]),
            None,
        ).unwrap();
        debug!(
        "app_entry: {:?}, app_stack: {:?}, app_aspace: {:?}",
        entry_vaddr,
        user_stack_base,
        uspace,);

        let uctx = UspaceContext::new(entry_vaddr.into(), user_stack_base, 2333);
        let current_d = current_dir()?;
        let user_task = spawn_user_task(app_path, Arc::new(Mutex::new(uspace)), uctx, current_d);

        axtask::spawn_task_by_ref(user_task.clone());

        let exit_code = user_task.join().unwrap();
        info!("app exit with code: {:?}", exit_code);
        return AxResult::Ok(())
    }



    let elf_file = load_elf_from_disk(&program_path)?;

    let current_task = current();

    let mut aspace = current_task.task_ext().aspace.lock();
    let elf_info = ELFInfo::new(elf_file, aspace.base());

    if Arc::strong_count(&current_task.task_ext().aspace) != 1 {
        warn!("Address space is shared by multiple tasks, exec is not supported.");
        return Err(AxError::Unsupported);
    }

    aspace.unmap_user_areas()?;
    axhal::arch::flush_tlb(None);

    //TODO: clone envs??
    let (entry_point, user_stack_base) =
        map_elf_sections(elf_info, &mut aspace, Some(args), Some(envs))?;

    let task_ext = unsafe { &mut *(current_task.task_ext_ptr() as *mut TaskExt) };
    task_ext.uctx = UspaceContext::new(entry_point.as_usize(), user_stack_base, 0);

    unsafe { current_task.task_ext().aspace.force_unlock() };

    current_task.set_name(&program_path);

    unsafe {
        task_ext.uctx.enter_uspace(
            current_task
                .kernel_stack_top()
                .expect("No kernel stack top"),
        )
    }
}

pub fn time_stat_from_kernel_to_user() {
    let curr_task = current();
    if (unsafe { curr_task.task_ext_ptr().is_null() }) {
        return;
    }
    curr_task
        .task_ext()
        .time_stat_from_kernel_to_user(monotonic_time_nanos() as usize);
}

pub fn time_stat_from_user_to_kernel() {
    let curr_task = current();
    if (unsafe { curr_task.task_ext_ptr().is_null() }) {
        return;
    }
    curr_task
        .task_ext()
        .time_stat_from_user_to_kernel(monotonic_time_nanos() as usize);
}

pub fn time_stat_output() -> (usize, usize, usize, usize) {
    let curr_task = current();
    let (utime_ns, stime_ns) = curr_task.task_ext().time_stat_output();
    (
        utime_ns / NANOS_PER_SEC as usize,
        utime_ns / NANOS_PER_MICROS as usize,
        stime_ns / NANOS_PER_SEC as usize,
        stime_ns / NANOS_PER_MICROS as usize,
    )
}

pub fn time_stat_ns() -> (usize, usize) {
    let curr_task = current();
    curr_task.task_ext().time_stat_output()
}

pub fn test(task: AxTaskRef) {
    let task_ext = task.task_ext();
    let mut buf = [0u8; 16];
    task_ext
        .aspace
        .lock()
        .read(0x1161c.into(), &mut buf)
        .unwrap();
}

#[register_trap_handler(PRE_TRAP)]
fn pre_trap_handler(trap_frame: &TrapFrame) -> bool {
    time_stat_from_user_to_kernel();
    true
}

#[register_trap_handler(POST_TRAP)]
fn post_trap_handler(trap_frame: &TrapFrame) -> bool {
    time_stat_from_kernel_to_user();
    true
}
