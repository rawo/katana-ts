//! The patch being edited, and whether it differs from what was last opened or saved.

use std::path::{Path, PathBuf};

use katana_proto::{ParamDef, Patch, ProtoError};

/// The library file the editor's patch came from; Save and Quick save write there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The default preset, or a file from outside the library.
    None,
    Patch(PathBuf),
    QuickSave(PathBuf),
}

impl Origin {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Origin::None => None,
            Origin::Patch(p) | Origin::QuickSave(p) => Some(p),
        }
    }
}

pub struct Editor {
    patch: Patch,
    /// What was last opened or saved; the editor is dirty while it differs.
    baseline: Patch,
    origin: Origin,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Editor {
        let patch = Patch::default_preset();
        Editor { baseline: patch.clone(), patch, origin: Origin::None }
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn get(&self, p: &ParamDef) -> Option<u32> {
        self.patch.get(p)
    }

    pub fn set(&mut self, p: &ParamDef, value: u32) -> Result<(), ProtoError> {
        self.patch.set(p, value)
    }

    pub fn is_dirty(&self) -> bool {
        self.patch != self.baseline
    }

    pub fn name(&self) -> String {
        let name = self.patch.name();
        if name.is_empty() {
            "(unnamed)".to_string()
        } else {
            name
        }
    }

    /// The name the user sees: the origin's file name without `.syx`, else
    /// the name stored in the patch.
    pub fn display_name(&self) -> String {
        match self.origin.path().and_then(Path::file_stem) {
            Some(stem) => stem.to_string_lossy().into_owned(),
            None => self.name(),
        }
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub fn path(&self) -> Option<&Path> {
        self.origin.path()
    }

    /// The patch that was last opened or saved.
    pub fn baseline(&self) -> &Patch {
        &self.baseline
    }

    /// Replaces the editor's patch; it is clean afterwards.
    pub fn load(&mut self, patch: Patch, origin: Origin) {
        self.baseline = patch.clone();
        self.patch = patch;
        self.origin = origin;
    }

    /// Replaces the editor's patch with a `.syx` file's; nothing changes on error.
    pub fn open_bytes(&mut self, bytes: &[u8], path: PathBuf) -> Result<(), ProtoError> {
        self.load(Patch::from_syx(bytes)?, Origin::Patch(path));
        Ok(())
    }

    /// The origin file is gone: keep the patch, with no file behind it.
    pub fn forget_origin(&mut self) {
        self.origin = Origin::None;
    }

    /// Records a successful save of `patch` (the editor's patch as written,
    /// e.g. renamed by Save as…): the editor holds it and is clean again.
    pub fn saved_as(&mut self, patch: Patch, origin: Origin) {
        self.load(patch, origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use katana_proto::{find_param, Patch};

    #[test]
    fn new_editor_is_clean_default() {
        let e = Editor::new();
        assert!(!e.is_dirty());
        assert_eq!(e.name(), "DEFAULT");
    }

    #[test]
    fn set_marks_dirty_and_setting_back_clears_it() {
        let mut e = Editor::new();
        let g = find_param("gain").unwrap();
        e.set(g, 5).unwrap();
        assert!(e.is_dirty());
        e.set(g, 0).unwrap();
        assert!(!e.is_dirty());
    }

    #[test]
    fn open_rejects_non_patch_keeps_editor() {
        let mut e = Editor::new();
        e.set(find_param("gain").unwrap(), 5).unwrap();
        assert!(e.open_bytes(b"not sysex", "x.syx".into()).is_err());
        assert_eq!(e.get(find_param("gain").unwrap()), Some(5));
        assert!(e.is_dirty());
        assert!(e.path().is_none());
    }

    #[test]
    fn open_then_saved_round_trip() {
        let gain = find_param("gain").unwrap();
        let mut src = Patch::default_preset();
        src.set(gain, 60).unwrap();
        let mut e = Editor::new();
        e.open_bytes(&src.to_syx(), "a.syx".into()).unwrap();
        assert!(!e.is_dirty());
        assert_eq!(e.get(gain), Some(60));
        e.set(gain, 61).unwrap();
        e.saved_as(e.patch().clone(), Origin::Patch("b.syx".into()));
        let bytes = e.patch().to_syx();
        assert!(!e.is_dirty());
        assert_eq!(e.path(), Some(std::path::Path::new("b.syx")));
        assert_eq!(Patch::from_syx(&bytes).unwrap().get(gain), Some(61));
    }
}
