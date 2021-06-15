use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust::{Yaml, YamlLoader};

fn target_dir_file(filename: &str) -> PathBuf {
    Path::new(&env::var_os("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join(filename)
}

fn generate_one_test(test: &str, command: &str) -> String {
    let (mut cmd, args) = command.split_once(" ").unwrap();
    cmd = cmd.trim();
    let args = args.trim().escape_debug().to_string();
    let args = shell_words::split(&args).unwrap();
    format!(
        r#"
#[test]
fn {}() {{
    mallockit::testing::test(env!("CARGO_CRATE_NAME"), "{}", &{:?});
}}
"#,
        test, cmd, args,
    )
}

fn generate_tests(meta: &Yaml) {
    let tests = meta["tests"]
        .as_hash()
        .map(|v| {
            v.iter()
                .map(|(k, v)| {
                    (
                        k.as_str().unwrap().to_owned(),
                        v.as_str().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut code = "".to_owned();
    for (t, cmd) in &tests {
        code += &generate_one_test(t, cmd);
    }
    fs::write(&target_dir_file("generated_tests.rs"), code).unwrap();
}

fn main() {
    let metadata_file = PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("MallocKit.yml")
        .canonicalize()
        .unwrap();
    let meta = &YamlLoader::load_from_str(&fs::read_to_string(&metadata_file).unwrap()).unwrap()[0];
    generate_tests(meta);
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        metadata_file.to_str().unwrap().to_owned()
    );
}
