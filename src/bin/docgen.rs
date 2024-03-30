use std::error::Error;
use openpid::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let spec: OpenPID = toml::from_str(&std::fs::read_to_string("./openpid.toml")?)?;
    openpid::docgen::document(&spec, std::path::PathBuf::from("./outputs"))?;
    Ok(())
}
