use std::{
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

include!(concat!(env!("OUT_DIR"), "/malloc_implementations.rs"));

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
macro_rules! test_malloc {
    ($runner: ident, $malloc: ident) => {
        concat_idents::concat_idents!(test_name = $runner, _, $malloc {
            #[test]
            fn test_name() {
                $runner(stringify!($malloc));
            }
        });
    };
}

#[macro_export]
macro_rules! test_all_malloc {
    ($runner: ident) => {
        macro_rules! __test_all_malloc {
            ($malloc: ident) => {
                test_malloc!($runner, $malloc);
            };
        }
        malloc_implementations!(__test_all_malloc);
        #[cfg(feature = "slow_tests")]
        slow_malloc_implementations!(__test_all_malloc);
    };
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
