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
        .map(|x| x.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        &dest_path,
        format!(
            "mallockit::name_list!(malloc_implementations: {});",
            mallocs
        ),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", mallocs_path);
}
