use alloc::string::{String, ToString};
use alloc::sync::Arc;
use axfs::CURRENT_DIR;
use axfs::api::{DirEntry, create_dir, read_dir, remove_file};
use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeType};
use core::ffi::{c_char, c_int, c_void};
use core::panic;
use static_assertions::assert_eq_size;

use axerrno::{LinuxError, LinuxResult};
use axfs::fops::OpenOptions;
use axio::{PollState, SeekFrom};
use axsync::Mutex;

use super::fd_ops::{FileLike, get_file_like};
use crate::AT_FDCWD;
use crate::ctypes::__IncompleteArrayField;
use crate::utils::str_to_cstr;
use crate::{ctypes, utils::char_ptr_to_str};

/// File wrapper for `axfs::fops::File`.
pub struct File {
    inner: Mutex<axfs::fops::File>,
    path: String,
}

impl File {
    fn new(inner: axfs::fops::File, path: String) -> Self {
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    fn add_to_fd_table(self) -> LinuxResult<c_int> {
        super::fd_ops::add_file_like(Arc::new(self))
    }

    fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        let f = super::fd_ops::get_file_like(fd)?;
        f.into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    /// Get the path of the file.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get the inner node of the file.    
    pub fn inner(&self) -> &Mutex<axfs::fops::File> {
        &self.inner
    }
}

impl FileLike for File {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(self.inner.lock().read(buf)?)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        Ok(self.inner.lock().write(buf)?)
    }

