use std::{path::PathBuf, process::Command};

const MALLOC_IMPLEMENTATIONS: &'static [&'static str] = &["bump", "buddy"];

#[macro_export]
macro_rules! tests_dir {
    ($filename: expr) => {{
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push($filename);
        p.to_str().unwrap().to_owned()
    }};
}

#[macro_export]
macro_rules! for_each_test {
    ($filename: expr) => {{
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests");
        p.push($filename);
        p.to_str().unwrap().to_owned()
    }};
}

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
    let child = Command::new("clang")
        .current_dir("..")
        .args(args)
        .env("DYLD_INSERT_LIBRARIES", dylib)
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
}

pub fn test(cmd: &str, args: &[&str]) {
    for malloc in MALLOC_IMPLEMENTATIONS {
        exec_with_malloc_wrapper(malloc, cmd, args);
    }
}

// pub fn exec_with_malloc_wrapper<
//     Cmd: AsRef<OsStr>,
//     Arg: AsRef<OsStr>,
//     Args: IntoIterator<Item = Arg>,
// >(
//     cmd: Cmd,
//     args: Args,
// ) {
//     let mut dylib = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//     dylib.push("..");
//     dylib.push("target");
//     if cfg!(debug_assertions) {
//         dylib.push("debug");
//     } else {
//         dylib.push("release");
//     }
//     dylib.push("libbump.dylib");
//     let child = Command::new("clang")
//         .args(args)
//         .env("DYLD_INSERT_LIBRARIES", dylib)
//         .spawn()
//         .unwrap();
//     let output = child.wait_with_output().unwrap();
//     assert!(output.status.success());
// }
