//! Backups of a channel (and of the current sound) before `store` overwrites it.

use std::path::{Path, PathBuf};

use anyhow::Context;
use katana_proto::Patch;

/// `~/.local/share/katana-ts/backups` on Linux, created if missing.
pub fn backup_dir() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "katana-ts").context("no home folder for backups")?;
    let dir = dirs.data_dir().join("backups");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

/// `<date_time>_<what>_<patch name>.syx`; `what` is a channel or `current`.
pub fn backup_name(now: chrono::NaiveDateTime, what: &str, patch_name: &str) -> String {
    katana_proto::backup::backup_name(&now.format("%Y-%m-%d_%H-%M-%S").to_string(), what, patch_name)
}

/// Writes `p` as `dir/name`, or `name` with `_2`, `_3`, … before `.syx` if taken.
pub fn write_backup(dir: &Path, name: &str, p: &Patch) -> anyhow::Result<PathBuf> {
    katana_proto::backup::write_backup(dir, name, p).with_context(|| format!("writing a backup in {}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn backup_name_format() {
        assert_eq!(backup_name(at("2026-10-29 21:05:09"), "A3", "Riff Shred"), "2026-10-29_21-05-09_A3_Riff Shred.syx");
    }
}
