mod common;

use std::path::{Path, PathBuf};

use common::{gain_of, names_in, patch, tmp, tsl};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::app::KatanaApp;
use katana_gui::editor::Origin;
use katana_gui::library::Library;
use katana_proto::{find_param, Patch};

fn harness() -> Harness<'static, KatanaApp> {
    Harness::builder()
        .with_size([1600.0, 960.0])
        .build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), KatanaApp::new())
}

fn lib_harness(root: &Path) -> Harness<'static, KatanaApp> {
    let app = KatanaApp::with_library(Library::open(root.to_path_buf()));
    Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app)
}

/// `root/patches/<name>.syx` with gain `gain`.
fn lib_patch(root: &Path, name: &str, gain: u32) -> PathBuf {
    let path = root.join("patches").join(format!("{name}.syx"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, patch(name, gain).to_syx()).unwrap();
    path
}

fn now(sec: u32) -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 10, 28).unwrap().and_hms_opt(15, 4, sec).unwrap()
}

/// The prompt's button, not the action bar's of the same name (the prompt sits lower).
fn prompt_button<'a>(h: &'a Harness<KatanaApp>, label: &'a str) -> egui_kittest::Node<'a> {
    h.get_all_by_label(label).max_by(|a, b| a.rect().top().total_cmp(&b.rect().top())).unwrap()
}

/// Writes a patch with gain 60 and returns its path.
fn patch_file(dir: &std::path::Path) -> PathBuf {
    let mut p = Patch::default_preset();
    p.set(find_param("gain").unwrap(), 60).unwrap();
    let path = dir.join("a.syx");
    std::fs::write(&path, p.to_syx()).unwrap();
    path
}

fn gain(h: &Harness<KatanaApp>) -> Option<u32> {
    h.state().editor.get(find_param("gain").unwrap())
}

fn set_gain(h: &mut Harness<KatanaApp>, v: u32) {
    h.state_mut().editor.set(find_param("gain").unwrap(), v).unwrap();
}

#[test]
fn open_valid_file_loads_and_is_clean() {
    let d = tmp("open-valid");
    let mut h = harness();
    h.state_mut().request_open(patch_file(&d));
    h.run();
    assert_eq!(gain(&h), Some(60));
    assert!(!h.state().editor.is_dirty());
    assert!(h.state().error.is_none());
}

#[test]
fn open_bad_file_shows_error() {
    let d = tmp("open-bad");
    let bad = d.join("bad.syx");
    std::fs::write(&bad, b"junk").unwrap();
    let mut h = harness();
    h.state_mut().request_open(bad);
    h.run();
    assert_eq!(gain(&h), Some(0));
    assert!(h.state().editor.path().is_none());
    let err = h.state().error.clone().unwrap();
    assert!(err.starts_with("Couldn't open bad.syx"), "{err}");
    h.get_by_label(&err);
}

#[test]
fn open_while_dirty_asks_first() {
    let d = tmp("open-dirty");
    let path = patch_file(&d);
    let mut h = harness();
    set_gain(&mut h, 5);
    h.state_mut().request_open(path.clone());
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
    assert_eq!(gain(&h), Some(5));

    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(gain(&h), Some(5));
    assert!(h.query_by_label("Discard unsaved changes to DEFAULT?").is_none());

    h.state_mut().request_open(path);
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    assert_eq!(gain(&h), Some(60));
    assert!(!h.state().editor.is_dirty());
}

