//! The preset library on disk: the user's patches, quick saves and downloaded
//! Tone Exchange sets, plus a small index of read-only flags and where picked
//! patches came from. Nothing here stops the app: broken files are skipped and
//! named in `problems`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use katana_proto::tsl::ToneSet;
use katana_proto::Patch;
use serde::{Deserialize, Serialize};

pub const PATCHES: &str = "patches";
pub const QUICK_SAVES: &str = "quick-saves";
pub const SETS: &str = "sets";
pub const AUTOSAVE: &str = "autosave";
const INDEX: &str = "index.json";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Patch,
    QuickSave,
}

/// A `.syx` in My patches or Quick saves.
#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: Kind,
    pub path: PathBuf,
    /// The file name without `.syx`: the name the user sees everywhere.
    pub name: String,
    pub modified: SystemTime,
    pub read_only: bool,
}

/// A downloaded `.tsl` set.
#[derive(Clone, Debug)]
pub struct SetEntry {
    pub path: PathBuf,
    /// The set name stored in the file.
    pub name: String,
    pub modified: SystemTime,
    pub patches: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sort {
    #[default]
    NewestFirst,
    OldestFirst,
    NameAz,
    NameZa,
}

/// Where a picked patch came from.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Source {
    set: String,
    patch: String,
}

/// `index.json`; paths are relative to the library root, with `/`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    read_only: BTreeSet<String>,
    #[serde(default)]
    source: BTreeMap<String, Source>,
}

pub struct Library {
    root: Option<PathBuf>,
    index: Index,
    entries: Vec<Entry>,
    sets: Vec<SetEntry>,
    /// Things skipped or reset while reading the library, for the status line.
    pub problems: Vec<String>,
}

