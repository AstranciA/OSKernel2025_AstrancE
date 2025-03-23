use std::path::{Path, PathBuf};
use std::env::current_dir;
use pathdiff::diff_paths;
use path_clean::clean;

pub fn convert_base(
    original_path: &Path,
    original_base: &Path,
    new_base: &Path,
) -> Option<PathBuf> {
    // Resolve relative base directories to absolute paths using the current working directory
    let resolve_to_absolute = |path: &Path| -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir().unwrap().join(path)
        }
    };

    let original_base_abs = clean(resolve_to_absolute(original_base));
    let new_base_abs = clean(resolve_to_absolute(new_base));

    // Create absolute target path and clean it
    let target = clean(original_base_abs.join(original_path));
    
    // Compute relative path from new_base_abs to target
    diff_paths(target, new_base_abs)
}


/*
 *#[cfg(test)]
 *mod tests {
 *    use super::*;
 *    use std::path::Path;
 *    use tempfile::TempDir;
 *
 *    fn create_test_env() -> TempDir {
 *        let temp_dir = TempDir::new().unwrap();
 *        std::env::set_current_dir(&temp_dir).unwrap();
 *        temp_dir
 *    }
 *
 *    #[test]
 *    fn relative_base_directories() {
 *        let _temp = create_test_env();
 *        let original = Path::new("./foo");
 *        
 *        // Create test directories in temp location
 *        std::fs::create_dir_all("a").unwrap();
 *        std::fs::create_dir_all("b").unwrap();
 *
 *        let result = convert_base(
 *            original,
 *            Path::new("a"),
 *            Path::new("b"),
 *        ).unwrap();
 *
 *        assert_eq!(result, Path::new("../a/foo"));
 *    }
 *
 *    #[test]
 *    fn nested_relative_bases() {
 *        let _temp = create_test_env();
 *        let original = Path::new("../file");
 *        
 *        // Create directory structure
 *        std::fs::create_dir_all("a/b").unwrap();
 *        std::fs::create_dir_all("c/d").unwrap();
 *
 *        let result = convert_base(
 *            original,
 *            Path::new("a/b"),
 *            Path::new("c/d"),
 *        ).unwrap();
 *
 *        assert_eq!(result, Path::new("../../a/file"));
 *    }
 *
 *    #[test]
 *    fn mixed_absolute_relative_bases() {
 *        let temp_dir = create_test_env();
 *        let original = Path::new("file.txt");
 *        
 *        // Create relative directory
 *        std::fs::create_dir_all("dir").unwrap();
 *
 *        let result = convert_base(
 *            original,
 *            Path::new("dir"),
 *            temp_dir.path(), // Use temp dir's absolute path
 *        ).unwrap();
 *
 *        let expected = Path::new("dir/file.txt");
 *        assert_eq!(result, expected);
 *    }
 *
 *    #[test]
 *    fn same_base_directory() {
 *        let _temp = create_test_env();
 *        let original = Path::new("./file.txt");
 *        
 *        std::fs::create_dir_all("a").unwrap();
 *
 *        let result = convert_base(
 *            original,
 *            Path::new("a"),
 *            Path::new("a"),
 *        ).unwrap();
 *
 *        assert_eq!(result, Path::new("file.txt"));
 *    }
 *}
 */
