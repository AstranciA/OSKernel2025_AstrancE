use std::sync::Arc;

use axfs_vfs::{VfsError, VfsNodeType, VfsResult};

use crate::*;

#[test]
fn test_procfs() {
    // 初始化ProcFS
    let procfs = ProcFileSystem::new();
    let root = procfs.root_dir_node(); // 内部管理接口
    let vroot = procfs.root_dir();     // VFS只读接口

    // 测试创建静态文件（使用内部接口）
    root.create_static_file("f1", b"hello").unwrap();
    root.create_static_file("f2", b"world").unwrap();
    root.create_dir("foo").unwrap();

    // 测试文件读取（通过VFS接口）
    let f1 = vroot.lookup("f1").unwrap();
    let mut buf = [0u8; 5];
    assert_eq!(f1.read_at(0, &mut buf).unwrap(), 5);
    assert_eq!(&buf, b"hello");

    // 测试目录列表
    let entries = {
        let mut entries = Vec::new();
        let mut buf = [0u8; 1024];
        let mut offset = 0;
        loop {
            let len = vroot.read_dir(offset, &mut buf).unwrap();
            if len == 0 {
                break;
            }
            let dir_entries = &buf[..len];
            for entry in dir_entries.chunks(32) {
                let name = &entry[..entry.iter().position(|&b| b == 0).unwrap_or(32)];
                entries.push(String::from_utf8_lossy(name).into_owned());
            }
            offset += len;
        }
        entries.sort();
        entries
    };
    assert_eq!(entries, ["f1", "f2", "foo"]);

    // 测试路径解析
    assert!(Arc::ptr_eq(
        &vroot.lookup("foo/bar").unwrap(),
        &vroot.lookup("///foo///bar").unwrap()
    ));

    // 测试删除操作（使用内部接口）
    assert_eq!(root.remove_node("f1"), Ok(()));
    assert_eq!(root.remove_node("f2"), Ok(()));
    assert_eq!(root.remove_node("foo"), Ok(()));

    // 验证已清空
    assert!(root.children.read().is_empty());
}

#[test]
fn test_dynamic_file() {
    let procfs = ProcFileSystem::new();
    let root = procfs.root_dir_node(); // 内部管理接口
    let vroot = procfs.root_dir();     // VFS只读接口

    // 创建动态文件（使用内部接口）
    let generator = Arc::new(|offset: u64, buf: &mut [u8]| {
        let content = b"dynamic content";
        let start = offset as usize;
        if start >= content.len() {
            return Ok(0);
        }
        let end = (start + buf.len()).min(content.len());
        buf[..end - start].copy_from_slice(&content[start..end]);
        Ok(end - start)
    }) as Arc<ProcFileGenerator>;

    root.create_dynamic_file("dyn", generator).unwrap();

    // 测试读取（通过VFS接口）
    let dyn_file = vroot.lookup("dyn").unwrap();
    let mut buf = [0u8; 7];
    assert_eq!(dyn_file.read_at(0, &mut buf).unwrap(), 7);
    assert_eq!(&buf, b"dynamic");

    // 测试VFS接口的写入权限
    assert_eq!(
        dyn_file.write_at(0, b"test").err(),
        Some(VfsError::PermissionDenied)
    );
}

#[test]
fn test_error_handling() {
    let procfs = ProcFileSystem::new();
    let root = procfs.root_dir_node();
    let vroot = procfs.root_dir();

    // 重复创建（内部接口）
    root.create_static_file("f1", b"").unwrap();
    assert_eq!(
        root.create_static_file("f1", b"").err(),
        Some(VfsError::AlreadyExists)
    );

    // VFS接口尝试创建文件
    assert_eq!(
        vroot.create("test", VfsNodeType::File).err(),
        Some(VfsError::PermissionDenied)
    );

    // 删除不存在的文件（内部接口）
    assert_eq!(
        root.remove_node("nonexist").err(),
        Some(VfsError::NotFound)
    );

    // 非空目录删除（内部接口）
    root.create_dir("dir").unwrap();
    root.lookup("dir").unwrap()
        .create_static_file("f", b"").unwrap();
    assert_eq!(
        root.remove_node("dir").err(),
        Some(VfsError::DirectoryNotEmpty)
    );

    // 无效路径（VFS接口）
    assert_eq!(
        vroot.lookup("invalid/path").err(),
        Some(VfsError::NotFound)
    );
}


