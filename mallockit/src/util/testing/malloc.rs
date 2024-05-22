use spin::Mutex;
use std::alloc::Allocator;
use std::fs;
use std::{path::PathBuf, process::Command};

fn env_segments() -> (&'static str, &'static str, &'static str) {
    if cfg!(target_os = "linux") {
        ("lib", "LD_PRELOAD", "so")
    } else if cfg!(target_os = "macos") {
        ("lib", "DYLD_INSERT_LIBRARIES", "dylib")
    } else {
        unimplemented!()
    }
}

fn run_cmd(env: &str, dylib: &str, cmd: &str) {
    let cmd = shell_words::split(cmd).unwrap();
    let (prog, args) = cmd.split_first().unwrap();
    std::println!("> env {}={} {} {}", env, dylib, prog, args.join(" "));
    let output = Command::new(prog)
        .current_dir("..")
        .args(args)
        .env(env, dylib)
        .output()
        .unwrap();
    std::println!("{}", String::from_utf8(output.stdout).unwrap());
    std::eprintln!("{}", String::from_utf8(output.stderr).unwrap());
    assert!(
        output.status.success(),
        "FAILED > env {}={} {} {}",
        env,
        dylib,
        prog,
        args.join(" ")
    );
}

pub fn test(malloc: &str, script: &str) {
    build_cdylib();
    let mut dylib = PathBuf::from("..")
        .join("target")
        .join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        });
    let (prefix, env, suffix) = env_segments();
    dylib.push(format!("{}{}.{}", prefix, malloc, suffix));
    let dylib = fs::canonicalize(&dylib).unwrap();
    let commands = script
        .trim()
        .split('\n')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty());
    for cmd in commands {
        run_cmd(env, dylib.to_str().unwrap(), cmd);
    }
}

fn build_cdylib() {
    static CDYLIB_BUILT: Mutex<bool> = Mutex::new(false);
    let mut status = CDYLIB_BUILT.lock();
    if *status {
        return;
    }
    *status = true;
    let args = if cfg!(debug_assertions) {
        vec!["build", "--features", "malloc"]
    } else {
        vec!["build", "--features", "malloc", "--release"]
    };
    let output = Command::new(env!("CARGO")).args(args).output().unwrap();
    if !output.status.success() {
        std::println!("{}", String::from_utf8(output.stdout).unwrap());
        std::eprintln!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Faild to build malloc library.")
    }
}

pub fn rs_test(alloc: impl Allocator) {
    let mut v = Vec::new_in(alloc);
    v.push(1);
    assert_eq!(v.pop(), Some(1));
    assert_eq!(v.pop(), None);
}
