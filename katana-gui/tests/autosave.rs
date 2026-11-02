mod common;

use std::path::{Path, PathBuf};

use common::{gain_of, names_in, patch, tmp};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use katana_gui::app::{KatanaApp, AUTOSAVE_SECS};
use katana_gui::editor::Origin;
use katana_gui::library::{Library, AUTOSAVE_KEEP};
use katana_proto::find_param;

fn at(min: u32, sec: u32) -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 10, 28).unwrap().and_hms_opt(15, min, sec).unwrap()
}

fn lib_patch(root: &Path, name: &str, gain: u32) -> PathBuf {
    let path = root.join("patches").join(format!("{name}.syx"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, patch(name, gain).to_syx()).unwrap();
    path
}

fn set_gain(app: &mut KatanaApp, v: u32) {
    app.editor.set(find_param("gain").unwrap(), v).unwrap();
}

fn gain(app: &KatanaApp) -> Option<u32> {
    gain_of(app.editor.patch())
}

fn autosaves(root: &Path) -> Vec<String> {
    names_in(&root.join("autosave")).into_iter().filter(|n| n.ends_with(".syx")).collect()
}

#[test]
fn autosave_keeps_last_ten() {
    let d = tmp("auto-ten");
    let mut lib = Library::open(d.clone());
    for i in 0..12 {
        lib.autosave(&patch("X", i), &Origin::None, true, None, at(0, i)).unwrap();
    }
    let files = autosaves(&d);
    assert_eq!(files.len(), AUTOSAVE_KEEP);
    assert_eq!(files[0], "autosave_2026-10-28_15-00-02.syx");
    let (p, origin, _, _) = lib.last_autosave().unwrap();
    assert_eq!(gain_of(&p), Some(11));
    assert_eq!(origin, Origin::None);
}

#[test]
fn autosaves_in_the_same_second_keep_their_order() {
    let d = tmp("auto-same");
    let mut lib = Library::open(d.clone());
    for i in 0..12 {
        lib.autosave(&patch("X", i), &Origin::None, true, None, at(0, 0)).unwrap();
    }
    assert_eq!(autosaves(&d).len(), AUTOSAVE_KEEP);
    assert_eq!(gain_of(&lib.last_autosave().unwrap().0), Some(11));
    // The two oldest are the ones dropped.
    assert!(!d.join("autosave/autosave_2026-10-28_15-00-00.syx").exists());
    assert!(!d.join("autosave/autosave_2026-10-28_15-00-00 (2).syx").exists());
    assert!(d.join("autosave/autosave_2026-10-28_15-00-00 (3).syx").exists());
}

#[test]
fn clock_going_back_keeps_the_newest_autosave() {
    // Ten auto-saves at 02:00, then the clock steps back an hour (DST ends).
    let d = tmp("auto-clock-back");
    let mut lib = Library::open(d.clone());
    let two = |s| chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap().and_hms_opt(2, 0, s).unwrap();
    for i in 0..10 {
        lib.autosave(&patch("X", i), &Origin::None, true, None, two(i)).unwrap();
    }
    lib.autosave(&patch("X", 99), &Origin::None, true, None, two(0) - chrono::Duration::hours(1)).unwrap();
    assert_eq!(autosaves(&d).len(), AUTOSAVE_KEEP);
    assert_eq!(gain_of(&lib.last_autosave().unwrap().0), Some(99));
    assert!(!d.join("autosave/autosave_2026-10-25_02-00-00.syx").exists(), "the oldest written goes");
}

#[test]
fn broken_autosave_state_is_ignored_and_named() {
    let d = tmp("auto-broken");
    let mut lib = Library::open(d.clone());
    assert!(lib.last_autosave().is_none());
    assert!(lib.problems.is_empty());
    std::fs::write(d.join("autosave/state.json"), "{").unwrap();
    assert!(lib.last_autosave().is_none());
    assert!(lib.problems.iter().any(|p| p.contains("auto-save")), "{:?}", lib.problems);
}

#[test]
fn tick_saves_only_after_interval_and_when_changed() {
    let d = tmp("auto-tick");
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.autosave_tick(0.0, at(0, 0));
    assert!(autosaves(&d).is_empty(), "nothing changed yet");
    set_gain(&mut app, 1);
    app.autosave_tick(0.0, at(0, 1));
    assert_eq!(autosaves(&d).len(), 1);
    set_gain(&mut app, 2);
    app.autosave_tick(100.0, at(1, 40));
    assert_eq!(autosaves(&d).len(), 1);
    app.autosave_tick(AUTOSAVE_SECS + 1.0, at(5, 1));
    assert_eq!(autosaves(&d).len(), 2);
    app.autosave_tick(700.0, at(11, 40));
    assert_eq!(autosaves(&d).len(), 2);
}

#[test]
fn restart_restores_state_and_origin() {
    let d = tmp("auto-restart");
    let a = lib_patch(&d, "A", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.request_open(a.clone());
    set_gain(&mut app, 33);
    app.autosave_tick(0.0, at(0, 0));
    drop(app); // the app is killed: no close, no prompt

    let app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(gain(&app), Some(33));
    assert_eq!(app.editor.origin(), &Origin::Patch(a));
    assert!(app.editor.is_dirty(), "A on disk still has gain 10");
    assert_eq!(app.editor.display_name(), "A");
}

#[test]
fn save_then_quit_restores_saved_not_older() {
    let d = tmp("auto-save-quit");
    let a = lib_patch(&d, "A", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.request_open(a.clone());
    set_gain(&mut app, 40);
    app.autosave_tick(0.0, at(0, 0));
    set_gain(&mut app, 41);
    app.save();
    assert!(app.on_close_requested(), "clean, so it closes");

    let mut app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(gain(&app), Some(41));
    assert!(!app.editor.is_dirty());
    app.save();
    assert_eq!(gain_of(&app.library.load(&a).unwrap()), Some(41));
}

#[test]
fn quick_save_and_save_as_also_autosave() {
    let d = tmp("auto-after-save");
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    set_gain(&mut app, 5);
    app.quick_save(at(0, 0));
    assert_eq!(autosaves(&d).len(), 1);
    set_gain(&mut app, 6);
    app.save_as("Blues");
    assert_eq!(autosaves(&d).len(), 2);
    let app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(app.editor.origin(), &Origin::Patch(d.join("patches/Blues.syx")));
    assert_eq!(gain(&app), Some(6));
    assert!(!app.editor.is_dirty());
}

#[test]
fn discard_at_close_restores_origin_content() {
    let d = tmp("auto-discard");
    let a = lib_patch(&d, "A", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.request_open(a.clone());
    set_gain(&mut app, 50);
    app.autosave_tick(0.0, at(0, 0)); // the unsaved 50 is auto-saved first
    let mut h =
        Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    assert!(!h.state_mut().on_close_requested());
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    drop(h);

    let app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(gain(&app), Some(10));
    assert_eq!(app.editor.origin(), &Origin::Patch(a));
    assert!(!app.editor.is_dirty());
}

#[test]
fn restore_with_missing_origin_falls_back() {
    let d = tmp("auto-missing");
    let a = lib_patch(&d, "A", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.request_open(a.clone());
    set_gain(&mut app, 20);
    app.autosave_tick(0.0, at(0, 0));
    std::fs::remove_file(&a).unwrap();

    let app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(gain(&app), Some(20));
    assert_eq!(app.editor.origin(), &Origin::None);
    assert!(app.editor.is_dirty());
    assert!(app.error.as_deref().unwrap_or_default().contains("A.syx"), "{:?}", app.error);
}

#[test]
fn start_without_autosave_is_the_default_preset() {
    let d = tmp("auto-none");
    let app = KatanaApp::start(Library::open(d));
    assert_eq!(app.editor.display_name(), "DEFAULT");
    assert!(!app.editor.is_dirty());
    assert!(app.error.is_none());
}

#[test]
fn status_line_shows_last_autosave_time() {
    let d = tmp("auto-status");
    let mut app = KatanaApp::with_library(Library::open(d));
    set_gain(&mut app, 1);
    app.autosave_tick(0.0, at(4, 12));
    let h = Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    h.get_by_label_contains("last auto-save 15:04");
}

#[test]
fn clean_quit_then_outside_change_opens_the_newer_file() {
    let d = tmp("auto-outside");
    let a = lib_patch(&d, "A", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.request_open(a.clone());
    set_gain(&mut app, 40);
    app.save();
    assert!(app.on_close_requested());
    // e.g. `katana-cli dump` into the library file while the app is closed
    std::fs::write(&a, patch("A", 77).to_syx()).unwrap();

    let mut app = KatanaApp::start(Library::open(d.clone()));
    assert_eq!(gain(&app), Some(77));
    assert!(!app.editor.is_dirty());
    app.save();
    assert_eq!(gain_of(&app.library.load(&a).unwrap()), Some(77));
}

#[test]
fn replaced_content_survives_restart() {
    let d = tmp("auto-replaced");
    let path = lib_patch(&d, "Riff", 10);
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.editor.load(patch("Riff", 10), Origin::Patch(path.clone()));
    app.editor.replace_from_amp(patch("From amp", 77));
    assert!(!app.editor.is_dirty());
    app.autosave_tick(0.0, at(0, 0));
    drop(app);

    let app = KatanaApp::start(Library::open(d));
    assert_eq!(app.editor.patch(), &patch("From amp", 77));
    assert_eq!(app.editor.path(), Some(path.as_path()));
}

#[test]
fn start_restores_channel_tab() {
    let d = tmp("auto-tab");
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    set_gain(&mut app, 5);
    app.channel_tab = Some(3);
    app.autosave_tick(0.0, at(0, 0));
    drop(app);
    assert_eq!(KatanaApp::start(Library::open(d)).channel_tab, Some(3));
}

#[test]
fn a_new_channel_tab_alone_is_auto_saved() {
    let d = tmp("auto-tab-only");
    let mut app = KatanaApp::with_library(Library::open(d.clone()));
    app.channel_tab = Some(6);
    app.autosave_tick(0.0, at(0, 0));
    drop(app);
    assert_eq!(KatanaApp::start(Library::open(d)).channel_tab, Some(6));
}