#[test]
fn export_writes_copy_and_leaves_editor_alone() {
    let d = tmp("export");
    let out = d.join("out.syx");
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().export_to(out.clone());
    h.run();
    let saved = Patch::from_syx(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(saved.get(find_param("gain").unwrap()), Some(7));
    assert!(h.state().editor.is_dirty());
    assert_eq!(h.state().editor.origin(), &Origin::None);
    assert!(h.state().error.is_none());
}

#[test]
fn export_failure_is_reported() {
    let d = tmp("export-fail");
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().export_to(d.join("missing-dir").join("out.syx"));
    h.run();
    assert!(h.state().editor.is_dirty());
    let err = h.state().error.clone().unwrap();
    assert!(err.starts_with("Couldn't save out.syx"), "{err}");
}

#[test]
fn close_while_dirty_is_cancelled() {
    let mut h = harness();
    assert!(h.state_mut().on_close_requested());
    set_gain(&mut h, 5);
    assert!(!h.state_mut().on_close_requested());
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
}

#[test]
fn default_file_name_is_safe() {
    assert_eq!(katana_gui::app::default_file_name("Bloom Crunch/v2"), "Bloom_Crunch_v2.syx");
}

#[test]
fn export_adds_missing_extension() {
    let d = tmp("export-ext");
    let mut h = harness();
    h.state_mut().export_to(d.join("mine"));
    assert!(d.join("mine.syx").exists());
    assert!(h.state().editor.path().is_none());
}

#[test]
fn escape_cancels_discard_prompt() {
    let d = tmp("escape");
    let path = patch_file(&d);
    let mut h = harness();
    set_gain(&mut h, 5);
    h.state_mut().request_open(path.clone());
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
    h.key_press(eframe::egui::Key::Escape);
    h.run();
    assert!(h.query_by_label("Discard unsaved changes to DEFAULT?").is_none());
    assert_eq!(gain(&h), Some(5));
    // Nothing is left pending: asking again shows the prompt again.
    h.state_mut().request_open(path);
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
}

#[test]
fn failed_export_keeps_existing_file() {
    let d = tmp("save-keep");
    let out = d.join("out.syx");
    std::fs::write(&out, b"old").unwrap();
    std::fs::create_dir(d.join("out.syx.tmp")).unwrap(); // the temp file can't be written
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().export_to(out.clone());
    assert_eq!(std::fs::read(&out).unwrap(), b"old");
    assert!(h.state().error.as_deref().unwrap().starts_with("Couldn't save out.syx"));
}

#[test]
fn export_overwrites_and_leaves_no_temp_file() {
    let d = tmp("save-over");
    let out = d.join("out.syx");
    std::fs::write(&out, b"old").unwrap();
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().export_to(out.clone());
    let saved = Patch::from_syx(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(saved.get(find_param("gain").unwrap()), Some(7));
    let names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["out.syx"]);
}

#[cfg(unix)]
#[test]
fn export_through_symlink_writes_the_real_file() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("save-link");
    let real = d.join("real.syx");
    std::fs::write(&real, b"old").unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = d.join("link.syx");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().export_to(link.clone());
    assert!(h.state().error.is_none());
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "link was replaced");
    let saved = Patch::from_syx(&std::fs::read(&real).unwrap()).unwrap();
    assert_eq!(saved.get(find_param("gain").unwrap()), Some(7));
    assert_eq!(std::fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn pick_while_dirty_asks_and_cancel_creates_nothing() {
    let d = tmp("pick-dirty");
    let src = d.join("s.tsl");
    std::fs::write(&src, tsl("S", &[patch("Iron Man", 44)])).unwrap();
    let mut h = lib_harness(&d.join("lib"));
    let set = h.state_mut().library.import(&src).unwrap();
    set_gain(&mut h, 5);
    h.state_mut().request_pick(set.clone(), 0);
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(names_in(&d.join("lib/patches")).is_empty());
    assert_eq!(gain(&h), Some(5));

    h.state_mut().request_pick(set, 0);
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    let picked = d.join("lib/patches/Iron Man.syx");
    assert!(picked.exists());
    assert_eq!(h.state().editor.origin(), &Origin::Patch(picked));
    assert_eq!(h.state().editor.display_name(), "Iron Man");
    assert_eq!(gain(&h), Some(44));
    assert!(!h.state().editor.is_dirty());
}

#[test]
fn quick_save_from_default_creates_then_overwrites() {
    let d = tmp("quick-default");
    let mut h = lib_harness(&d);
    set_gain(&mut h, 8);
    h.state_mut().quick_save(now(1));
    let files = names_in(&d.join("quick-saves"));
    assert_eq!(files, ["quick-save_2026-10-28_15-04-01.syx"]);
    let qs = d.join("quick-saves").join(&files[0]);
    assert_eq!(h.state().editor.origin(), &Origin::QuickSave(qs.clone()));
    assert!(!h.state().editor.is_dirty());
    set_gain(&mut h, 9);
    h.state_mut().quick_save(now(2));
    assert_eq!(names_in(&d.join("quick-saves")).len(), 1);
    assert_eq!(gain_of(&h.state().library.load(&qs).unwrap()), Some(9));
}

#[test]
fn quick_save_from_named_patch_never_overwrites_it() {
    let d = tmp("quick-named");
    let a = lib_patch(&d, "A", 10);
    let before = std::fs::read(&a).unwrap();
    let mut h = lib_harness(&d);
    h.state_mut().request_open(a.clone());
    set_gain(&mut h, 11);
    h.state_mut().quick_save(now(3));
    assert_eq!(std::fs::read(&a).unwrap(), before);
    let files = names_in(&d.join("quick-saves"));
    assert_eq!(files.len(), 1);
    assert_eq!(gain_of(&h.state().library.load(&d.join("quick-saves").join(&files[0])).unwrap()), Some(11));
}

#[test]
fn quick_save_from_locked_quick_save_makes_a_new_one() {
    let d = tmp("quick-locked");
    let mut h = lib_harness(&d);
    h.state_mut().quick_save(now(4));
    let Origin::QuickSave(qs) = h.state().editor.origin().clone() else { panic!("not a quick save") };
    h.state_mut().library.set_read_only(&qs, true).unwrap();
    set_gain(&mut h, 3);
    h.state_mut().quick_save(now(5));
    assert_eq!(names_in(&d.join("quick-saves")).len(), 2);
    assert_eq!(gain_of(&h.state().library.load(&qs).unwrap()), Some(0));
}

#[test]
fn save_overwrites_origin_and_is_blocked_when_read_only() {
    let d = tmp("save-origin");
    let a = lib_patch(&d, "A", 1);
    let mut h = lib_harness(&d);
    h.state_mut().request_open(a.clone());
    set_gain(&mut h, 7);
    h.state_mut().save();
    assert_eq!(gain_of(&h.state().library.load(&a).unwrap()), Some(7));
    assert!(!h.state().editor.is_dirty());
    h.state_mut().library.set_read_only(&a, true).unwrap();
    set_gain(&mut h, 8);
    h.run();
    assert!(!h.state().can_save());
    assert!(h.get_by_label("Save").accesskit_node().is_disabled());
    assert!(!h.get_by_label("Save as…").accesskit_node().is_disabled());
}

#[test]
fn save_without_origin_opens_name_prompt() {
    let d = tmp("save-prompt");
    let mut h = lib_harness(&d);
    set_gain(&mut h, 6);
    h.get_by_label("Save").click();
    h.run();
    h.get_by_label("Name").set_text_value_accesskit("Blues");
    h.run();
    prompt_button(&h, "Save").click();
    h.run();
    let blues = d.join("patches/Blues.syx");
    assert!(blues.exists());
    assert_eq!(h.state().editor.origin(), &Origin::Patch(blues.clone()));
    assert_eq!(h.state().library.load(&blues).unwrap().name(), "Blues");
    assert!(h.query_by_label("Name").is_none());
    h.get_by_label("Blues");
    assert!(!h.state().editor.is_dirty());
}

#[test]
fn save_as_prompt_refuses_empty_name_and_cancels() {
    let d = tmp("save-as-cancel");
    let mut h = lib_harness(&d);
    h.get_by_label("Save as…").click();
    h.run();
    h.get_by_label("Name").set_text_value_accesskit("   ");
    h.run();
    assert!(prompt_button(&h, "Save").accesskit_node().is_disabled());
    prompt_button(&h, "Cancel").click();
    h.run();
    assert!(h.query_by_label("Name").is_none());
    assert!(names_in(&d.join("patches")).is_empty());
}

#[test]
fn import_adds_to_library_without_touching_editor() {
    let d = tmp("import");
    let src = d.join("s.tsl");
    std::fs::write(&src, tsl("Black Sabbath", &[patch("Iron Man", 1)])).unwrap();
    let mut h = lib_harness(&d.join("lib"));
    h.state_mut().import(src);
    h.run();
    assert_eq!(h.state().library.sets().len(), 1);
    assert_eq!(h.state().editor.display_name(), "DEFAULT");
    assert!(!h.state().editor.is_dirty());
    assert!(h.state().error.is_none());
}

#[test]
fn open_outside_library_has_no_origin() {
    let d = tmp("open-outside");
    let mut h = lib_harness(&d.join("lib"));
    h.state_mut().request_open(patch_file(&d));
    assert_eq!(h.state().editor.origin(), &Origin::None);
    assert_eq!(gain(&h), Some(60));
}
