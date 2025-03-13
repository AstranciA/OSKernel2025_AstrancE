use std::{io::Read, process::Command};

// TODO: test other architectures
use super::*;

const ELF_FILES: [&str; 2] = [
    "./testcases/target/release/00hello",
    "./testcases/target/release/01fib",
];

const LINKER_SCRIPT_CONTENTS: &str = r#"
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad 2
    .quad app_0_start
    .quad app_1_start
    .quad app_1_end

    .section .data
    .global app_app_0_start
    .global app_app_0_end
app_app_0_start:
    .incbin "./testcases/target/release/00hello"
app_app_0_end:

    .section .data
    .global app_app_1_start
    .global app_app_1_end
app_app_1_start:
    .incbin "./testcases/target/release/01fib"
app_app_1_end:
"#;

fn build_test() {
    Command::new("cargo")
        .current_dir("./testcases")
        .arg("build")
        .arg("--release")
        //.arg("--target=riscv64gc-unknown-none-elf")
        .status()
        .expect("Failed to execute cargo build");
}
#[test]
pub fn gen_elf() {
    build_test();
    let linker_script_path = "./testcases/link.ld";
    let mut paths = Vec::new();
    for elf in ELF_FILES {
        let path = Path::new(elf);
        paths.push(path);
    }

    let elf_loader = BatBuilder::from_elfs(paths.as_slice());
    elf_loader.generate_link_script(linker_script_path);

    let mut linker_script = File::open(linker_script_path).unwrap();

    let mut contents = String::new();
    linker_script.read_to_string(&mut contents).unwrap();

    assert_eq!(contents, LINKER_SCRIPT_CONTENTS)
}
