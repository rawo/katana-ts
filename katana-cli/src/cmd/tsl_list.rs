use std::path::Path;

use anyhow::Context;
use katana_proto::tsl::ToneSet;

/// Numbered patch names of a `.tsl` set, as `store --patch` counts them.
pub fn run(file: &Path) -> anyhow::Result<()> {
    let bytes = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    let set = ToneSet::parse(&bytes).with_context(|| format!("reading {}", file.display()))?;
    for (i, p) in set.patches.iter().enumerate() {
        match &p.patch {
            Ok(_) => println!("{:>3}  {}", i + 1, p.name),
            Err(e) => println!("{:>3}  {} (can't be loaded: {e})", i + 1, p.name),
        }
    }
    Ok(())
}
