use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context};
use katana_proto::{read_patch, read_slot, Channel, MidirTransport};

use crate::ask::confirm_stdin;
use crate::files::{format_of, patch_bytes, Format};

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq)]
pub enum DumpPlan {
    /// The current patch into a `.syx` file, as before stored channels existed.
    Current(PathBuf),
    Channel(Channel, PathBuf, Format),
}

/// `dump <file>` or `dump <channel> <file>`; checked before the amp is opened.
pub fn plan(first: &str, second: Option<&Path>) -> anyhow::Result<DumpPlan> {
    match (first.parse::<Channel>(), second) {
        (Ok(ch), None) => bail!("dump {ch} needs a file: katana-cli dump {ch} <file.syx|file.tsl>"),
        (Err(_), None) => Ok(DumpPlan::Current(PathBuf::from(first))),
        (Ok(ch), Some(file)) => Ok(DumpPlan::Channel(ch, file.to_path_buf(), format_of(file)?)),
        (Err(e), Some(_)) => Err(e.into()),
    }
}

pub fn run(port: &str, first: &str, second: Option<&Path>, yes: bool) -> anyhow::Result<()> {
    match plan(first, second)? {
        DumpPlan::Current(out) => run_current(port, &out),
        DumpPlan::Channel(ch, out, format) => {
            if out.exists() && !confirm_stdin(&format!("Overwrite {}?", out.display()), yes)? {
                println!("nothing written");
                return Ok(());
            }
            let mut t = MidirTransport::open(port)?;
            let p = read_slot(&mut t, ch.slot(), TIMEOUT)?;
            std::fs::write(&out, patch_bytes(format, &p)).with_context(|| format!("writing {}", out.display()))?;
            println!("wrote {}: {ch} '{}'", out.display(), p.name());
            Ok(())
        }
    }
}

fn run_current(port: &str, out: &Path) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let p = read_patch(&mut t, TIMEOUT)?;
    std::fs::write(out, p.to_syx()).with_context(|| format!("writing {}", out.display()))?;
    println!("wrote {} ({} bytes of patch data)", out.display(), p.mem.0.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_channel_without_file_is_refused() {
        assert!(plan("a3", None).unwrap_err().to_string().contains("needs a file"));
        assert_eq!(plan("now.syx", None).unwrap(), DumpPlan::Current(PathBuf::from("now.syx")));
        assert_eq!(
            plan("A3", Some(Path::new("x.TSL"))).unwrap(),
            DumpPlan::Channel(Channel::A3, PathBuf::from("x.TSL"), Format::Tsl)
        );
        assert!(plan("now.syx", Some(Path::new("x.syx"))).is_err());
        assert!(plan("A3", Some(Path::new("x.txt"))).is_err());
    }
}
