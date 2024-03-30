use std::error::Error;
use openpid::prelude::*;
use openpid_docgen::*;

fn main() -> Result<(), Box<dyn Error>> {
    let spec: OpenPID = toml::from_str(&std::fs::read_to_string("./openpid.toml")?)?;
    document(&spec, std::path::PathBuf::from("./outputs"))?;
    Ok(())
}
