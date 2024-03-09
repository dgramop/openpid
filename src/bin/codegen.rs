use std::path::PathBuf;

use openpid::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec: OpenPID = toml::from_str(&std::fs::read_to_string("./openpid.toml")?)?;
    println!("{:#?}", spec);

    println!("{:?}",spec.codegen_linux_c(PathBuf::from("/tmp/codegen")));

    Ok(())
}
