use std::{
    fs::File,
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use harness::Bencher;

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
    alloc_name: String,
    alloc_path: String,
    cmd: Command,
    out: PathBuf,
    server: Option<Child>,
}

const LOCAL_DEV_DIR: &str = "./mimalloc-bench/extern";

impl Bench {
    pub fn new(name: &str) -> Self {
        let malloc = std::env::var("MALLOC").unwrap();
        let is_external = std::env::var("IS_MALLOCKIT").unwrap() == "0";
        if is_external {
            println!("[{name}] malloc: {malloc} (external)");
        } else {
            println!("[{name}] malloc: {malloc} (mallockit)");
        }
        Self {
            name: name.to_string(),
            alloc_name: malloc.clone(),
            alloc_path: Self::get_malloc_lib_path(&malloc, is_external),
            cmd: Command::new(Self::get_binary_path(name)),
            out: harness::utils::HARNESS_BENCH_SCRATCH_DIR.join("log"),
            server: None,
        }
        .init()
    }

    fn init(mut self) -> Self {
        self.cmd.stdin(Stdio::null());
        self.init_args_and_stdin();
        self.cmd
            .stdout(Stdio::from(File::create(&self.out).unwrap()));
        if self.name != "redis" {
            Self::use_malloc(&mut self.cmd, &self.alloc_name, &self.alloc_path);
        }
        self.prepare();
        self
    }

    fn get_binary_path(name: &str) -> String {
        match name {
            "lean" => "../bin/lean".to_owned(),
            "lua" => "make".to_owned(),
            "redis" => format!("{LOCAL_DEV_DIR}/redis-6.2.7/src/redis-benchmark"),
            "rocksdb" => format!("{LOCAL_DEV_DIR}/rocksdb-8.1.1/db_bench"),
            _ => format!("./mimalloc-bench/out/bench/{name}"),
        }
    }

    fn get_malloc_lib_path(name: &str, is_external: bool) -> String {
        if !is_external {
            return format!("lib{name}.{DYLIB_EXT}");
        }
        match name {
            "hd" => format!("{LOCAL_DEV_DIR}/hd/src/libhoard.{DYLIB_EXT}"),
            "je" => format!("{LOCAL_DEV_DIR}/je/lib/libjemalloc.{DYLIB_EXT}"),
            "tc" => format!("{LOCAL_DEV_DIR}/tc/.libs/libtcmalloc_minimal.{DYLIB_EXT}"),
            "mi" => format!("{LOCAL_DEV_DIR}/mi/out/release/libmimalloc.{DYLIB_EXT}"),
            "sys" => "1".to_owned(),
            _ => panic!("Unknown malloc: {name}"),
        }
    }

    fn init_args_and_stdin(&mut self) {
        let procs = num_cpus::get().to_string();
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
            "larson" => {
                self.cmd
                    .args(["5", "8", "1000", "5000", "100", "4141", &procs]);
            }
            "lean" => {
                let procs = usize::min(num_cpus::get(), 8).to_string();
                self.cmd
                    .args(["--make", "-j", &procs])
                    .current_dir(format!("{LOCAL_DEV_DIR}/lean/library"));
            }
            "lua" => {
                self.cmd.current_dir(format!("{LOCAL_DEV_DIR}/lua"));
            }
            "redis" => {
                self.cmd.args([
                    "-r", "1000000", "-n", "100000", "-q", "-P", "16", "lpush", "a", "1", "2", "3",
                    "4", "5", "lrange", "a", "1", "5",
                ]);
            }
            "rocksdb" => {
                self.cmd.args(["--benchmarks=fillrandom"]);
            }
            "z3" => {
                self.cmd
                    .args(["-smt2", "./mimalloc-bench/bench/z3/test1.smt2"]);
            }
            _ => panic!("Unknown benchmark: {}", self.name),
        }
    }

    fn prepare(&mut self) {
        match self.name.as_str() {
            "lean" => {
                let status = Command::new("make")
                    .arg("clean-olean")
                    .current_dir(format!("{LOCAL_DEV_DIR}/lean/out/release"))
                    .status()
                    .unwrap();
                assert!(status.success());
            }
            "lua" => {
                let status = Command::new("make")
                    .arg("clean")
                    .current_dir(format!("{LOCAL_DEV_DIR}/lua"))
                    .status()
                    .unwrap();
                assert!(status.success());
            }
            "redis" => {
                // start the background server
                let mut cmd = Command::new(format!("{LOCAL_DEV_DIR}/redis-6.2.7/src/redis-server"));
                Self::use_malloc(&mut cmd, &self.alloc_name, &self.alloc_path);
                self.server = Some(cmd.spawn().unwrap());
            }
            _ => {}
        }
    }

    fn use_malloc(cmd: &mut Command, alloc: &str, path: &str) {
        if alloc != "sys" {
            cmd.env(LD_PRELOAD, path);
        } else {
            cmd.env("SYSMALLOC", "1");
        }
    }

    pub fn run(&mut self) {
        if self.name == "redis" {
            let redis_dir = format!("{LOCAL_DEV_DIR}/redis-6.2.7/src");
            std::thread::sleep(std::time::Duration::from_secs(1));
            let status = Command::new(format!("{redis_dir}/redis-cli"))
                .arg("flushall")
                .status()
                .unwrap();
            assert!(status.success());
            let status = self.cmd.status().unwrap();
            assert!(status.success());
            std::thread::sleep(std::time::Duration::from_secs(1));
            let status = Command::new(format!("{redis_dir}/redis-cli"))
                .arg("flushall")
                .status()
                .unwrap();
            assert!(status.success());
            std::thread::sleep(std::time::Duration::from_secs(1));
            let status = Command::new(format!("{redis_dir}/redis-cli"))
                .arg("shutdown")
                .status()
                .unwrap();
            assert!(status.success());
            self.server.as_mut().unwrap().wait().unwrap();
        } else {
            let status = self.cmd.status().unwrap();
            assert!(status.success());
        }
    }

    pub fn finalize(&mut self, bencher: &Bencher) {
        let log = std::fs::read_to_string(&self.out).unwrap();
        println!("{}", log);
        if self.name == "larson" {
            let time = log
                .lines()
                .find(|line| line.contains("operations per second, relative time:"))
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim()
                .trim_end_matches("s.")
                .parse::<f32>()
                .unwrap();
            bencher.add_stat("time", time * 1000.0);
        }
        if self.name == "redis" {
            let s = log
                .lines()
                .find(|line| line.contains("requests per second"))
                .unwrap();
            let s = s.split_once(" requests per second").unwrap().0;
            let s = s.split_whitespace().last().unwrap().trim();
            let ops = s.parse::<f32>().unwrap();
            let time = 2000000f32 / ops;
            bencher.add_stat("time", time * 1000.0);
        }
    }
}
