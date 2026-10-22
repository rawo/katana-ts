use std::time::Duration;

use anyhow::bail;
use katana_proto::{read_patch, write_patch, MidirTransport};

/// Dump, write back, dump again, and compare.
pub fn run(port: &str) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let a = read_patch(&mut t, Duration::from_secs(2))?;
    write_patch(&mut t, &a)?;
    let b = read_patch(&mut t, Duration::from_secs(2))?;
    match a.mem.diff(&b.mem).first() {
        None => {
            println!("identical");
            Ok(())
        }
        Some((addr, old, new)) => bail!("first difference at {addr}: {old:02X?} -> {new:02X?}"),
    }
}
