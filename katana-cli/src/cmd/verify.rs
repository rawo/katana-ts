use std::time::Duration;

use anyhow::bail;
use katana_proto::{read_patch, read_slot, verify_write_back, MidirTransport, SLOTS};

const TIMEOUT: Duration = Duration::from_secs(2);

/// Writes a different stored patch into the current patch and reads it back,
/// then restores the original and reads that back. `slot` picks the stored
/// patch; by default the first one that differs from the current patch.
pub fn run(port: &str, slot: Option<u8>) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let current = read_patch(&mut t, TIMEOUT)?;
    let (slot, scratch) = match slot {
        Some(n) => (n, read_slot(&mut t, n, TIMEOUT)?),
        None => {
            let mut found = None;
            for n in 1..=SLOTS {
                let p = read_slot(&mut t, n, TIMEOUT)?;
                if p != current {
                    found = Some((n, p));
                    break;
                }
            }
            match found {
                Some(f) => f,
                None => bail!("every stored slot equals the current patch; nothing different to write"),
            }
        }
    };
    println!("writing stored slot PATCH({slot}) into the current patch, then restoring…");
    verify_write_back(&mut t, &scratch, TIMEOUT)?;
    println!("identical: slot PATCH({slot}) written and read back, original restored and read back");
    Ok(())
}
