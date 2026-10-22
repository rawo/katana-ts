use std::path::Path;

use anyhow::Context;
use katana_proto::{write_patch, MidirTransport, Patch};

pub fn run(port: &str, file: &Path) -> anyhow::Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    // Validate the whole file before anything is sent to the amp.
    let p = Patch::from_syx(&bytes).with_context(|| format!("{} is not a complete KATANA Gen 3 patch", file.display()))?;
    let mut t = MidirTransport::open(port)?;
    write_patch(&mut t, &p)?;
    println!("loaded {} into the current patch", file.display());
    Ok(())
}
