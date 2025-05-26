//! Low-level filesystem operations.

#[cfg(feature = "myfs")]
pub use crate::dev::Disk;
#[cfg(feature = "myfs")]
pub use crate::fs::myfs::MyFileSystemIf;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use axerrno::{AxError, AxResult, LinuxError, LinuxResult, ax_err, ax_err_type};
use axfs_vfs::{VfsError, VfsNodeRef};
use axio::SeekFrom;
use cap_access::{Cap, WithCap};
use core::ffi::c_int;
use core::fmt;
use spin::Mutex;

/// Alias of [`axfs_vfs::VfsNodeType`].
pub type FileType = axfs_vfs::VfsNodeType;
/// Alias of [`axfs_vfs::VfsDirEntry`].
pub type DirEntry = axfs_vfs::VfsDirEntry;
/// Alias of [`axfs_vfs::VfsNodeAttr`].
pub type FileAttr = axfs_vfs::VfsNodeAttr;
/// Alias of [`axfs_vfs::VfsNodePerm`].
pub type FilePerm = axfs_vfs::VfsNodePerm;

/// An opened file object, with open permissions and a cursor.
pub struct File {
    pub node: WithCap<VfsNodeRef>,
    is_append: bool,
    offset: u64,
    xattrs: Mutex<BTreeMap<String, Vec<u8>>>, // extra attr
}

/// An opened directory object, with open permissions and a cursor for
/// [`read_dir`](Directory::read_dir).
pub struct Directory {
    node: WithCap<VfsNodeRef>,
    entry_idx: usize,
    xattrs: Mutex<BTreeMap<String, Vec<u8>>>, // extra attr
}

/// Options and flags which can be used to configure how a file is opened.
#[derive(Default, Clone)]
pub struct OpenOptions {
    // generic
    read: bool,
    write: bool,
    execute: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    directory: bool,
    // system-specific
    _custom_flags: i32,
    _mode: u32,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    pub const fn new() -> Self {
        Self {
            // generic
            read: false,
            write: false,
            execute: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            directory: false,
            // system-specific
            _custom_flags: 0,
            _mode: 0o666,
        }
    }
    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) {
        self.read = read;
    }
    /// Sets the option for write access.
    pub fn write(&mut self, write: bool) {
        self.write = write;
    }
    /// Sets the option for execute access.
    pub fn execute(&mut self, execute: bool) {
        self.execute = execute;
    }
    /// Sets the option for the append mode.
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    /// Sets the option for truncating a previous file.
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    /// Sets the option to create a new file, or open it if it already exists.
    pub fn create(&mut self, create: bool) {
        self.create = create;
    }
    /// Sets the option to create a new file, failing if it already exists.
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }
    /// Sets the option to open a directory.
    pub fn directory(&mut self, directory: bool) {
        self.directory = directory;
    }
    /// check whether contains directory.
    pub fn has_directory(&self) -> bool {
        self.directory
    }

    /// Sets the create flags.
    pub fn set_create(mut self, create: bool, create_new: bool) -> Self {
        self.create = create;
        self.create_new = create_new;
        self
    }

    /// Sets the read flag.
    pub fn set_read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    /// Sets the write flag.
    pub fn set_write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    const fn is_valid(&self) -> bool {
        if !self.read && !self.write && !self.append && !self.directory {
            return false;
        }
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return false;
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return false;
                }
            }
        }
        true
    }
}

impl File {
    pub fn access_node(&self, cap: Cap) -> AxResult<&VfsNodeRef> {
        self.node.access_or_err(cap, AxError::PermissionDenied)
    }

    fn _open_at(dir: Option<&VfsNodeRef>, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        debug!("open file: {} {:?}", path, opts);
        if !opts.is_valid() {
            return ax_err!(InvalidInput);
        }

        let node_option = crate::root::lookup(dir, path);
        let node = if opts.create || opts.create_new {
            match node_option {
                Ok(node) => {
                    // already exists
                    if opts.create_new {
                        return ax_err!(AlreadyExists);
                    }
                    node
                }
                // not exists, create new
                Err(VfsError::NotFound) => crate::root::create_file(dir, path)?,
                Err(e) => return Err(e),
            }
        } else {
            // just open the existing
            node_option?
        };

        let attr = node.get_attr()?;
        // if attr.is_dir() {
        //     return ax_err!(IsADirectory);
        // }
        let access_cap = opts.into();
        if !perm_to_cap(attr.perm()).contains(access_cap) {
            return ax_err!(PermissionDenied);
        }

        node.open()?;
        if opts.truncate {
            node.truncate(0)?;
        }
        Ok(Self {
            node: WithCap::new(node, access_cap),
            is_append: opts.append,
            offset: 0,
            xattrs: Mutex::new(BTreeMap::new()),
        })
    }

