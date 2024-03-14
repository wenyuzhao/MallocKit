// <test name> cfrac
// <allocator name> $alloc
// <environment args> ${ldpreload}=$alloc_lib
// <command> "./cfrac 17545186520507317056371138836327483792789528"
// <repeat> $i

// report: time, maxrss, usertime, systemtime, minor-page-faults

use std::{
    fs::File,
    process::{Command, Stdio},
};

const LD_PRELOAD: &str = if cfg!(target_os = "linux") {
    "LD_PRELOAD"
} else {
    "DYLD_INSERT_LIBRARIES"
};
const DYLIB_EXT: &str = if cfg!(target_os = "linux") {
    "so"
} else {
    "dylib"
};

pub fn run(bench: &str, alloc: &str) {
    let mut cmd = Command::new(format!("./mimalloc-bench/out/bench/{bench}"));
    let mut stdin = Stdio::null();
    match bench {
        "barnes" => {
            let f = File::open(format!("./mimalloc-bench/bench/barnes/input")).unwrap();
            stdin = Stdio::from(f);
        }
        "cfrac" => {
            cmd.arg("17545186520507317056371138836327483792789528");
        }

        _ => panic!("Unknown benchmark: {}", bench),
    };
    cmd.stdin(stdin);
    // Enable a specific malloc implementation
    if alloc != "sys" {
        cmd.env(LD_PRELOAD, format!("lib{alloc}.{DYLIB_EXT}"));
    }
    let output = cmd.status().unwrap();
    assert!(output.success());
}
