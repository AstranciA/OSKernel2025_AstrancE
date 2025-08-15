// axfs_procfs/src/dir.rs

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};

use axfs_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeAttrX, VfsNodeOps, VfsNodeRef, VfsNodeType};
use axfs_vfs::{VfsError, VfsResult};
use spin::RwLock;

use crate::file::{ProcDynamicFile, ProcFile, ProcFileGenerator};

/// 一个函数，用于动态生成目录条目。
///
/// 当目录被列出 (`read_dir`) 或查找条目时，该函数会被调用。
/// 它应该返回一个 `(名称, ProcEntry)` 元组的向量。
pub type ProcDirGenerator = dyn Fn() -> VfsResult<Vec<(String, ProcEntry)>> + Send + Sync;

/// 表示 procfs 目录中的一个条目。
///
/// 它可以是子目录、静态文件或动态文件。
#[derive(Clone)]
pub enum ProcEntry {
    Dir(Arc<ProcDir>),
    File(Arc<ProcFile>),
    DynamicFile(Arc<ProcDynamicFile>),
}

impl ProcEntry {
    /// 将 procfs 条目转换为通用的 VFS 节点引用。
    pub fn to_vfs_node(&self) -> VfsNodeRef {
        match self {
            ProcEntry::Dir(dir) => dir.clone() as VfsNodeRef,
            ProcEntry::File(file) => file.clone() as VfsNodeRef,
            ProcEntry::DynamicFile(dyn_file) => dyn_file.clone() as VfsNodeRef,
        }
    }
}

/// procfs 中的目录节点。
///
/// `ProcDir` 可以同时包含静态定义的条目（通过 `create_*` 方法添加）
/// 和由一个或多个 `ProcDirGenerator` 动态生成的条目。
pub struct ProcDir {
    this: Weak<ProcDir>,
    parent: RwLock<Weak<dyn VfsNodeOps>>,
    /// 静态定义的子节点。
    children: RwLock<BTreeMap<String, ProcEntry>>,
    /// MODIFIED: 动态生成子节点的函数列表。
    generators: RwLock<Vec<Arc<ProcDirGenerator>>>,
}

impl ProcDir {
    /// 创建一个新的、空的 `ProcDir`。
    pub fn new(parent: Option<Weak<dyn VfsNodeOps>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
            // MODIFIED: 初始化为空的 Vec
            generators: RwLock::new(Vec::new()),
        })
    }

    /// NEW: 为此目录添加一个生成器函数。
    ///
    /// 可以多次调用此方法以添加多个独立的生成器。
    pub fn add_generator(&self, generator: Arc<ProcDirGenerator>) {
        self.generators.write().push(generator);
    }

    /// 设置父目录。在挂载文件系统时调用。
    pub fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// 检查具有给定名称的条目是否存在。
    ///
    /// 这会同时检查静态和所有动态生成器生成的条目。
    pub fn exist(&self, name: &str) -> bool {
        if self.children.read().contains_key(name) {
            return true;
        }
        // MODIFIED: 检查所有生成器
        for generator in self.generators.read().iter() {
            if let Ok(dynamic_children) = generator() {
                if dynamic_children.iter().any(|(n, _)| n == name) {
                    return true;
                }
            }
        }
        false
    }

    /// 在此目录或其子目录中查找条目。
    ///
    /// `path` 可以是单个名称或多组件路径。
    /// 此函数会按顺序搜索静态条目和所有动态生成器。
    pub fn lookup_entry(&self, path: &str) -> VfsResult<ProcEntry> {
        let (name, rest) = split_path(path);

        if name.is_empty() {
            return Ok(ProcEntry::Dir(self.this.upgrade().unwrap()));
        }

        if name == "." {
            return if let Some(rest) = rest {
                self.lookup_entry(rest)
            } else {
                Ok(ProcEntry::Dir(self.this.upgrade().unwrap()))
            };
        }

        if name == ".." {
            log::error!("Not implemented `..` in procfs yet");
            return Err(VfsError::InvalidInput)
        }

        // 1. 首先在静态子节点中查找
        if let Some(entry) = self.children.read().get(name) {
            let entry = entry.clone();
            return if let Some(rest) = rest {
                if let ProcEntry::Dir(dir) = entry {
                    dir.lookup_entry(rest)
                } else {
                    Err(VfsError::NotADirectory)
                }
            } else {
                Ok(entry)
            };
        }

        // 2. 如果静态子节点中没有，则按顺序查询所有生成器
        // MODIFIED: 迭代所有生成器
        for generator in self.generators.read().iter() {
            if let Ok(dynamic_children) = generator() {
                if let Some((_, entry)) = dynamic_children.into_iter().find(|(n, _)| n == name) {
                    // 找到了，现在处理路径的其余部分
                    return if let Some(rest) = rest {
                        if let ProcEntry::Dir(dir) = entry {
                            dir.lookup_entry(rest)
                        } else {
                            Err(VfsError::NotADirectory)
                        }
                    } else {
                        Ok(entry)
                    };
                }
            }
        }

        // 3. 在任何地方都没有找到
        Err(VfsError::NotFound)
    }

    /// 按路径查找子目录并返回它。
    pub fn lookup_dir(&self, path: &str) -> VfsResult<Arc<ProcDir>> {
        match self.lookup_entry(path)? {
            ProcEntry::Dir(dir) => Ok(dir),
            _ => Err(VfsError::NotADirectory),
        }
    }

    /// 在此目录中创建静态文件。
    pub fn create_static_file(&self, name: &str, content: &[u8]) -> VfsResult {
        if self.exist(name) {
            return Err(VfsError::AlreadyExists);
        }
        let file = Arc::new(ProcFile::new(content));
        self.children
            .write()
            .insert(name.into(), ProcEntry::File(file));
        Ok(())
    }

    /// 在此目录中创建动态文件。
    pub fn create_dynamic_file(&self, name: &str, generator: Arc<ProcFileGenerator>) -> VfsResult {
        if self.exist(name) {
            return Err(VfsError::AlreadyExists);
        }
        let dyn_file = Arc::new(ProcDynamicFile::new(generator));
        self.children
            .write()
            .insert(name.into(), ProcEntry::DynamicFile(dyn_file));
        Ok(())
    }

    /// 创建一个静态子目录。
    pub fn create_dir(&self, name: &str) -> VfsResult<Arc<ProcDir>> {
        if self.exist(name) {
            return Err(VfsError::AlreadyExists);
        }
        let dir = ProcDir::new(Some(self.this.clone()));
        self.children
            .write()
            .insert(name.into(), ProcEntry::Dir(dir.clone()));
        Ok(dir)
    }

    /// 从此目录中删除一个静态节点。
    ///
    /// 如果节点是一个非空目录或不存在，则失败。
    /// 此方法不能删除动态生成的节点。
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let entry = children.get(name).ok_or(VfsError::NotFound)?;

        if let ProcEntry::Dir(dir) = entry {
            // MODIFIED: 检查目录是否包含静态子节点或拥有任何生成器
            if !dir.children.read().is_empty() || !dir.generators.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }

        children.remove(name);
        Ok(())
    }
}