    /// Opens a file at the path relative to the current directory. Returns a
    /// [`File`] object.
    pub fn open(path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_at(None, path, opts)
    }

    pub fn open_at(dir: &VfsNodeRef, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_at(Some(dir), path, opts)
    }

    /// Truncates the file to the specified size.
    pub fn truncate(&self, size: u64) -> AxResult {
        self.access_node(Cap::WRITE)?.truncate(size)?;
        Ok(())
    }

    /// Reads the file at the current position. Returns the number of bytes
    /// read.
    ///
    /// After the read, the cursor will be advanced by the number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        let node = self.access_node(Cap::READ)?;
        let read_len = node.read_at(self.offset, buf)?;
        self.offset += read_len as u64;
        Ok(read_len)
    }

    /// Reads the file at the given position. Returns the number of bytes read.
    ///
    /// It does not update the file cursor.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> AxResult<usize> {
        let node = self.access_node(Cap::READ)?;
        let read_len = node.read_at(offset, buf)?;
        Ok(read_len)
    }

    /// Writes the file at the current position. Returns the number of bytes
    /// written.
    ///
    /// After the write, the cursor will be advanced by the number of bytes
    /// written.
    pub fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        let offset = if self.is_append {
            self.get_attr()?.size()
        } else {
            self.offset
        };
        let node = self.access_node(Cap::WRITE)?;
        let write_len = node.write_at(offset, buf)?;
        self.offset = offset + write_len as u64;
        Ok(write_len)
    }

    /// Writes the file at the given position. Returns the number of bytes
    /// written.
    ///
    /// It does not update the file cursor.
    pub fn write_at(&self, offset: u64, buf: &[u8]) -> AxResult<usize> {
        let node = self.access_node(Cap::WRITE)?;
        let write_len = node.write_at(offset, buf)?;
        Ok(write_len)
    }

    /// Flushes the file, writes all buffered data to the underlying device.
    pub fn flush(&self) -> AxResult {
        self.access_node(Cap::WRITE)?.fsync()?;
        Ok(())
    }

    /// Sets the cursor of the file to the specified offset. Returns the new
    /// position after the seek.
    pub fn seek(&mut self, pos: SeekFrom) -> AxResult<u64> {
        let size = self.get_attr()?.size();
        let new_offset = match pos {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(off) => self.offset.checked_add_signed(off),
            SeekFrom::End(off) => size.checked_add_signed(off),
        }
        .ok_or_else(|| ax_err_type!(InvalidInput))?;
        self.offset = new_offset;
        Ok(new_offset)
    }

    /// Gets the file attributes.
    pub fn get_attr(&self) -> AxResult<FileAttr> {
        self.access_node(Cap::empty())?.get_attr()
    }

    ///Gets the file extra attributes
    pub fn get_xattr(&self, name: &str, buf: &mut [u8], size: usize) -> Result<usize, LinuxError> {
        let xattrs = self.xattrs.lock();
        let val = xattrs.get(name).ok_or(LinuxError::ENODATA)?; // Attribute not found
        if val.len() > size {
            return Err(LinuxError::ERANGE); // Buffer too small
        }
        if val.len() > buf.len() {
            return Err(LinuxError::ERANGE); // Provided buffer slice too small
        }
        // Copy data into the provided buffer
        buf[..val.len()].copy_from_slice(val);
        Ok(val.len())
    }
    pub fn set_xattr(
        &mut self,
        name: &str,
        value: &[u8],
        size: usize,
    ) -> Result<usize, LinuxError> {
        if size > value.len() {
            return Err(LinuxError::EINVAL); // Size exceeds provided buffer length
        }

        let slice = &value[..size]; // Take only the portion up to `size`
        let mut xattrs = self.xattrs.lock();
        xattrs.insert(name.to_string(), slice.to_vec());
        Ok(size)
    }

    //TODO：fix the lxattr

    // pub fn list_xattr(&self, buf: *mut u8) -> Result<usize, i32> {
    //     let file = self;
    //     let xattrs = file.xattrs.lock();
    //     let mut offset = 0;
    //     for key in xattrs.keys() {
    //         let bytes = key.as_bytes();
    //         if offset + bytes.len() + 1 > buf.len() {
    //             return Err(-1)?;
    //         }
    //         buf[offset..offset + bytes.len()].copy_from_slice(bytes);
    //         buf[offset + bytes.len()] = 0;
    //         offset += bytes.len() + 1;
    //     }
    //     Ok(offset)
    // }
    pub fn remove_xattr(&mut self, name: &str) -> Result<usize, LinuxError> {
        let mut xattrs = self.xattrs.lock();
        if xattrs.is_empty() {
            Ok(0) // Success: no attributes exist, treat as removed
        } else if xattrs.remove(name).is_some() {
            Ok(0) // Success: attribute removed
        } else {
            debug!("remove xattr at bottoum is failed");
            Err(LinuxError::ENODATA) // Attribute not found
        }
    }
}

