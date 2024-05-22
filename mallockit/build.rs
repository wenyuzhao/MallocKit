use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;

fn target_dir_file(filename: &str) -> PathBuf {
    Path::new(&env::var_os("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join(filename)
}

struct MallocTestsGenerator {
    tests: HashMap<String, String>,
}

impl MallocTestsGenerator {
    fn new(meta: &Metadata) -> Self {
        let mut tests = HashMap::new();
        let ws_meta = meta.workspace_metadata.as_object();
        if let Some(v) = ws_meta.and_then(|v| v.get("malloc-tests")) {
            for (name, cmd) in v.as_object().unwrap() {
                let cmd = cmd.as_str().unwrap();
                tests.insert(name.to_owned(), cmd.to_owned());
            }
        }
        Self { tests }
    }

    fn generate_one(&self, test: &str, script: &str) -> String {
        format!(
            r#"
                #[test]
                fn {}() {{
                    ::mallockit::util::testing::malloc::test(env!("CARGO_CRATE_NAME"), {:?});
                }}
            "#,
            test, script,
        )
    }

    fn generate(&self) {
        let mut code = "".to_owned();
        for (t, cmd) in &self.tests {
            code += &self.generate_one(t, cmd);
        }
        fs::write(target_dir_file("generated_tests.rs"), code).unwrap();
    }
}

fn main() {
    let meta = cargo_metadata::MetadataCommand::new().exec().unwrap();

    // Malloc tests
    let malloc_tests_gen = MallocTestsGenerator::new(&meta);
    malloc_tests_gen.generate();
    // Rust allocator tests

    println!("cargo:rerun-if-changed=build.rs");
    let manifest_path = meta.workspace_root.as_std_path().join("Cargo.toml");
    println!(
        "cargo:rerun-if-changed={}",
        manifest_path.to_str().unwrap().to_owned()
    );
}
