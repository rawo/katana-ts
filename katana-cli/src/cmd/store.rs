//! `store <file> <channel>`: a `.syx` patch, or one patch of a `.tsl` set, into a stored channel.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use katana_proto::amp::store_patch;
use katana_proto::{read_patch, read_slot, Channel, MidirTransport, Patch};

use crate::ask::confirm_stdin;
use crate::backup::{backup_dir, backup_name, write_backup};
use crate::files::read_patch_file;

const TIMEOUT: Duration = Duration::from_secs(2);

/// Everything that can be checked before the amp is touched.
fn prepare(file: &Path, channel: &str, patch: Option<usize>) -> anyhow::Result<(Channel, Patch)> {
    let ch: Channel = channel.parse()?;
    Ok((ch, read_patch_file(file, patch)?))
}

pub fn run(port: &str, file: &Path, channel: &str, patch: Option<usize>, yes: bool) -> anyhow::Result<()> {
    let (ch, p) = prepare(file, channel, patch)?;
    let mut t = MidirTransport::open(port)?;
    let old = read_slot(&mut t, ch.slot(), TIMEOUT)?;
    let current = read_patch(&mut t, TIMEOUT)?;
    let dir = backup_dir()?;
    let now = chrono::Local::now().naive_local();
    let old_backup = write_backup(&dir, &backup_name(now, &ch.to_string(), &old.name()), &old)?;
    println!("backup of {ch} '{}': {}", old.name(), old_backup.display());
    let current_backup = write_backup(&dir, &backup_name(now, "current", &current.name()), &current)?;
    println!("backup of the current sound '{}': {}", current.name(), current_backup.display());

    if !confirm_stdin(&format!("{ch} currently '{}' → replace with '{}'?", old.name(), p.name()), yes)? {
        println!("nothing changed");
        return Ok(());
    }
    // Ctrl-C waits until the amp is out of editor mode with the store finished or failed.
    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = interrupted.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)).context("installing the Ctrl-C handler")?;

    let report = store_patch(&mut t, ch, &p, TIMEOUT).with_context(|| {
        format!("storing into {ch} failed; backups: {}, {}", old_backup.display(), current_backup.display())
    })?;
    println!("stored '{}' in {ch}; the amp is now on {ch}", p.name());
    if !report.solo_delay_stored {
        println!("solo delay not stored");
    }
    if interrupted.load(Ordering::SeqCst) {
        std::process::exit(130);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::tests::temp_dir;

    #[test]
    fn prepare_checks_before_amp() {
        let d = temp_dir("prepare");
        let syx = d.join("a.syx");
        let p = Patch::default_preset();
        std::fs::write(&syx, p.to_syx()).unwrap();
        assert!(prepare(&syx, "C1", None).is_err());
        assert_eq!(prepare(&syx, "a3", None).unwrap(), (Channel::A3, p.clone()));
        let tsl = d.join("a.tsl");
        std::fs::write(&tsl, katana_proto::tsl::write_tsl(&p)).unwrap();
        assert!(prepare(&tsl, "A3", None).unwrap_err().to_string().contains("--patch"));
    }
}