    fn stat(&self) -> LinuxResult<ctypes::stat> {
        let metadata = self.inner.lock().get_attr()?;
        Ok(attr2stat(metadata))
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: true,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}

fn attr2stat(metadata: VfsNodeAttr) -> ctypes::stat {
    let ty = metadata.file_type() as u8;
    let perm = metadata.perm().bits() as u32;
    let st_mode = ((ty as u32) << 12) | perm;
    ctypes::stat {
        st_ino: 1,
        st_nlink: 1,
        st_mode,
        st_uid: 1000,
        st_gid: 1000,
        st_size: metadata.size() as _,
        st_blocks: metadata.blocks() as _,
        st_blksize: 512,
        ..Default::default()
    }
}

/// Convert open flags to [`OpenOptions`].
fn flags_to_options(flags: c_int, _mode: ctypes::mode_t) -> OpenOptions {
    let flags = flags as u32;
    let mut options = OpenOptions::new();
    match flags & 0b11 {
        ctypes::O_RDONLY => options.read(true),
        ctypes::O_WRONLY => options.write(true),
        _ => {
            options.read(true);
            options.write(true);
        }
    };
    if flags & ctypes::O_APPEND != 0 {
        options.append(true);
    }
    if flags & ctypes::O_TRUNC != 0 {
        options.truncate(true);
    }
    if flags & ctypes::O_CREAT != 0 {
        options.create(true);
    }
    if flags & ctypes::O_EXEC != 0 {
        //options.create_new(true);
        options.execute(true);
    }
    if flags & ctypes::O_DIRECTORY != 0 {
        options.directory(true);
    }
    options
}

/// Open a file by `filename` and insert it into the file descriptor table.
///
/// Return its index in the file table (`fd`). Return `EMFILE` if it already
/// has the maximum number of files open.
pub fn sys_open(filename: *const c_char, flags: c_int, mode: ctypes::mode_t) -> c_int {
    let filename = char_ptr_to_str(filename);
    debug!("sys_open <= {:?} {:#o} {:#o}", filename, flags, mode);
    syscall_body!(sys_open, {
        add_file_or_directory_fd(
            axfs::fops::File::open,
            axfs::fops::Directory::open_dir,
            filename?,
            &flags_to_options(flags, mode),
        )
    })
}

/// Open or create a file.
/// fd: file descriptor
/// filename: file path to be opened or created
/// flags: open flags
/// mode: see man 7 inode
/// return new file descriptor if succeed, or return -1.
pub fn sys_openat(
    dirfd: c_int,
    filename: *const c_char,
    flags: c_int,
    mode: ctypes::mode_t,
) -> c_int {
    let filename = match char_ptr_to_str(filename) {
        Ok(s) => s,
        Err(_) => return LinuxError::EFAULT as c_int,
    };

    debug!(
        "sys_openat <= {} {:?} {:#o} {:#o}",
        dirfd, filename, flags, mode
    );

    if filename.starts_with('/') || dirfd == AT_FDCWD as _ {
        return sys_open(filename.as_ptr() as _, flags, mode);
    }

    Directory::from_fd(dirfd)
        .and_then(|dir| {
            add_file_or_directory_fd(
                |filename, options| dir.inner.lock().open_file_at(filename, options),
                |filename, options| dir.inner.lock().open_dir_at(filename, options),
                filename,
                &flags_to_options(flags, mode),
            )
        })
        .unwrap_or_else(|e| {
            debug!("sys_openat => {}", e);
            -1
        })
}

/// Create a directory by `dirname` relatively to `dirfd`.
/// TODO: handle `mode`
pub fn sys_mkdirat(dirfd: c_int, dirname: *const c_char, mode: ctypes::mode_t) -> c_int {
    let dirname = match char_ptr_to_str(dirname) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    debug!("sys_mkdirat <= {} {:?} {:#o}", dirfd, dirname, mode);

    if dirname.starts_with('/') || dirfd == AT_FDCWD as _ {
        return create_dir(dirname).and(Ok(0)).unwrap_or_else(|e| {
            debug!("sys_mkdirat => {}", e);
            -1
        });
    }

    Directory::from_fd(dirfd)
        .and_then(|dir| {
            dir.inner.lock().create_dir(dirname);
            Ok(0)
        })
        .unwrap_or_else(|e| {
            debug!("sys_mkdirat => {}", e);
            -1
        })
}

/// Create a directory by `dirname` relatively to `dirfd`.
/// TODO: handle `mode`
pub unsafe fn sys_fstatat(
    dirfd: c_int,
    pathname_p: *const c_char,
    statbuf: *mut ctypes::stat,
    flags: c_int,
) -> LinuxResult<c_int> {
    let pathname = char_ptr_to_str(pathname_p)?;

    debug!(
        "sys_fstatat <= {} {pathname_p:p} {:?} {:#o}",
        dirfd, pathname, flags
    );
    debug!("{:?}", unsafe {
        core::slice::from_raw_parts(pathname_p, 20)
    });
    static mut IDX: usize = 0;
    unsafe {
        if IDX == 7 {
            //panic!()
        }
        IDX += 1;
    }

    if pathname.starts_with('/') || dirfd == AT_FDCWD as _ {
        let dir = CURRENT_DIR.lock().clone();
        let file = dir.lookup(pathname)?;
        let stat = attr2stat(file.get_attr()?);
        unsafe { *statbuf = stat };
        return Ok(0);
    }

    let dir: Arc<Directory> = Directory::from_fd(dirfd)?;
    // FIXME: correct path; flags
    let file: File = File::new(
        dir.inner
            .lock()
            .open_file_at(pathname, &flags_to_options(flags, 0))?,
        pathname.into(),
    );
    let stat = file.stat()?;
    unsafe { *statbuf = stat };
    Ok(0)
}

/// Use the function to open file or directory, then add into file descriptor table.
/// First try opening files, if fails, try directory.
fn add_file_or_directory_fd<F, D, E>(
    open_file: F,
    open_dir: D,
    filename: &str,
    options: &OpenOptions,
) -> LinuxResult<c_int>
where
    E: Into<LinuxError>,
    F: FnOnce(&str, &OpenOptions) -> Result<axfs::fops::File, E>,
    D: FnOnce(&str, &OpenOptions) -> Result<axfs::fops::Directory, E>,
{
    if !options.has_directory() {
        match open_file(filename, options)
            .map_err(Into::into)
            .and_then(|f| File::new(f, filename.into()).add_to_fd_table())
        {
            Err(LinuxError::EISDIR) => {}
            r => return r,
        }
    }

    Directory::new(
        open_dir(filename, options).map_err(Into::into)?,
        filename.to_string(),
    )
    .add_to_fd_table()
}

/// Set the position of the file indicated by `fd`.
///
/// Return its position after seek.
pub fn sys_lseek(fd: c_int, offset: ctypes::off_t, whence: c_int) -> ctypes::off_t {
    debug!("sys_lseek <= {} {} {}", fd, offset, whence);
    syscall_body!(sys_lseek, {
        let pos = match whence {
            0 => SeekFrom::Start(offset as _),
            1 => SeekFrom::Current(offset as _),
            2 => SeekFrom::End(offset as _),
            _ => return Err(LinuxError::EINVAL),
        };
        let off = File::from_fd(fd)?.inner.lock().seek(pos)?;
        Ok(off)
    })
}

/// Get the file metadata by `path` and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_stat(path: *const c_char, buf: *mut ctypes::stat) -> c_int {
    let path = char_ptr_to_str(path);
    debug!("sys_stat <= {:?} {:#x}", path, buf as usize);
    syscall_body!(sys_stat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }
        let mut options = OpenOptions::new();
        options.read(true);
        let file = axfs::fops::File::open(path?, &options)?;
        let st = File::new(file, path?.to_string()).stat()?;
        unsafe { *buf = st };
        Ok(0)
    })
}

