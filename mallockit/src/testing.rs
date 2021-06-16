use spin::Mutex;
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

pub fn test(malloc: &str, cmd: &str, args: &[&str]) {
    build_cdylib();
    let mut dylib = PathBuf::from(".")
        .join("target")
        .join(if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        });
    let (prefix, env, suffix) = env_segments();
    dylib.push(format!("{}{}.{}", prefix, malloc, suffix));
    std::println!(
        "🔵 env {}={} {} {}",
        env,
        dylib.to_str().unwrap(),
        cmd,
        args.join(" ")
    );
    let output = Command::new(cmd)
        .current_dir("..")
        .args(args)
        .env(env, dylib)
        .output()
        .unwrap();
    std::println!("{}", String::from_utf8(output.stdout).unwrap());
    std::eprintln!("{}", String::from_utf8(output.stderr).unwrap());
    assert!(output.status.success());
}

fn build_cdylib() {
    static CDYLIB_BUILT: Mutex<bool> = Mutex::new(false);
    let mut status = CDYLIB_BUILT.lock();
    if *status {
        return;
    }
    *status = true;
    let args = if cfg!(debug_assertions) {
        vec!["+nightly", "build"]
    } else {
        vec!["+nightly", "build", "--release"]
    };
    let output = Command::new("cargo").args(args).output().unwrap();
    if !output.status.success() {
        std::println!("{}", String::from_utf8(output.stdout).unwrap());
        std::eprintln!("{}", String::from_utf8(output.stderr).unwrap());
        panic!("Faild to build malloc library.")
    }
}
