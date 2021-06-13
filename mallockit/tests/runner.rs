use std::{
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

fn exec_with_malloc_wrapper(malloc: &str, cmd: &str, args: &[&str]) {
    let mut dylib = PathBuf::from(".");
    dylib.push("target");
    if cfg!(debug_assertions) {
        dylib.push("debug");
    } else {
        dylib.push("release");
    }
    let prefix = "lib";
    let (env, suffix) = if cfg!(target_os = "linux") {
        ("LD_PRELOAD", "so")
    } else if cfg!(target_os = "macos") {
        ("DYLD_INSERT_LIBRARIES", "dylib")
    } else {
        unimplemented!()
    };
    dylib.push(format!("{}{}.{}", prefix, malloc, suffix));
    println!(
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
    println!("status: {}", output.status);
    if !output.status.success() {
        io::stdout().write_all(&output.stdout).unwrap();
        io::stderr().write_all(&output.stderr).unwrap();
    }
    assert!(output.status.success());
}

pub fn test(malloc: &str, cmd: &str, args: &[&str]) {
    exec_with_malloc_wrapper(malloc, cmd, args);
}
