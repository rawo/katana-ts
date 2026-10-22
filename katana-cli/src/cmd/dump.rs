use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use katana_proto::{read_patch, MidirTransport};

pub fn run(port: &str, out: &Path) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let p = read_patch(&mut t, Duration::from_secs(2))?;
    std::fs::write(out, p.to_syx()).with_context(|| format!("writing {}", out.display()))?;
    println!("wrote {} ({} bytes of patch data)", out.display(), p.mem.0.len());
    Ok(())
}
