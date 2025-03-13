use std::{fs::File, io::Write, path::Path};
#[cfg(test)]
mod test;

pub struct BatBuilder {
    apps: Vec<AppInfo>,
}

impl BatBuilder {
    pub fn from_elfs(elf_files: &[impl AsRef<Path>]) -> Self {
        let apps = elf_files
            .iter()
            .enumerate()
            .map(|(app_id, path)| {
                let path = path.as_ref();
                let path_name = path
                    .to_str()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid ELF file path",
                        )
                    })
                    .unwrap();
                let name_no_ext = path
                    .file_stem()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid ELF file path",
                        )
                    })
                    .unwrap()
                    .to_str()
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Non-UTF8 filename")
                    })
                    .unwrap();

                AppInfo {
                    app_idx: app_id,
                    app_name: name_no_ext.into(),
                    app_path: path_name.into(),
                }
            })
            .collect();
        Self { apps }
    }

    /** Generate a linker script for the given ELF files.
     *  Linker format reference: [https://github.com/rcore-os/rCore-Tutorial-v3]
     */
    // TODO: support other architectures
    //#[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
    pub fn generate_link_script(&self, output_path: impl AsRef<Path>) {
        let output_path = output_path.as_ref();

        let mut f = File::create(output_path).unwrap();

        let apps = &self.apps;

        // application metadata
        writeln!(
            f,
            r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {}"#,
            apps.len()
        )
        .unwrap();

        // 生成应用入口指针数组
        for app in apps {
            writeln!(f, r#"    .quad app_{}_start"#, app.app_idx).unwrap();
        }
        writeln!(f, r#"    .quad app_{}_end"#, self.apps.len() - 1).unwrap();

        // 为每个文件生成引用代码
        for app in apps {
            let app_id = app.app_id();
            let app_name = &app.app_name;
            let app_path = &app.app_path;
            println!("Linking app_{}: {} ({:?})", app_id, app_name, app_path);
            writeln!(
                f,
                r#"
    .section .data
    .global app_{0}_start
    .global app_{0}_end
app_{0}_start:
    .incbin "{1}"
app_{0}_end:"#,
                app_id, app_path
            )
            .unwrap();
        }
    }
}

struct AppInfo {
    app_idx: usize,
    app_name: String,
    app_path: String,
}

impl AppInfo {
    fn app_id(&self) -> String {
        format!("app_{}", self.app_idx)
    }
}
