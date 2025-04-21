use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use arceos_posix_api::FD_TABLE;
use axerrno::{AxError, AxResult};
use axfs::{CURRENT_DIR, CURRENT_DIR_PATH};
use axhal::trap::{POST_TRAP, PRE_TRAP, register_trap_handler};
use core::sync::atomic::AtomicUsize;
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU64, Ordering},
};
use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange};
use axfs::api::set_current_dir;
use crate::{
    copy_from_kernel,
    ctypes::{CloneFlags, TimeStat, WaitStatus},
    elf::ELFInfo,
    loader::load_app_from_disk,
    mm::{load_elf_to_mem, map_elf_sections},
};

use axhal::{
    arch::{TrapFrame, UspaceContext},
    time::{NANOS_PER_MICROS, NANOS_PER_SEC, monotonic_time_nanos},
};
use axmm::heap::HeapSpace;
use axmm::{AddrSpace, kernel_aspace};
use axns::{AxNamespace, AxNamespaceIf};
use axsync::Mutex;
use axtask::{AxTaskRef, TaskExtMut, TaskExtRef, TaskInner, WeakAxTaskRef, current};

pub fn new_user_aspace_empty() -> AxResult<AddrSpace> {
    /*
     *AddrSpace::new_empty(
     *    VirtAddr::from_usize(config::USER_SPACE_BASE),
     *    config::USER_SPACE_SIZE,
     *)
     */
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
) -> TaskInner {
    let mut task = TaskInner::new(
        move || {
            // TODO: no current
            let curr = axtask::current();
            let kstack_top = curr.kernel_stack_top().unwrap();
            trace!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                curr.task_ext().uctx.get_ip(),
                curr.task_ext().uctx.get_sp(),
                kstack_top,
            );
            // FIXME:
            set_current_dir("/musl/basic");
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
) -> AxTaskRef {
    spawn_user_task_inner(app_name, aspace, uctx).into_arc()
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
pub fn fork(current_task: AxTaskRef, from_umode: bool) -> AxResult<AxTaskRef> {
    clone_task(current_task, None, CloneFlags::FORK, from_umode)
}

pub fn clone_task(
    current_task: AxTaskRef,
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

    let new_task_ref = spawn_user_task(
        current_task.name(),
        Arc::new(Mutex::new(new_aspace)),
        new_uctx,
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
pub fn exec_current(program_name: &str) -> AxResult<()> {
    debug!("exec: {}", program_name);

    let current_task = current();
    let program_path = program_name.to_string();

    let mut aspace = current_task.task_ext().aspace.lock();

    if Arc::strong_count(&current_task.task_ext().aspace) != 1 {
        warn!("Address space is shared by multiple tasks, exec is not supported.");
        return Err(AxError::Unsupported);
    }

    aspace.unmap_user_areas()?;
    axhal::arch::flush_tlb(None);

    let elf_file = load_app_from_disk(&program_path).map_err(|_| {
        error!("Failed to load app {}", program_path);
        AxError::NotFound
    })?;
    let elf_info = ELFInfo::new(elf_file, aspace.base());

    current_task.set_name(&program_path);

    //TODO: clone envs
    let (entry_point, user_stack_base) =
        map_elf_sections(elf_info, &mut aspace, Some(&[program_path]), None)?;

    let task_ext = unsafe { &mut *(current_task.task_ext_ptr() as *mut TaskExt) };
    task_ext.uctx = UspaceContext::new(entry_point.as_usize(), user_stack_base, 0);

    unsafe { current_task.task_ext().aspace.force_unlock() };

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
fn pre_trap_handler() -> bool {
    time_stat_from_user_to_kernel();
    true
}

#[register_trap_handler(POST_TRAP)]
fn post_trap_handler() -> bool {
    time_stat_from_kernel_to_user();
    true
}
