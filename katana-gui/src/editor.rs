//! The patch being edited, and whether it differs from what was last opened or saved.

use std::path::{Path, PathBuf};

use katana_proto::{Address, ParamDef, Patch, ProtoError};

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
    /// The amp's settings replaced a library preset's; the origin is still that file.
    replaced: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Editor {
        let patch = Patch::default_preset();
        Editor { baseline: patch.clone(), patch, origin: Origin::None, replaced: false }
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
        self.replaced = false;
    }

    /// Replaces the editor's patch with a `.syx` file's; nothing changes on error.
    pub fn open_bytes(&mut self, bytes: &[u8], path: PathBuf) -> Result<(), ProtoError> {
        self.load(Patch::from_syx(bytes)?, Origin::Patch(path));
        Ok(())
    }

    /// Puts back an auto-saved state: `patch` being edited, `baseline` what
    /// its origin holds on disk.
    pub fn restore(&mut self, patch: Patch, baseline: Patch, origin: Origin) {
        self.patch = patch;
        self.baseline = baseline;
        self.origin = origin;
        self.replaced = false;
    }

    /// The origin file is gone: keep the patch, with no file behind it.
    pub fn forget_origin(&mut self) {
        self.origin = Origin::None;
        self.replaced = false;
    }

    /// The amp's settings take over the editor; clean afterwards. The origin
    /// stays, so Save would overwrite that file (`replaced_note` says so).
    pub fn replace_from_amp(&mut self, patch: Patch) {
        self.baseline = patch.clone();
        self.patch = patch;
        self.replaced = self.origin.path().is_some();
    }

    /// A change made on the amp: counts as an edit, like one made in the app.
    pub fn apply_amp_change(&mut self, addr: u32, data: &[u8]) {
        self.patch.mem.apply(Address(addr), data);
    }

    /// Set while the amp's settings replaced a library preset.
    pub fn replaced_note(&self) -> Option<String> {
        let file = self.origin.path()?.file_name()?.to_string_lossy();
        self.replaced.then(|| format!("Saving will overwrite library preset {file}"))
    }

    /// Whether a restart must restore the editor's content and not re-read its
    /// origin file: edited, or replaced by the amp's settings.
    pub fn needs_restore_as_is(&self) -> bool {
        self.is_dirty() || self.replaced
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

    const GAIN: u32 = katana_proto::device::addr([0x20, 0x00, 0x06, 0x00]);

    fn gain_patch(v: u32) -> Patch {
        let mut p = Patch::default_preset();
        p.set(find_param("gain").unwrap(), v).unwrap();
        p
    }

    fn from_library() -> Editor {
        let mut e = Editor::new();
        e.load(gain_patch(10), Origin::Patch("/lib/patches/Riff.syx".into()));
        e
    }

    #[test]
    fn replace_keeps_origin_and_is_clean() {
        let mut e = from_library();
        e.replace_from_amp(gain_patch(20));
        assert_eq!(e.patch(), &gain_patch(20));
        assert!(!e.is_dirty());
        assert_eq!(e.path(), Some(Path::new("/lib/patches/Riff.syx")));
    }

    #[test]
    fn amp_change_makes_dirty() {
        let mut e = from_library();
        e.apply_amp_change(GAIN, &[33]);
        assert_eq!(e.get(find_param("gain").unwrap()), Some(33));
        assert!(e.is_dirty());
    }

    #[test]
    fn note_names_the_library_file_while_replaced() {
        let mut e = from_library();
        assert_eq!(e.replaced_note(), None);
        e.replace_from_amp(gain_patch(20));
        assert_eq!(e.replaced_note().as_deref(), Some("Saving will overwrite library preset Riff.syx"));
    }

    #[test]
    fn note_only_for_library_origin() {
        let mut e = Editor::new();
        e.replace_from_amp(gain_patch(20));
        assert_eq!(e.replaced_note(), None);
    }

    #[test]
    fn load_clears_note() {
        let mut e = from_library();
        e.replace_from_amp(gain_patch(20));
        e.load(gain_patch(1), Origin::None);
        assert_eq!(e.replaced_note(), None);
    }

    #[test]
    fn restore_save_and_forget_clear_note() {
        let replaced = || {
            let mut e = from_library();
            e.replace_from_amp(gain_patch(20));
            e
        };
        let mut e = replaced();
        e.restore(gain_patch(1), gain_patch(1), Origin::None);
        assert_eq!(e.replaced_note(), None);
        let mut e = replaced();
        e.saved_as(gain_patch(1), Origin::Patch("b.syx".into()));
        assert_eq!(e.replaced_note(), None);
        let mut e = replaced();
        e.forget_origin();
        assert_eq!(e.replaced_note(), None);
    }

    #[test]
    fn replaced_content_must_be_restored_as_is() {
        let mut e = from_library();
        assert!(!e.needs_restore_as_is());
        e.replace_from_amp(gain_patch(20));
        assert!(!e.is_dirty());
        assert!(e.needs_restore_as_is());
    }

    #[test]
    fn dirty_content_must_be_restored_as_is() {
        let mut e = from_library();
        e.set(find_param("gain").unwrap(), 11).unwrap();
        assert!(e.needs_restore_as_is());
    }

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
