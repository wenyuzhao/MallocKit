use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn target_dir_file(filename: &str) -> PathBuf {
    Path::new(&env::var_os("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join(filename)
}

fn generate_one_test(test: &str, script: &str) -> String {
    format!(
        r#"
#[test]
fn {}() {{
    mallockit::util::testing::test(env!("CARGO_CRATE_NAME"), {:?});
}}
"#,
        test, script,
    )
}

fn generate_malloc_tests(tests: &HashMap<String, String>) {
    let mut code = "".to_owned();
    for (t, cmd) in tests {
        code += &generate_one_test(t, cmd);
    }
    fs::write(target_dir_file("generated_tests.rs"), code).unwrap();
}

fn main() {
    let meta = cargo_metadata::MetadataCommand::new().exec().unwrap();
    let mut malloc_tests = HashMap::new();
    if let Some(v) = meta
        .workspace_metadata
        .as_object()
        .and_then(|v| v.get("malloc-tests"))
    {
        for (name, cmd) in v.as_object().unwrap().iter() {
            let cmd = cmd.as_str().unwrap();
            malloc_tests.insert(name.to_owned(), cmd.to_owned());
        }
    }
    generate_malloc_tests(&malloc_tests);
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_path = meta.workspace_root.as_std_path().join("Cargo.toml");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_path.to_str().unwrap().to_owned()
    );
}