impl Library {
    /// The XDG data folder (`~/.local/share/katana-ts` on Linux).
    pub fn default_root() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "katana-ts").map(|d| d.data_dir().to_path_buf())
    }

    /// Creates the subfolders, reads the index and lists everything. Never fails.
    pub fn open(root: PathBuf) -> Library {
        let mut problems = Vec::new();
        for d in [PATCHES, QUICK_SAVES, SETS, AUTOSAVE] {
            if let Err(e) = std::fs::create_dir_all(root.join(d)) {
                problems.push(format!("Couldn't create {}: {e}", root.join(d).display()));
            }
        }
        let index = read_index(&root, &mut problems);
        let mut lib = Library { root: Some(root), index, entries: Vec::new(), sets: Vec::new(), problems: Vec::new() };
        lib.refresh();
        problems.append(&mut lib.problems);
        lib.problems = problems;
        lib
    }

    /// No folder: lists nothing, and every write fails.
    pub fn detached() -> Library {
        Library { root: None, index: Index::default(), entries: Vec::new(), sets: Vec::new(), problems: Vec::new() }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Lists the folders again; `problems` then holds what this listing skipped.
    pub fn refresh(&mut self) {
        self.problems.clear();
        self.entries.clear();
        self.sets.clear();
        let Some(root) = self.root.clone() else { return };
        for (dir, kind) in [(PATCHES, Kind::Patch), (QUICK_SAVES, Kind::QuickSave)] {
            for (path, modified) in files(&root.join(dir), "syx") {
                match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| check_syx(&b)) {
                    Ok(()) => self.entries.push(Entry {
                        kind,
                        name: stem(&path),
                        read_only: self.is_read_only(&path),
                        path,
                        modified,
                    }),
                    Err(e) => self.problems.push(format!("Skipped {}: {e}", file_name(&path))),
                }
            }
        }
        for (path, modified) in files(&root.join(SETS), "tsl") {
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| ToneSet::parse(&b).map_err(|e| e.to_string()))
            {
                Ok(set) => self.sets.push(SetEntry {
                    name: if set.name.is_empty() { stem(&path) } else { set.name },
                    patches: set.patches.into_iter().map(|p| p.name).collect(),
                    path,
                    modified,
                }),
                Err(e) => self.problems.push(format!("Skipped {}: {e}", file_name(&path))),
            }
        }
    }

    /// My patches and quick saves together.
    pub fn patches(&self) -> &[Entry] {
        &self.entries
    }

    pub fn sets(&self) -> &[SetEntry] {
        &self.sets
    }

    pub fn is_read_only(&self, path: &Path) -> bool {
        self.key(path).is_some_and(|k| self.index.read_only.contains(&k))
    }

    pub fn set_read_only(&mut self, path: &Path, on: bool) -> Result<(), String> {
        let key = self.key(path).ok_or_else(|| format!("{} is not in the library", file_name(path)))?;
        let mut index = self.index.clone();
        if on {
            index.read_only.insert(key);
        } else {
            index.read_only.remove(&key);
        }
        self.save_index(index)?;
        for e in self.entries.iter_mut().filter(|e| e.path == path) {
            e.read_only = on;
        }
        Ok(())
    }

    /// Copies a `.syx` into My patches or a `.tsl` into Downloaded sets
    /// (numbered name if taken), after checking it. The source is unchanged.
    pub fn import(&mut self, from: &Path) -> Result<PathBuf, String> {
        let cant = |e: String| format!("Can't import {}: {e}", file_name(from));
        let ext = from.extension().map(|e| e.to_string_lossy().to_lowercase());
        let dir = match ext.as_deref() {
            Some("syx") => PATCHES,
            Some("tsl") => SETS,
            _ => return Err(cant("not a .syx or .tsl".into())),
        };
        let bytes = std::fs::read(from).map_err(|e| cant(e.to_string()))?;
        if dir == PATCHES {
            check_syx(&bytes).map_err(cant)?;
        } else {
            ToneSet::parse(&bytes).map_err(|e| cant(e.to_string()))?;
        }
        let ext = if dir == PATCHES { "syx" } else { "tsl" };
        self.write_new(dir, &stem(from), ext, &bytes)
    }

    /// Saves patch `index` of a downloaded set as a new patch in My patches,
    /// named after it, and returns it. The set file is never changed.
    pub fn pick(&mut self, set: &Path, index: usize) -> Result<(PathBuf, Patch), String> {
        let bytes = std::fs::read(set).map_err(|e| format!("Couldn't read {}: {e}", file_name(set)))?;
        let tone_set = ToneSet::parse(&bytes).map_err(|e| format!("Couldn't read {}: {e}", file_name(set)))?;
        let entry = tone_set
            .patches
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("{} has no patch {}", tone_set.name, index + 1))?;
        let patch = entry.patch.map_err(|e| format!("Can't use {} from {}: {e}", entry.name, tone_set.name))?;
        let path = self.write_new(PATCHES, &safe_stem(&entry.name), "syx", &patch.to_syx())?;
        if let Some(key) = self.key(&path) {
            let mut index = self.index.clone();
            index.source.insert(key, Source { set: tone_set.name, patch: entry.name });
            // Only metadata: the patch itself is saved either way.
            let _ = self.save_index(index);
        }
        Ok((path, patch))
    }

    /// A new named patch in My patches; the name is also written into the patch.
    pub fn save_as(&mut self, name: &str, patch: &Patch) -> Result<PathBuf, String> {
        let mut p = patch.clone();
        p.set_name(name.trim());
        self.write_new(PATCHES, &safe_stem(name), "syx", &p.to_syx())
    }

    /// A new `quick-save_<local time>.syx` in Quick saves.
    pub fn new_quick_save(&mut self, patch: &Patch, now: chrono::NaiveDateTime) -> Result<PathBuf, String> {
        let stem = format!("quick-save_{}", now.format("%Y-%m-%d_%H-%M-%S"));
        self.write_new(QUICK_SAVES, &stem, "syx", &patch.to_syx())
    }

    /// Replaces a library patch; refused while it is read-only.
    pub fn overwrite(&mut self, path: &Path, patch: &Patch) -> Result<(), String> {
        if self.is_read_only(path) {
            return Err(format!("{} is read-only", stem(path)));
        }
        write_atomic(path, &patch.to_syx()).map_err(|e| format!("Couldn't save {}: {e}", file_name(path)))?;
        self.refresh();
        Ok(())
    }

    /// Deletes a patch, quick save or set; refused while it is read-only.
    pub fn delete(&mut self, path: &Path) -> Result<(), String> {
        if self.is_read_only(path) {
            return Err(format!("{} is read-only", stem(path)));
        }
        std::fs::remove_file(path).map_err(|e| format!("Couldn't delete {}: {e}", file_name(path)))?;
        if let Some(key) = self.key(path) {
            let mut index = self.index.clone();
            index.read_only.remove(&key);
            if index.source.remove(&key).is_some() {
                let _ = self.save_index(index);
            }
        }
        self.refresh();
        Ok(())
    }

    pub fn load(&self, path: &Path) -> Result<Patch, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't open {}: {e}", file_name(path)))?;
        Patch::from_syx(&bytes).map_err(|e| format!("Couldn't open {}: {e}", file_name(path)))
    }

    /// Which list a library file belongs to, by its folder.
    pub fn kind_of(&self, path: &Path) -> Option<Kind> {
        let root = self.root.as_ref()?;
        let parent = path.parent()?;
        if parent == root.join(PATCHES) {
            Some(Kind::Patch)
        } else if parent == root.join(QUICK_SAVES) {
            Some(Kind::QuickSave)
        } else {
            None
        }
    }

    /// Writes `bytes` to a free `<stem>.<ext>` in subfolder `dir`.
    fn write_new(&mut self, dir: &str, stem: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let root = self.root.as_ref().ok_or("No library folder")?;
        let path = free_path(&root.join(dir), stem, ext);
        write_atomic(&path, bytes).map_err(|e| format!("Couldn't save {}: {e}", file_name(&path)))?;
        self.refresh();
        Ok(path)
    }

    /// `path` relative to the root, as stored in the index.
    fn key(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(self.root.as_ref()?).ok()?;
        Some(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
    }

    fn save_index(&mut self, index: Index) -> Result<(), String> {
        let root = self.root.as_ref().ok_or("No library folder")?;
        let text = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
        write_atomic(&root.join(INDEX), text.as_bytes()).map_err(|e| format!("Couldn't write {INDEX}: {e}"))?;
        self.index = index;
        Ok(())
    }
}

