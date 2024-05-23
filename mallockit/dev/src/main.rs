use std::process::Command;

use clap::Parser;

/// Run a program with a custom allocator
#[derive(Parser, Debug)]
struct Options {
    /// Malloc implementation to use
    malloc: String,

    /// Space or comma separated list of features to activate
    #[arg(short = 'F', long)]
    features: Vec<String>,

    /// Activate all available features
    #[arg(long, default_value_t = false)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long, default_value_t = false)]
    no_default_features: bool,

    /// Build artifacts in release mode, with optimizations
    #[arg(short, long, default_value_t = false)]
    release: bool,

    /// Build artifacts with the specified profile
    #[arg(short, long)]
    profile: Option<String>,

    /// Run the program under GDB
    #[arg(long, default_value_t = false)]
    gdb: bool,

    /// Run the program under LLDB
    #[arg(long, default_value_t = false)]
    lldb: bool,

    /// The program to run, with arguments
    #[arg(last = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

fn build_crate(opts: &Options) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").args(["-p", &opts.malloc]);
    let mut features = opts.features.clone();
    if !features.contains(&"malloc".to_owned()) && !opts.all_features {
        features.push("malloc".to_string());
    }
    if features.len() > 0 {
        cmd.arg("--features").arg(features.join(","));
    }
    if opts.all_features {
        cmd.arg("--all-features");
    }
    if opts.no_default_features {
        cmd.arg("--no-default-features");
    }
    if opts.release {
        cmd.arg("--release");
    }
    if let Some(profile) = &opts.profile {
        cmd.arg("--profile").arg(profile);
    }
    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

const ENV: &str = if cfg!(target_os = "macos") {
    "DYLD_INSERT_LIBRARIES"
} else {
    "LD_PRELOAD"
};

const EXT: &str = if cfg!(target_os = "macos") {
    "dylib"
} else {
    "so"
};

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    // 1. Build the crate
    build_crate(&options)?;
    // 2. Run the program
    let args = options.command.clone();
    let profile = if options.release { "release" } else { "debug" };
    let dylib = format!("target/{}/lib{}.{}", profile, options.malloc, EXT);
    let mut cmd = if !options.gdb && !options.lldb {
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        cmd.env(ENV, dylib);
        println!("{:?}", cmd);
        cmd
    } else if options.gdb {
        let mut cmd = Command::new("rust-gdb");
        cmd.args(&["-ex", &format!("set environment {ENV}={dylib}")]);
        cmd.arg("--args").args(&args);
        cmd
    } else {
        let mut cmd = Command::new("rust-lldb");
        cmd.args(&["-o", &format!("env {ENV}={dylib}")]);
        cmd.arg("--source-quietly");
        cmd.arg("--").args(&args);
        cmd
    };
    cmd.env("RUST_BACKTRACE", "1");
    let status = cmd.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}
