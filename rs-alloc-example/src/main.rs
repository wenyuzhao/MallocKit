use std::{fs::File, io::Read};

use hoard::Global;

#[global_allocator]
static GLOBAL: Global = Global;

fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    let mut file = File::open("../Cargo.toml")?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let toml_file = toml::from_str::<toml::Value>(&content)?;
    println!("{:?}", toml_file);
    Ok(())
}
