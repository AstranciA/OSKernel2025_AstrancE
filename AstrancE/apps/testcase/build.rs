use std::{env, process::Command};

use acbat::BatBuilder;

const TESTCASES: [&str; 2] = [
    //"../../testcases/nimbos/build/x86_64/test_hello_world"
    "../../testcases/simple/build/x86_64/hello",
    "../../testcases/simple/build/x86_64/fib"
];
const LINKER_FILE: &str = "./link_apps.S";

fn main() {
    println!("cargo:rerun-if-changed=../../testcases/simple");
    //let status = Command::new("make").current_dir("../../testcases/simple").args(&[
        //"build"
    //]);
    let mut bat_builer = BatBuilder::default();
    for testcase in TESTCASES {
        println!("cargo:rerun-if-changed={}", testcase);
        bat_builer.add_elf(testcase);
    }



    bat_builer.generate_link_script(LINKER_FILE);
}
