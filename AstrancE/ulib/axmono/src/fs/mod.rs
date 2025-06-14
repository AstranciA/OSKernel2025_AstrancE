// smaps.rs (或者您希望放置此逻辑的文件)

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use axfs::{
    PROC_ROOT, VfsError, VfsResult,
    proc::{ProcDir, ProcDirGenerator, ProcEntry, ProcFileGenerator},
};
use axprocess::Process;
use axtask::{TaskExtRef, current};
use core::fmt::Write;
use memory_addr::PAGE_SIZE_4K;

use crate::task::{PROCESS_TABLE, ProcessData};

/// 此函数在内部被文件生成器调用，它会一次性创建所有数据，
fn generate_smaps_content(process: Arc<Process>) -> VfsResult<String> {
    let aspace = process
        .data::<ProcessData>()
        .ok_or(VfsError::InvalidData)?
        .aspace
        .lock();
    let mut output = String::new();

    for area in aspace.areas.iter() {
        let stats = area.stat();

        // 使用 `?` 传播 `writeln!` 可能返回的 core::fmt::Error，
        // 将其映射到 VfsError::Io。
        writeln!(
            &mut output,
            "{:016x}-{:016x} {} {:08x} 00:00 0                          {}",
            area.start().as_usize(),
            area.end().as_usize(),
            area.flags(),
            0,
            0
        )
        .map_err(|_| VfsError::Io)?;

        writeln!(&mut output, "Size:           {:8} kB", stats.size / 1024)
            .map_err(|_| VfsError::Io)?;
        writeln!(&mut output, "Rss:            {:8} kB", stats.rss / 1024)
            .map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Pss:            {:8} kB", stats.pss() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Shared_Clean:   {:8} kB", stats.shared_clean() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Shared_Dirty:   {:8} kB", stats.shared_dirty() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Private_Clean:  {:8} kB", stats.private_clean() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Private_Dirty:  {:8} kB", stats.private_dirty() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Referenced:     {:8} kB", stats.referenced() / 1024).map_err(|_| VfsError::Io)?;
        // writeln!(&mut output, "Anonymous:      {:8} kB", stats.anonymous() / 1024).map_err(|_| VfsError::Io)?;
        writeln!(&mut output, "Swap:           {:8} kB", stats.swap / 1024)
            .map_err(|_| VfsError::Io)?;
        writeln!(&mut output, "").map_err(|_| VfsError::Io)?;
    }

    Ok(output)
}

/// 为特定进程创建一个流式的 smaps 文件生成器。
///
/// 这个函数返回一个闭包，该闭包符合 `ProcFileGenerator` 的签名，
/// 实现了按需读取文件内容的功能。
fn create_smaps_file_generator(process: Arc<Process>) -> Arc<ProcFileGenerator> {
    Arc::new(move |offset: u64, buf: &mut [u8]| -> VfsResult<usize> {
        // 1. 在需要时动态生成完整内容。
        let full_content = generate_smaps_content(process.clone())?;

        // 2. 计算要从完整内容中复制的数据范围。
        let start = offset as usize;
        if start >= full_content.len() {
            return Ok(0); // 偏移量超出文件末尾，没有数据可读。
        }

        let end = (start + buf.len()).min(full_content.len());
        let slice_to_copy = &full_content[start..end];

        // 3. 将数据片段复制到 VFS 提供的缓冲区中。
        buf[..slice_to_copy.len()].copy_from_slice(slice_to_copy.as_bytes());

        // 4. 返回实际复制的字节数。
        Ok(slice_to_copy.len())
    })
}

/// 创建一个目录生成器，用于在访问 /proc 时动态生成所有进程的目录。
fn create_pid_dir_generator() -> Arc<ProcDirGenerator> {
    Arc::new(|| {
        let process_table = PROCESS_TABLE.read();
        let mut entries = Vec::new();

        for (pid, process) in process_table.iter() {
            // 为每个 PID 创建一个新的 ProcDir 节点
            let pid_dir = ProcDir::new(None);

            // 为该 PID 的 smaps 文件创建类型安全的文件生成器
            let smaps_generator = create_smaps_file_generator(process.clone());
            pid_dir
                .create_dynamic_file("smaps", smaps_generator)
                .expect("Failed to create smaps file in a new proc dir");

            // TODO: 在这里为该 PID 添加其他文件，如 "cmdline", "status" 等。
            // let cmdline_generator = create_cmdline_file_generator(process.clone());
            // pid_dir.create_dynamic_file("cmdline", cmdline_generator).unwrap();

            // 将配置好的 PID 目录添加到要返回的条目列表中
            entries.push((pid.to_string(), ProcEntry::Dir(pid_dir)));
        }
        Ok(entries)
    })
}

/// 初始化 procfs 的 smaps 相关功能。
///
/// 这个函数应该在内核初始化序列中被调用。
pub fn init_fs() -> VfsResult<()> {
    let proc_root = PROC_ROOT.clone();

    // 1. 为 /proc/[pid] 目录结构设置主生成器。
    let pid_generator = create_pid_dir_generator();
    proc_root.add_generator(pid_generator);

    // 2. 专门处理 /proc/self，它代表当前进程。
    let self_dir = proc_root.create_dir("self")?;

    // 为 /proc/self/smaps 创建一个特殊的、符合签名的文件生成器。
    let self_smaps_generator = Arc::new(|offset: u64, buf: &mut [u8]| -> VfsResult<usize> {
        // a. 在文件被读取时，动态获取“当前”正在运行的进程。
        let curr = current();
        let current_process = curr.task_ext().thread.process().clone();

        // b. 生成该进程的 smaps 内容。
        let full_content = generate_smaps_content(current_process)?;

        // c. 计算并复制数据片段到缓冲区，实现流式读取。
        let start = offset as usize;
        if start >= full_content.len() {
            return Ok(0);
        }
        let end = (start + buf.len()).min(full_content.len());
        let slice_to_copy = &full_content[start..end];
        buf[..slice_to_copy.len()].copy_from_slice(slice_to_copy.as_bytes());

        Ok(slice_to_copy.len())
    });

    self_dir.create_dynamic_file("smaps", self_smaps_generator)?;

    // TODO: 在这里为 /proc/self 添加其他文件，如 "cmdline", "status" 等。

    Ok(())
}