impl Directory {
    fn access_node(&self, cap: Cap) -> AxResult<&VfsNodeRef> {
        self.node.access_or_err(cap, AxError::PermissionDenied)
    }

    fn _open_dir_at(dir: Option<&VfsNodeRef>, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        debug!("open dir: {}", path);
        if !opts.read {
            return ax_err!(InvalidInput);
        }
        if opts.create || opts.create_new || opts.write || opts.append || opts.truncate {
            return ax_err!(InvalidInput);
        }

        let node = crate::root::lookup(dir, path)?;
        let attr = node.get_attr()?;
        if !attr.is_dir() {
            return ax_err!(NotADirectory);
        }
        let access_cap = opts.into();
        let cap = perm_to_cap(attr.perm());
        if !cap.contains(access_cap) {
            return ax_err!(PermissionDenied);
        }

        node.open()?;
        Ok(Self {
            // Here we use `cap` as capability instead of `access_cap` to allow the user to manipulate the directory
            // without explicitly setting [`OpenOptions::execute`], but without requiring execute access even for
            // directories that don't have this permission.
            node: WithCap::new(node, cap),
            entry_idx: 0,
            xattrs: Default::default(),
        })
    }

    fn access_at(&self, path: &str) -> AxResult<Option<&VfsNodeRef>> {
        if path.starts_with('/') {
            Ok(None)
        } else {
            Ok(Some(self.access_node(Cap::EXECUTE)?))
        }
    }

    /// Opens a directory at the path relative to the current directory.
    /// Returns a [`Directory`] object.
    pub fn open_dir(path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_dir_at(None, path, opts)
    }

    /// Opens a directory at the path relative to this directory. Returns a
    /// [`Directory`] object.
    pub fn open_dir_at(&self, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_dir_at(self.access_at(path)?, path, opts)
    }

    /// Opens a file at the path relative to this directory. Returns a [`File`]
    /// object.
    pub fn open_file_at(&self, path: &str, opts: &OpenOptions) -> AxResult<File> {
        File::_open_at(self.access_at(path)?, path, opts)
    }

    /// Creates an empty file at the path relative to this directory.
    pub fn create_file(&self, path: &str) -> AxResult<VfsNodeRef> {
        crate::root::create_file(self.access_at(path)?, path)
    }

    /// Creates an empty directory at the path relative to this directory.
    pub fn create_dir(&self, path: &str) -> AxResult {
        crate::root::create_dir(self.access_at(path)?, path)
    }

    /// Removes a file at the path relative to this directory.
    pub fn remove_file(&self, path: &str) -> AxResult {
        crate::root::remove_file(self.access_at(path)?, path)
    }

    /// Removes a directory at the path relative to this directory.
    pub fn remove_dir(&self, path: &str) -> AxResult {
        crate::root::remove_dir(self.access_at(path)?, path)
    }

    /// Reads directory entries starts from the current position into the
    /// given buffer. Returns the number of entries read.
    ///
    /// After the read, the cursor will be advanced by the number of entries
    /// read.
    pub fn read_dir(&mut self, dirents: &mut [DirEntry]) -> AxResult<usize> {
        let n = self
            .access_node(Cap::READ)?
            .read_dir(self.entry_idx, dirents)?;
        self.entry_idx += n;
        Ok(n)
    }

