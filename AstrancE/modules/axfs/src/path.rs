// path.rs
// 路径处理模块，提供路径拼接和标准化功能

use alloc::string::String;
use alloc::vec::Vec;

use super::*;

/// 拼接多个路径片段为一个完整的路径
///
/// # 参数
/// - `base`: 基础路径（可以是绝对路径或相对路径）
/// - `segments`: 需要拼接的路径片段列表
///
/// # 返回值
/// 拼接后的路径字符串，未进行标准化处理
///
/// # 示例
/// ```
/// let path = join("/home", &["user", "docs"]);
/// assert_eq!(path, "/home/user/docs");
/// let path = join("user", &["docs"]);
/// assert_eq!(path, "user/docs");
/// ```
pub fn join(base: &str, segments: &[&str]) -> String {
    let mut result = String::from(base);
    // 去除基础路径末尾的'/'，避免重复分隔符
    result = result.trim_end_matches('/').to_string();

    // 依次拼接每个路径片段
    for &seg in segments.iter() {
        if seg.is_empty() {
            continue;
        }
        // 如果当前结果为空或不是绝对路径，且片段以'/'开头，则直接拼接
        if result.is_empty() || !result.starts_with('/') {
            result.push_str(seg);
        } else {
            // 否则添加分隔符'/'，并去除片段开头的'/'（如果有）
            result.push('/');
            result.push_str(seg.trim_start_matches('/'));
        }
    }

    // 如果基础路径是绝对路径，确保结果也是绝对路径
    if base.starts_with('/') && !result.starts_with('/') {
        result = format!("/{}", result);
    }
    result
}

/// 标准化路径，处理`.`、`..`、多余的'/'等，返回规范化的路径
///
/// # 参数
/// - `path`: 需要标准化的路径字符串
/// - `current_dir`: 当前工作目录（用于相对路径转换为绝对路径），如果为None则不转换
///
/// # 返回值
/// 标准化后的路径字符串
///
/// # 示例
/// ```
/// let path = canonicalize("/home/../usr/./local", None);
/// assert_eq!(path, "/usr/local");
/// let path = canonicalize("docs/../files", Some("/home/user"));
/// assert_eq!(path, "/home/user/files");
/// ```
pub fn canonicalize(path: &str, current_dir: Option<&str>) -> String {
    // 如果是相对路径，且提供了当前工作目录，则转换为绝对路径
    let mut full_path = if path.starts_with('/') {
        String::from(path)
    } else if let Some(cwd) = current_dir {
        join(cwd, &[path])
    } else {
        String::from(path)
    };

    // 处理空路径
    if full_path.is_empty() {
        return String::from("/");
    }

    // 分割路径为组件，忽略多余的'/'
    let components: Vec<&str> = full_path.split('/').filter(|s| !s.is_empty()).collect();

    let mut result = Vec::new();
    let is_absolute = full_path.starts_with('/');

    // 处理路径组件，解析`.`和`..`
    for comp in components {
        match comp {
            "." => continue, // 当前目录，无需处理
            ".." => {
                // 上级目录，弹出最后一个组件（如果有）
                if !result.is_empty() {
                    result.pop();
                } else if !is_absolute {
                    // 如果是相对路径，且无法再向上，保留".."
                    result.push(comp);
                }
                // 绝对路径且无法向上时，直接忽略".."
            }
            _ => result.push(comp), // 普通目录或文件名，保留
        }
    }

    // 构建最终路径
    if is_absolute {
        if result.is_empty() {
            String::from("/")
        } else {
            format!("/{}", result.join("/"))
        }
    } else {
        if result.is_empty() {
            String::from(".")
        } else {
            result.join("/")
        }
    }
}

/// 获取路径的父目录
///
/// # 参数
/// - `path`: 输入路径
///
/// # 返回值
/// 父目录路径，如果是根目录（"/"）或空路径，则返回None
///
/// # 示例
/// ```
/// let parent = parent_dir("/home/user/docs");
/// assert_eq!(parent, Some("/home/user".to_string()));
/// let parent = parent_dir("/");
/// assert_eq!(parent, None);
/// ```
pub fn parent_dir(path: &str) -> Option<String> {
    let normalized = canonicalize(path, None);
    if normalized == "/" || normalized.is_empty() {
        return None;
    }
    let last_slash = normalized.rfind('/').unwrap_or(0);
    if last_slash == 0 {
        Some(String::from("/"))
    } else {
        Some(normalized[..last_slash].to_string())
    }
}

/// 获取路径的文件名或最后一个组件
///
/// # 参数
/// - `path`: 输入路径
///
/// # 返回值
/// 文件名或最后一个路径组件，如果路径为空或为根目录，则返回None
///
/// # 示例
/// ```
/// let name = base_name("/home/user/docs.txt");
/// assert_eq!(name, Some("docs.txt".to_string()));
/// let name = base_name("/");
/// assert_eq!(name, None);
/// ```
pub fn base_name(path: &str) -> Option<String> {
    let normalized = path.trim_end_matches('/').to_string();
    if normalized.is_empty() || normalized == "/" {
        return None;
    }
    let last_slash = normalized.rfind('/').unwrap_or(0);
    Some(normalized[last_slash + 1..].to_string())
}

#[cfg(test)]
mod tests {
    use crate::path::*;

    #[test]
    fn test_join() {
        assert_eq!(join("/home", &["user", "docs"]), "/home/user/docs");
        assert_eq!(join("/home/", &["user", "docs"]), "/home/user/docs");
        assert_eq!(join("home", &["user", "docs"]), "home/user/docs");
        assert_eq!(join("/", &["home", "user"]), "/home/user");
        assert_eq!(join("", &["home", "user"]), "home/user");
    }

    #[test]
    fn test_canonicalize() {
        // 绝对路径
        assert_eq!(canonicalize("/home/../usr/./local", None), "/usr/local");
        assert_eq!(canonicalize("/home/user//docs///", None), "/home/user/docs");
        assert_eq!(canonicalize("/home/../../usr", None), "/usr");
        assert_eq!(canonicalize("/home/../../", None), "/");
        assert_eq!(canonicalize("/", None), "/");

        // 相对路径（无当前目录）
        assert_eq!(canonicalize("home/../usr/./local", None), "usr/local");
        assert_eq!(canonicalize("home/../../usr", None), "usr");
        assert_eq!(canonicalize("home/../..", None), ".");

        // 相对路径（有当前目录）
        assert_eq!(
            canonicalize("docs/../files", Some("/home/user")),
            "/home/user/files"
        );
        assert_eq!(canonicalize("../files", Some("/home/user")), "/home/files");
        assert_eq!(canonicalize("../../files", Some("/home/user")), "/files");
    }

    #[test]
    fn test_parent_dir() {
        assert_eq!(
            parent_dir("/home/user/docs"),
            Some("/home/user".to_string())
        );
        assert_eq!(parent_dir("/home"), Some("/".to_string()));
        assert_eq!(parent_dir("/"), None);
        assert_eq!(parent_dir("home/user"), Some("home".to_string()));
        assert_eq!(parent_dir("home"), Some(".".to_string()));
    }

    #[test]
    fn test_base_name() {
        assert_eq!(
            base_name("/home/user/docs.txt"),
            Some("docs.txt".to_string())
        );
        assert_eq!(base_name("/home/user/"), Some("user".to_string()));
        assert_eq!(base_name("/"), None);
        assert_eq!(base_name("home/user"), Some("user".to_string()));
        assert_eq!(base_name("home"), Some("home".to_string()));
    }
}
