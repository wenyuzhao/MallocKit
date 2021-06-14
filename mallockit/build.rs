use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust::{Yaml, YamlLoader};

fn out_dir_file(filename: &str) -> PathBuf {
    Path::new(&env::var_os("OUT_DIR").unwrap()).join(filename)
}

fn generate_one_test(malloc: &str, test: &str, command: &str, is_slow: bool) -> String {
    let (mut cmd, args) = command.split_once(" ").unwrap();
    cmd = cmd.trim();
    let args = args.trim().escape_debug().to_string();
    let args = shell_words::split(&args).unwrap();
    let slow_test_guard = if is_slow {
        "\n#[cfg(feature = \"slow_tests\")]"
    } else {
        ""
    };
    format!(
        r#"
#[test]{}
fn {}_{}() {{
    runner::test("{}", "{}", &{:?});
}}
"#,
        slow_test_guard, test, malloc, malloc, cmd, args,
    )
}

fn generate_tests(meta: &Yaml) {
    let mallocs = meta["algorithms"]
        .as_vec()
        .map(|v| {
            v.iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let slow_mallocs = meta["slow-algorithms"]
        .as_vec()
        .map(|v| {
            v.iter()
                .map(|x| x.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
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
        for m in &mallocs {
            code += &generate_one_test(m, t, cmd, false);
        }
        for m in &slow_mallocs {
            code += &generate_one_test(m, t, cmd, true);
        }
    }
    fs::write(&out_dir_file("generated_tests.rs"), code).unwrap();
}

fn main() {
    let metadata_file = PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("mallockit.yaml")
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let meta = &YamlLoader::load_from_str(&fs::read_to_string(&metadata_file).unwrap()).unwrap()[0];
    generate_tests(meta);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", metadata_file);
}
