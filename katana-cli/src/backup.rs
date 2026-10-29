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
    let safe: String =
        patch_name.chars().map(|c| if c.is_ascii_alphanumeric() || " _-".contains(c) { c } else { '_' }).collect();
    let safe = safe.trim();
    let safe = if safe.is_empty() { "unnamed" } else { safe };
    format!("{}_{what}_{safe}.syx", now.format("%Y-%m-%d_%H-%M-%S"))
}

/// Writes `p` as `dir/name`, or `name` with `_2`, `_3`, … before `.syx` if taken.
pub fn write_backup(dir: &Path, name: &str, p: &Patch) -> anyhow::Result<PathBuf> {
    let stem = name.strip_suffix(".syx").unwrap_or(name);
    let mut n = 1;
    loop {
        let path = if n == 1 { dir.join(name) } else { dir.join(format!("{stem}_{n}.syx")) };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                f.write_all(&p.to_syx()).with_context(|| format!("writing {}", path.display()))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e).with_context(|| format!("writing {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::tests::temp_dir;

    fn at(s: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn backup_name_format() {
        assert_eq!(backup_name(at("2026-10-29 21:05:09"), "A3", "Riff Shred"), "2026-10-29_21-05-09_A3_Riff Shred.syx");
    }

    #[test]
    fn backup_name_is_safe() {
        let n = backup_name(at("2026-10-29 21:05:09"), "current", "../a/b");
        assert!(!n.contains('/') && !n.contains(".."), "{n}");
        assert_eq!(backup_name(at("2026-10-29 21:05:09"), "A3", "  "), "2026-10-29_21-05-09_A3_unnamed.syx");
    }

    #[test]
    fn write_backup_never_overwrites() {
        let d = temp_dir("backup");
        let p = katana_proto::Patch::default_preset();
        let a = write_backup(&d, "x.syx", &p).unwrap();
        let b = write_backup(&d, "x.syx", &p).unwrap();
        assert_eq!(a, d.join("x.syx"));
        assert_eq!(b, d.join("x_2.syx"));
        assert_eq!(katana_proto::Patch::from_syx(&std::fs::read(b).unwrap()).unwrap(), p);
    }
}