/// Get file metadata by `fd` and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_fstat(fd: c_int, buf: *mut ctypes::stat) -> c_int {
    debug!("sys_fstat <= {} {:#x}", fd, buf as usize);
    syscall_body!(sys_fstat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }

        unsafe { *buf = get_file_like(fd)?.stat()? };
        Ok(0)
    })
}

/// Get the metadata of the symbolic link and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_lstat(path: *const c_char, buf: *mut ctypes::stat) -> ctypes::ssize_t {
    let path = char_ptr_to_str(path);
    debug!("sys_lstat <= {:?} {:#x}", path, buf as usize);
    syscall_body!(sys_lstat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }
        unsafe { *buf = Default::default() }; // TODO
        Ok(0)
    })
}

/// Get the path of the current directory.
pub fn sys_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    debug!("sys_getcwd <= {:#x} {}", buf as usize, size);
    syscall_body!(sys_getcwd, {
        if buf.is_null() {
            return Ok(core::ptr::null::<c_char>() as _);
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, size as _) };
        let cwd = axfs::api::current_dir()?;
        let cwd = cwd.as_bytes();
        if cwd.len() < size {
            dst[..cwd.len()].copy_from_slice(cwd);
            dst[cwd.len()] = 0;
            Ok(buf)
        } else {
            Err(LinuxError::ERANGE)
        }
    })
}

/// Rename `old` to `new`
/// If new exists, it is first removed.
///
/// Return 0 if the operation succeeds, otherwise return -1.
pub fn sys_rename(old: *const c_char, new: *const c_char) -> c_int {
    syscall_body!(sys_rename, {
        let old_path = char_ptr_to_str(old)?;
        let new_path = char_ptr_to_str(new)?;
        debug!("sys_rename <= old: {:?}, new: {:?}", old_path, new_path);
        axfs::api::rename(old_path, new_path)?;
        Ok(0)
    })
}

/// Directory wrapper for `axfs::fops::Directory`.
pub struct Directory {
    inner: Mutex<axfs::fops::Directory>,
    path: String,
}