    /// Rename a file or directory to a new name.
    /// Delete the original file if `old` already exists.
    ///
    /// This only works then the new path is in the same mounted fs.
    pub fn rename(&self, old: &str, new: &str) -> AxResult {
        crate::root::rename(old, new)
    }

    /// Gets the file attributes.
    pub fn get_attr(&self) -> AxResult<FileAttr> {
        self.access_node(Cap::empty())?.get_attr()
    }

    ///Gets the dir extra attributes
    pub fn get_xattr(&self, name: &str, buf: &mut [u8], size: usize) -> Result<usize, LinuxError> {
        let xattrs = self.xattrs.lock();
        let val = xattrs.get(name).ok_or(LinuxError::ENODATA)?; // Attribute not found
        if val.len() > size {
            return Err(LinuxError::ERANGE); // Buffer too small
        }
        if val.len() > buf.len() {
            return Err(LinuxError::ERANGE); // Provided buffer slice too small
        }
        // Copy data into the provided buffer
        buf[..val.len()].copy_from_slice(val);
        Ok(val.len())
    }
    pub fn set_xattr(
        &mut self,
        name: &str,
        value: &[u8],
        size: usize,
    ) -> Result<usize, LinuxError> {
        if size > value.len() {
            return Err(LinuxError::EINVAL); // Size exceeds provided buffer length
        }

        let slice = &value[..size]; // Take only the portion up to `size`
        let mut xattrs = self.xattrs.lock();
        xattrs.insert(name.to_string(), slice.to_vec());
        Ok(size)
    }

    //TODO：fix the lxattr

    // pub fn list_xattr(&self, buf: *mut u8) -> Result<usize, i32> {
    //     let file = self;
    //     let xattrs = file.xattrs.lock();
    //     let mut offset = 0;
    //     for key in xattrs.keys() {
    //         let bytes = key.as_bytes();
    //         if offset + bytes.len() + 1 > buf.len() {
    //             return Err(-1)?;
    //         }
    //         buf[offset..offset + bytes.len()].copy_from_slice(bytes);
    //         buf[offset + bytes.len()] = 0;
    //         offset += bytes.len() + 1;
    //     }
    //     Ok(offset)
    // }
    pub fn remove_xattr(&mut self, name: &str) -> Result<usize, LinuxError> {
        let mut xattrs = self.xattrs.lock();
        if xattrs.is_empty() {
            Ok(0) // Success: no attributes exist, treat as removed
        } else if xattrs.remove(name).is_some() {
            Ok(0) // Success: attribute removed
        } else {
            debug!("remove xattr at bottoum is failed");
            Err(LinuxError::ENODATA) // Attribute not found
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { self.node.access_unchecked().release().ok() };
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe { self.node.access_unchecked().release().ok() };
    }
}

impl fmt::Debug for OpenOptions {
    #[allow(unused_assignments)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut written = false;
        macro_rules! fmt_opt {
            ($field: ident, $label: literal) => {
                if self.$field {
                    if written {
                        write!(f, " | ")?;
                    }
                    write!(f, $label)?;
                    written = true;
                }
            };
        }
        fmt_opt!(read, "READ");
        fmt_opt!(write, "WRITE");
        fmt_opt!(append, "APPEND");
        fmt_opt!(truncate, "TRUNC");
        fmt_opt!(create, "CREATE");
        fmt_opt!(create_new, "CREATE_NEW");
        Ok(())
    }
}

impl From<&OpenOptions> for Cap {
    fn from(opts: &OpenOptions) -> Cap {
        let mut cap = Cap::empty();
        if opts.read {
            cap |= Cap::READ;
        }
        if opts.write | opts.append {
            cap |= Cap::WRITE;
        }
        if opts.execute {
            cap |= Cap::EXECUTE;
        }
        cap
    }
}

fn perm_to_cap(perm: FilePerm) -> Cap {
    let mut cap = Cap::empty();
    if perm.owner_readable() {
        cap |= Cap::READ;
    }
    if perm.owner_writable() {
        cap |= Cap::WRITE;
    }
    if perm.owner_executable() {
        cap |= Cap::EXECUTE;
    }
    cap
}
