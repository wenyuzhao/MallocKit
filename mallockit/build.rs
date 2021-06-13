use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

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
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let slow_mallocs = meta["slow-algorithms"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let tests = meta["tests"]
        .as_hash()
        .unwrap()
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().unwrap().to_owned(),
                v.as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let mut code = "".to_owned();
    for m in &mallocs {
        for (t, cmd) in &tests {
            code += &generate_one_test(m, t, cmd, false);
        }
    }
    for m in &slow_mallocs {
        for (t, cmd) in &tests {
            code += &generate_one_test(m, t, cmd, true);
        }
    }
    let outfile = Path::new(&env::var_os("OUT_DIR").unwrap()).join("generated_tests.rs");
    fs::write(&outfile, code).unwrap();
}

fn main() {
    let metadata_file = {
        let mut p = PathBuf::from(env::var("OUT_DIR").unwrap());
        p.push("..");
        p.push("..");
        p.push("..");
        p.push("..");
        p.push("..");
        p.push("mallockit.yaml");
        let p = p.canonicalize().unwrap();
        p.to_str().unwrap().to_owned()
    };
    let meta = &YamlLoader::load_from_str(&fs::read_to_string(&metadata_file).unwrap()).unwrap()[0];
    generate_tests(meta);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", metadata_file);
}
