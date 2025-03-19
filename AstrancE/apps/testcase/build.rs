use std::{env, process::Command};

use acbat::BatBuilder;

const TESTCASES: [&str; 1] = [
    //"../../testcases/nimbos/build/x86_64/test_hello_world"
    //"../../testcases/simple/build/riscv64/hello",
    //"../../testcases/nimbos/build/riscv64/hello_world",
    "../../testcases/nimbos/build/riscv64/nothing",
];
const LINKER_FILE: &str = "./link_apps.S";

fn main() {
    println!("cargo:rerun-if-changed=../../testcases/simple");
    let status = Command::new("make").current_dir("../../testcases/simple").args(&[
        "ARCH=riscv64",
        "build"
    ]).status();
    assert!(status.unwrap().success());
    let mut bat_builer = BatBuilder::default();
    for testcase in TESTCASES {
        println!("cargo:rerun-if-changed={}", testcase);
        bat_builer.add_elf(testcase);
    }



    bat_builer.generate_link_script(LINKER_FILE);
}
