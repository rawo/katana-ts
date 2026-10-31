//! Backups of a channel (and of the current sound) before a store overwrites it.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Patch;

/// `<stamp>_<what>_<patch name>.syx`; `stamp` is `%Y-%m-%d_%H-%M-%S`, `what` a
/// channel or `current`. The name keeps `[A-Za-z0-9 _-]`; anything else becomes `_`.
pub fn backup_name(stamp: &str, what: &str, patch_name: &str) -> String {
    let safe: String =
        patch_name.chars().map(|c| if c.is_ascii_alphanumeric() || " _-".contains(c) { c } else { '_' }).collect();
    let safe = safe.trim();
    let safe = if safe.is_empty() { "unnamed" } else { safe };
    format!("{stamp}_{what}_{safe}.syx")
}

/// Writes `p` as `dir/name`, or `name` with `_2`, `_3`, … before `.syx` if taken.
pub fn write_backup(dir: &Path, name: &str, p: &Patch) -> std::io::Result<PathBuf> {
    let stem = name.strip_suffix(".syx").unwrap_or(name);
    let mut n = 1;
    loop {
        let path = if n == 1 { dir.join(name) } else { dir.join(format!("{stem}_{n}.syx")) };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(&p.to_syx())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(test: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("katana-proto-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn backup_name_format() {
        assert_eq!(backup_name("2026-10-29_21-05-09", "A3", "Riff Shred"), "2026-10-29_21-05-09_A3_Riff Shred.syx");
    }

    #[test]
    fn backup_name_is_safe() {
        let n = backup_name("2026-10-29_21-05-09", "current", "../a/b");
        assert!(!n.contains('/') && !n.contains(".."), "{n}");
        assert_eq!(backup_name("2026-10-29_21-05-09", "A3", "  "), "2026-10-29_21-05-09_A3_unnamed.syx");
    }

    #[test]
    fn write_backup_never_overwrites() {
        let d = temp_dir("backup");
        let p = Patch::default_preset();
        let a = write_backup(&d, "x.syx", &p).unwrap();
        let b = write_backup(&d, "x.syx", &p).unwrap();
        assert_eq!(a, d.join("x.syx"));
        assert_eq!(b, d.join("x_2.syx"));
        assert_eq!(Patch::from_syx(&std::fs::read(b).unwrap()).unwrap(), p);
    }
}
