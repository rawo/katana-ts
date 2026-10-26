use std::path::PathBuf;

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use katana_gui::app::KatanaApp;
use katana_proto::{find_param, Patch};

fn harness() -> Harness<'static, KatanaApp> {
    Harness::builder()
        .with_size([1600.0, 960.0])
        .build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), KatanaApp::new())
}

/// A fresh, empty directory for one test.
fn tmp(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("katana-gui-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
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
fn save_writes_and_clears_dirty() {
    let d = tmp("save");
    let out = d.join("out.syx");
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().save_to(out.clone());
    h.run();
    let saved = Patch::from_syx(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(saved.get(find_param("gain").unwrap()), Some(7));
    assert!(!h.state().editor.is_dirty());
    assert!(h.state().error.is_none());
}

#[test]
fn save_failure_keeps_dirty() {
    let d = tmp("save-fail");
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().save_to(d.join("missing-dir").join("out.syx"));
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
fn save_adds_missing_extension() {
    let d = tmp("save-ext");
    let mut h = harness();
    h.state_mut().save_to(d.join("mine"));
    assert!(d.join("mine.syx").exists());
    assert_eq!(h.state().editor.path(), Some(d.join("mine.syx").as_path()));
}