impl Directory {
    fn new(inner: axfs::fops::Directory, path: String) -> Self {
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    fn add_to_fd_table(self) -> LinuxResult<c_int> {
        super::fd_ops::add_file_like(Arc::new(self))
    }

    /// Open a directory by `fd`.
    pub fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        let f = super::fd_ops::get_file_like(fd)?;
        f.into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    /// Get the path of the directory.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl FileLike for Directory {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    /*
     *fn stat(&self) -> LinuxResult<ctypes::stat> {
     *    Err(LinuxError::EBADF)
     *}
     */

    fn stat(&self) -> LinuxResult<ctypes::stat> {
        let metadata = self.inner.lock().get_attr()?;
        let ty = metadata.file_type() as u8;
        let perm = metadata.perm().bits() as u32;
        let st_mode = ((ty as u32) << 12) | perm;
        Ok(ctypes::stat {
            st_ino: 1,
            st_nlink: 2,
            st_mode,
            st_uid: 1000,
            st_gid: 1000,
            st_size: metadata.size() as _,
            //st_blocks: metadata.blocks() as _,
            st_blocks: 1,
            st_blksize: 512,
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: false,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}

/*
 *pub unsafe fn sys_getdents(
 *    dir_fd: i32,
 *    buf: *mut ctypes::dirent,
 *    count: c_int,
 *) -> LinuxResult<isize> {
 *    let dir: Arc<Directory> = Directory::from_fd(dir_fd)?;
 *    let mut curr_dent = buf;
 *    let count = count.try_into().map_err(|_| LinuxError::EINVAL)?;
 *    let mut inner = dir.inner.lock();
 *    let end = (buf as *const u8).wrapping_add(count);
 *    let dirent_size = core::mem::size_of::<ctypes::dirent>();
 *    // TODO: support file name longer than 64 bytes
 *    // 64 : sizeof [char; 64];
 *    let mut nread = 0;
 *    while (curr_dent as *const u8).wrapping_add(dirent_size + 64) < end {
 *        let mut dirent_buf = [VfsDirEntry::default()];
 *        match inner.read_dir(&mut dirent_buf) {
 *            Ok(n) if n == 0 => break,
 *            Ok(n) => nread += n,
 *            Err(_) => break,
 *        }
 *        let name = dirent_buf[0].name_as_bytes();
 *        let name = unsafe { String::from_utf8_lossy(name) };
 *        assert!(name.len() < 64);
 *        let d_reclen = core::mem::size_of::<ctypes::dirent>() + name.len() + 1;
 *        unsafe {
 *            *curr_dent = ctypes::dirent {
 *                d_ino: 1,
 *                d_off: 0,
 *                d_reclen: d_reclen as u16,
 *                d_type: dirent_buf[0].entry_type() as u8,
 *                d_name: __IncompleteArrayField::<c_char>::new(),
 *            };
 *            let mut name_ptr = (curr_dent as *mut c_char).wrapping_add(19); // offset of d_name in dirent
 *            let str_len = str_to_cstr(&name, name_ptr);
 *            // FIXME: align struct??
 *            curr_dent = name_ptr.wrapping_add(str_len) as *mut _;
 *        };
 *        // cut off d_name at `\0`
 *    }
 *
 *    return Ok(nread as isize);
 *}
 */
pub unsafe fn sys_getdents(
    dir_fd: i32,
    buf: *mut ctypes::dirent,
    count: c_int,
) -> LinuxResult<isize> {
    const MAX_NAME_LEN: usize = 255; // Linux NAME_MAX
    const DIRENT_MIN_SIZE: usize = core::mem::size_of::<ctypes::dirent>();

    let dir = Directory::from_fd(dir_fd)?;
    let mut inner = dir.inner.lock();

    let buf_start = buf as *const u8;
    let buf_end = buf_start.wrapping_add(count as usize);
    let mut curr_ptr = buf as *mut u8;
    let mut entries_written = 0;

    // Temporary buffer for directory entries
    let mut dirent_buf = [VfsDirEntry::default(); 1];

    loop {
        // Check remaining space (need space for struct + name + null terminator)
        let remaining = buf_end as usize - curr_ptr as usize;
        if remaining < DIRENT_MIN_SIZE + MAX_NAME_LEN + 1 {
            break;
        }

        // Read next directory entry
        match inner.read_dir(&mut dirent_buf) {
            Ok(0) => break, // No more entries
            Ok(_) => (),
            Err(e) => {
                if entries_written == 0 {
                    return Err(e.into());
                }
                break;
            }
        }

        let entry = &dirent_buf[0];
        let name = entry.name_as_bytes();
        let name_len = name.len().min(MAX_NAME_LEN);

        // Calculate required space
        let reclen = core::mem::align_of::<ctypes::dirent>()
            .max(8)
            .max(DIRENT_MIN_SIZE + name_len + 1);

        if (curr_ptr as usize + reclen) > buf_end as usize {
            break;
        }

        // Fill dirent structure
        let dirent = curr_ptr as *mut ctypes::dirent;
        unsafe {
            (*dirent).d_ino = 1;
            (*dirent).d_off = 0;
            (*dirent).d_reclen = reclen as u16;
            (*dirent).d_type = entry.entry_type() as u8;

            // Copy name (including null terminator)
            let name_dst = (*dirent).d_name.as_mut_ptr();
            core::ptr::copy_nonoverlapping(name.as_ptr(), name_dst, name_len);
            *name_dst.add(name_len) = 0;

            curr_ptr = curr_ptr.add(reclen);
        }
        entries_written += 1;
    }

    Ok(if entries_written > 0 {
        (curr_ptr as usize - buf_start as usize) as isize
    } else {
        0
    })
}

pub fn sys_unlink(path: *const c_char) -> LinuxResult<isize> {
    let path = char_ptr_to_str(path).map_err(|_| LinuxError::EFAULT)?;
    warn!("sys_unlink <= {:?}", path);
    remove_file(path)?;
    warn!("sys_unlink <= {:?}", path);
    Ok(0)
}
pub fn sys_unlinkat(dir_fd: i32, path: *const c_char) -> LinuxResult<isize> {
    if dir_fd < 0 {
        return sys_unlink(path);
    }
    let dir: Arc<Directory> = Directory::from_fd(dir_fd)?;
    let path = char_ptr_to_str(path).map_err(|_| LinuxError::EFAULT)?;
    dir.inner.lock().remove_file(path)?;
    Ok(0)
}