impl VfsNodeOps for ProcDir {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_dir(4096, 0))
    }

    fn get_attr_x(&self) -> VfsResult<axfs_vfs::VfsNodeAttrX> {
        Ok(VfsNodeAttrX::new_dir(4096, 0))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let entry = self.lookup_entry(path)?;
        Ok(entry.to_vfs_node())
    }

    fn read_dir(&self, start_idx: usize, dirents: &mut [VfsDirEntry]) -> VfsResult<usize> {
        // MODIFIED: 合并来自所有来源的条目
        let mut all_children = BTreeMap::new();

        // 1. 从所有动态生成器收集条目
        for generator in self.generators.read().iter() {
            if let Ok(dynamic_children) = generator() {
                for (name, entry) in dynamic_children {
                    // 如果名称冲突，后一个生成器的条目会覆盖前一个
                    all_children.insert(name, entry);
                }
            }
        }

        // 2. 获取静态子节点。如果名称冲突，静态条目将覆盖动态条目。
        for (name, entry) in self.children.read().iter() {
            all_children.insert(name.clone(), entry.clone());
        }

        // 3. 填充 dirents 缓冲区，包括 "." 和 ".."
        let mut children_iter = all_children.iter().skip(start_idx.saturating_sub(2));

        let mut count = 0;
        for ent in dirents.iter_mut() {
            let current_idx = start_idx + count;
            match current_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, entry)) = children_iter.next() {
                        let ty = match entry {
                            ProcEntry::Dir(_) => VfsNodeType::Dir,
                            ProcEntry::File(_) | ProcEntry::DynamicFile(_) => VfsNodeType::File,
                        };
                        *ent = VfsDirEntry::new(name, ty);
                    } else {
                        return Ok(count); // 没有更多条目
                    }
                }
            }
            count += 1;
        }
        Ok(count)
    }

    fn create(&self, path: &str, ty: VfsNodeType) -> VfsResult {
        let (name, rest) = split_path(path);

        if let Some(rest) = rest {
            let entry = self.lookup_entry(name)?;
            if let ProcEntry::Dir(dir) = entry {
                dir.create(rest, ty)
            } else {
                Err(VfsError::NotADirectory)
            }
        } else {
            match ty {
                VfsNodeType::Dir => {
                    self.create_dir(name)?;
                    Ok(())
                }
                VfsNodeType::File => {
                    self.create_static_file(name, b"")?;
                    Ok(())
                }
                _ => Err(VfsError::Unsupported),
            }
        }
    }

    axfs_vfs::impl_vfs_dir_default! {}
}

fn split_path(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
