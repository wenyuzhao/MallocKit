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

pub fn get_stdin(bench: &str) -> Stdio {
    match bench {
        "barnes" => {
            Stdio::from(File::open(format!("./mimalloc-bench/bench/barnes/input")).unwrap())
        }
        _ => Stdio::null(),
    }
}

pub fn get_command(bench: &str) -> Stdio {
    match bench {
        "barnes" => {
            Stdio::from(File::open(format!("./mimalloc-bench/bench/barnes/input")).unwrap())
        }
        _ => Stdio::null(),
    }
}

pub struct Bench {
    name: String,
    cmd: Command,
}

impl Bench {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cmd: Command::new(format!("./mimalloc-bench/out/bench/{name}")),
        }
        .init()
    }

    fn init(mut self) -> Self {
        self.cmd.stdin(Stdio::null());
        self.init_args_and_stdin();
        self
    }

    fn init_args_and_stdin(&mut self) {
        match self.name.as_str() {
            "barnes" => {
                self.cmd.stdin(Stdio::from(
                    File::open("./mimalloc-bench/bench/barnes/input").unwrap(),
                ));
            }
            "cfrac" => {
                self.cmd.arg("17545186520507317056371138836327483792789528");
            }
            "espresso" => {
                self.cmd
                    .arg("./mimalloc-bench/bench/espresso/largest.espresso");
            }
            "gs" => {
                self.cmd.args([
                    "-dBATCH",
                    "-dNODISPLAY",
                    "./mimalloc-bench/extern/large.pdf",
                ]);
            }
            "z3" => {
                self.cmd
                    .args(["-smt2", "./mimalloc-bench/bench/z3/test1.smt2"]);
            }
            _ => panic!("Unknown benchmark: {}", self.name),
        }
    }

    pub fn alloc(mut self, alloc: &str) -> Self {
        if alloc != "sys" {
            self.cmd.env(LD_PRELOAD, format!("lib{alloc}.{DYLIB_EXT}"));
        } else {
            self.cmd.env("SYSMALLOC", "1");
        }
        self
    }

    pub fn run(&mut self) {
        let output = self.cmd.status().unwrap();
        assert!(output.success());
    }
}