fn read_index(root: &Path, problems: &mut Vec<String>) -> Index {
    let path = root.join(INDEX);
    let Ok(text) = std::fs::read(&path) else { return Index::default() };
    match serde_json::from_slice(&text) {
        Ok(index) => index,
        Err(_) => {
            let _ = std::fs::rename(&path, root.join(format!("{INDEX}.bad")));
            problems.push(format!("{INDEX} is damaged; read-only flags reset"));
            Index::default()
        }
    }
}

fn check_syx(bytes: &[u8]) -> Result<(), String> {
    Patch::from_syx(bytes).map(|_| ()).map_err(|e| e.to_string())
}

/// Regular files in `dir` with extension `ext` (any case), with their modified time.
fn files(dir: &Path, ext: &str) -> Vec<(PathBuf, SystemTime)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok().filter(|m| m.is_file())?;
            Some((p, m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
        })
        .collect()
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// A readable file name: `/ \ : * ? " < > |` and control characters become
/// `_`; surrounding spaces trimmed; nothing left gives "patch".
pub fn safe_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_control() || r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect();
    let s = s.trim();
    if s.is_empty() {
        "patch".to_string()
    } else {
        s.to_string()
    }
}

/// `dir/stem.ext`, or `dir/stem (2).ext`, `(3)`, … when taken. "Taken" ignores
/// letter case, as macOS file systems do.
pub fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let taken: BTreeSet<String> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect())
        .unwrap_or_default();
    (1..)
        .map(|n| if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} ({n}).{ext}") })
        .find(|name| !taken.contains(&name.to_lowercase()))
        .map(|name| dir.join(name))
        .expect("some number is free")
}

/// Search: true for an empty query, or when `name` contains any of the query's
/// words, ignoring case.
pub fn matches(query: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    let mut words = query.split_whitespace().peekable();
    words.peek().is_none() || words.any(|w| name.contains(&w.to_lowercase()))
}

pub fn sort_entries(v: &mut [Entry], sort: Sort) {
    match sort {
        Sort::NewestFirst => v.sort_by_key(|e| std::cmp::Reverse(e.modified)),
        Sort::OldestFirst => v.sort_by_key(|e| e.modified),
        Sort::NameAz => v.sort_by_key(|e| e.name.to_lowercase()),
        Sort::NameZa => v.sort_by_key(|e| std::cmp::Reverse(e.name.to_lowercase())),
    }
}

/// Writes `<file name>.tmp` next to `path`, then renames it over `path`, so a
/// failed save never leaves a half-written file in place of the old one.
/// A symlink is followed (the file it points to is replaced, not the link), and
/// an existing file keeps its permissions.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut tmp = target.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let old_permissions = std::fs::metadata(&target).map(|m| m.permissions()).ok();
    std::fs::write(&tmp, bytes)
        .and_then(|()| old_permissions.map_or(Ok(()), |p| std::fs::set_permissions(&tmp, p)))
        .and_then(|()| std::fs::rename(&tmp, &target))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
}
