//! Patch files by extension: `.syx` (DT1 messages) or `.tsl` (Tone Exchange set).

use std::path::Path;

use anyhow::{bail, Context};
use katana_proto::tsl::{write_tsl, ToneSet};
use katana_proto::Patch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Syx,
    Tsl,
}

pub fn format_of(path: &Path) -> anyhow::Result<Format> {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("syx") => Ok(Format::Syx),
        Some("tsl") => Ok(Format::Tsl),
        _ => bail!("{}: use a .syx or .tsl file", path.display()),
    }
}

/// Reads a whole patch: a `.syx` file, or patch `patch` (from 1) of a `.tsl` set.
pub fn read_patch_file(path: &Path, patch: Option<usize>) -> anyhow::Result<Patch> {
    let format = format_of(path)?;
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    match (format, patch) {
        (Format::Syx, Some(_)) => bail!("--patch only applies to .tsl files"),
        (Format::Syx, None) => {
            Patch::from_syx(&bytes).with_context(|| format!("{} is not a complete KATANA Gen 3 patch", path.display()))
        }
        (Format::Tsl, n) => {
            let set = ToneSet::parse(&bytes).with_context(|| format!("reading {}", path.display()))?;
            let len = set.patches.len();
            let Some(n) = n else {
                bail!(
                    "{} is a set: choose a patch with --patch N (1–{len}); see katana-cli tsl-list {}",
                    path.display(),
                    path.display()
                )
            };
            let Some(entry) = n.checked_sub(1).and_then(|i| set.patches.get(i)) else {
                bail!("{} has patches 1–{len}, not {n}", path.display())
            };
            match &entry.patch {
                Ok(p) => Ok(p.clone()),
                Err(e) => bail!("patch {n} '{}' can't be loaded: {e}", entry.name),
            }
        }
    }
}

/// A whole file holding `p` in format `f`.
pub fn patch_bytes(f: Format, p: &Patch) -> Vec<u8> {
    match f {
        Format::Syx => p.to_syx(),
        Format::Tsl => write_tsl(p),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use katana_proto::tsl::write_tsl;
    use std::path::PathBuf;

    /// A fresh empty folder under the system temp dir.
    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("katana-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn named(name: &str) -> Patch {
        let mut p = Patch::default_preset();
        p.set_name(name);
        p
    }

    /// A two-patch set file: "One", "Two".
    fn two_patch_set(dir: &Path) -> PathBuf {
        let mut doc: serde_json::Value = serde_json::from_slice(&write_tsl(&named("One"))).unwrap();
        let two: serde_json::Value = serde_json::from_slice(&write_tsl(&named("Two"))).unwrap();
        doc["data"][0].as_array_mut().unwrap().push(two["data"][0][0].clone());
        let path = dir.join("set.tsl");
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        path
    }

    #[test]
    fn format_by_extension() {
        assert!(matches!(format_of(Path::new("a.syx")).unwrap(), Format::Syx));
        assert!(matches!(format_of(Path::new("a.SYX")).unwrap(), Format::Syx));
        assert!(matches!(format_of(Path::new("a.Tsl")).unwrap(), Format::Tsl));
        for bad in ["a.json", "a"] {
            assert!(format_of(Path::new(bad)).unwrap_err().to_string().contains(".syx or .tsl"), "{bad}");
        }
    }

    #[test]
    fn tsl_needs_patch_number() {
        let d = temp_dir("tsl-needs");
        let set = two_patch_set(&d);
        let e = read_patch_file(&set, None).unwrap_err().to_string();
        assert!(e.contains("--patch") && e.contains("1–2"), "{e}");
        assert_eq!(read_patch_file(&set, Some(2)).unwrap().name(), "Two");
    }

    #[test]
    fn syx_refuses_patch_number() {
        let d = temp_dir("syx-refuses");
        let f = d.join("a.syx");
        std::fs::write(&f, named("A").to_syx()).unwrap();
        assert_eq!(read_patch_file(&f, None).unwrap().name(), "A");
        assert!(read_patch_file(&f, Some(1)).unwrap_err().to_string().contains("--patch only applies to .tsl"));
    }

    #[test]
    fn tsl_patch_number_out_of_range() {
        let d = temp_dir("tsl-range");
        let set = two_patch_set(&d);
        for n in [0, 3] {
            assert!(read_patch_file(&set, Some(n)).unwrap_err().to_string().contains("1–2"), "{n}");
        }
    }

    #[test]
    fn tsl_broken_patch_is_refused() {
        let d = temp_dir("tsl-broken");
        let mut doc: serde_json::Value = serde_json::from_slice(&write_tsl(&named("Bad"))).unwrap();
        doc["data"][0][0]["paramSet"]["PATCH%COLOR"].as_array_mut().unwrap().push("01".into());
        let f = d.join("bad.tsl");
        std::fs::write(&f, serde_json::to_vec(&doc).unwrap()).unwrap();
        let e = read_patch_file(&f, Some(1)).unwrap_err().to_string();
        assert!(e.contains("patch 1 'Bad' can't be loaded"), "{e}");
    }

    #[test]
    fn patch_bytes_round_trip() {
        let d = temp_dir("bytes");
        let p = named("Round");
        for (f, name) in [(Format::Syx, "r.syx"), (Format::Tsl, "r.tsl")] {
            let path = d.join(name);
            std::fs::write(&path, patch_bytes(f, &p)).unwrap();
            let patch = if name.ends_with("tsl") { Some(1) } else { None };
            assert_eq!(read_patch_file(&path, patch).unwrap(), p);
        }
    }
}
