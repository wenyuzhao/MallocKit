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

    /// The program to run, with arguments
    #[arg(last = true, allow_hyphen_values = true)]
    command: Vec<String>,
}

fn build_crate(opts: &Options) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    let mut cmd = cmd.arg("build").args(["-p", &opts.malloc]);
    if opts.features.len() > 0 {
        cmd = cmd.arg("--features").arg(opts.features.join(","));
    }
    if opts.all_features {
        cmd = cmd.arg("--all-features");
    }
    if opts.no_default_features {
        cmd = cmd.arg("--no-default-features");
    }
    if opts.release {
        cmd = cmd.arg("--release");
    }
    if let Some(profile) = &opts.profile {
        cmd = cmd.arg("--profile").arg(profile);
    }
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("cargo run failed: {}", status);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options = Options::parse();
    // 1. Build the crate
    build_crate(&options)?;
    // 2. Run the program
    let args = options.command.clone();
    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    let profile = if options.release { "release" } else { "debug" };
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let ld_preload_env = if cfg!(target_os = "macos") {
        "DYLD_INSERT_LIBRARIES"
    } else {
        "LD_PRELOAD"
    };
    cmd.env(
        ld_preload_env,
        format!("target/{}/lib{}.{}", profile, options.malloc, ext),
    );
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("program failed: {}", status);
    }
    Ok(())
}
