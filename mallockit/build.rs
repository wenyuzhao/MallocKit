use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use yaml_rust::YamlLoader;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("malloc_implementations.rs");
    let mallocs_path = {
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
    let meta = &YamlLoader::load_from_str(&fs::read_to_string(&mallocs_path).unwrap()).unwrap()[0];
    let mallocs = meta["algorithms"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let slow_mallocs = meta["slow-algorithms"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        &dest_path,
        format!(
            "mallockit::name_list!(malloc_implementations: {});\nmallockit::name_list!(slow_malloc_implementations: {});",
            mallocs, slow_mallocs,
        ),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", mallocs_path);
}
