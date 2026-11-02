//! The app against a scripted worker: events go in on `LinkEnd.events`, jobs
//! come out on `LinkEnd.jobs`.

mod common;

use std::path::{Path, PathBuf};

use common::{gain_of, names_in, patch, tmp};
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::amp::{AmpLink, Event, Job, LinkEnd};
use katana_gui::app::{KatanaApp, CHANNEL_NAMES};
use katana_gui::editor::Origin;
use katana_gui::library::Library;
use katana_proto::{encode_param, find_param, Channel};

const GAIN: &str = "gain";

struct Rig {
    h: Harness<'static, KatanaApp>,
    end: LinkEnd,
    root: PathBuf,
}

fn rig(test: &str) -> Rig {
    let root = tmp(&format!("amp-{test}"));
    let (link, end): (AmpLink, LinkEnd) = AmpLink::pair();
    let app = KatanaApp::with_library(Library::open(root.clone())).with_amp(link);
    let h = Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    Rig { h, end, root }
}

impl Rig {
    fn event(&mut self, e: Event) {
        self.end.events.send(e).unwrap();
        self.h.run();
    }

    fn connected(mut self) -> Rig {
        self.event(Event::Connected);
        self.event(Event::AmpOn(1));
        self
    }

    fn jobs(&self) -> Vec<Job> {
        self.end.jobs.try_iter().collect()
    }

    fn app(&mut self) -> &mut KatanaApp {
        self.h.state_mut()
    }

    fn click(&mut self, label: &str) {
        self.h.get_by_label(label).click();
        self.h.run();
    }

    /// `label` in an open combo box: the channel tabs have the same names and come first.
    fn click_in_popup(&mut self, label: &str) {
        self.h.get_all_by_label(label).last().unwrap().click();
        self.h.run();
    }

    fn live(mut self) -> Rig {
        self.click("Live edit");
        self.event(Event::Current(1, patch("On amp", 1)));
        self.jobs();
        self
    }

    fn disabled(&self, label: &str) -> bool {
        self.h.get_by_label(label).accesskit_node().is_disabled()
    }

    /// Whether `label`'s channel tab is the one shown.
    fn selected(&self, label: &str) -> bool {
        let tab = self.h.state().channel_tab.map(|s| CHANNEL_NAMES[usize::from(s)]);
        tab == Some(label)
    }

    /// Text of the status line.
    fn status(&self) -> String {
        self.h.get_by_label_contains("Amp: ").accesskit_node().value().unwrap_or_default().to_string()
    }
}

fn lib_file(root: &Path, name: &str, gain: u32) -> PathBuf {
    let path = root.join("patches").join(format!("{name}.syx"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, patch(name, gain).to_syx()).unwrap();
    path
}

fn gain(r: &mut Rig) -> Option<u32> {
    gain_of(r.app().editor.patch())
}

#[test]
fn connect_enables_amp_actions() {
    let mut r = rig("connect");
    for label in ["Load to amp", "A1", "PANEL", "Live edit"] {
        assert!(r.disabled(label), "{label}");
    }
    r.h.get_by_label("not connected");
    r = r.connected();
    for label in ["Load to amp", "A1", "PANEL", "B4", "Live edit"] {
        assert!(!r.disabled(label), "{label}");
    }
    r.h.get_by_label("connected");
    assert!(r.status().contains("Amp: connected, on A1"), "{}", r.status());
}

#[test]
fn live_on_sends_live_on_and_takes_amp_settings() {
    let mut r = rig("live-on").connected();
    r.app().edit(find_param(GAIN).unwrap(), 5).unwrap();
    r.click("Live edit");
    assert_eq!(r.jobs(), vec![Job::LiveOn]);
    assert_eq!(names_in(&r.root.join("quick-saves")).len(), 1, "unsaved edits are quick-saved first");

    let p = patch("On amp", 42);
    r.event(Event::Current(1, p.clone()));
    assert_eq!(r.app().editor.patch(), &p);
    assert!(!r.app().editor.is_dirty());
    assert!(r.selected("A1"));
    assert!(r.status().contains("Live edit on"), "{}", r.status());
}

#[test]
fn clean_editor_makes_no_quick_save_for_live() {
    let r = rig("live-clean").connected().live();
    assert!(names_in(&r.root.join("quick-saves")).is_empty());
}

#[test]
fn live_off_sends_live_off() {
    let mut r = rig("live-off").connected().live();
    r.click("Live edit");
    assert_eq!(r.jobs(), vec![Job::LiveOff]);
    assert!(!r.app().live);
    assert!(r.status().contains("Live edit off"), "{}", r.status());
}

#[test]
fn current_after_live_off_is_ignored() {
    let mut r = rig("late-current").connected().live();
    r.click("Live edit");
    r.event(Event::Current(1, patch("Late", 99)));
    assert_ne!(gain(&mut r), Some(99));
}

#[test]
fn live_edit_sends_set_param() {
    let mut r = rig("set-param").connected();
    let def = find_param(GAIN).unwrap();
    r.app().edit(def, 30).unwrap();
    assert_eq!(r.jobs(), vec![], "Live off sends nothing");

    let mut r = r.live();
    r.app().edit(def, 31).unwrap();
    assert_eq!(r.jobs(), vec![Job::SetParam { addr: def.addr, data: encode_param(def, 31).unwrap() }]);
}

#[test]
fn ui_edit_goes_to_the_amp_with_live_on() {
    let mut r = rig("ui-edit").connected().live();
    let def = find_param("reverb-sw").unwrap();
    r.click("REVERB OFF");
    assert_eq!(r.jobs(), vec![Job::SetParam { addr: def.addr, data: vec![1] }]);
}

#[test]
fn tab_click_live_off_reads_slot_only() {
    let mut r = rig("tab-off").connected();
    r.click("B1");
    assert_eq!(r.jobs(), vec![Job::ReadSlot(5)]);
    assert!(r.selected("B1"));

    let p = patch("Stored B1", 51);
    r.event(Event::Slot(5, p.clone()));
    assert_eq!(r.app().editor.patch(), &p);
    assert!(!r.app().editor.is_dirty());
    assert_eq!(r.app().library.channel_copy(5), Some(p));
    assert!(r.status().contains("on A1"), "the amp stays where it was: {}", r.status());
}

#[test]
fn tab_click_live_on_selects() {
    let mut r = rig("tab-on").connected().live();
    r.click("A3");
    assert_eq!(r.jobs(), vec![Job::Select(3)]);
    let p = patch("Third", 33);
    r.event(Event::Current(3, p.clone()));
    assert_eq!(r.app().editor.patch(), &p);
    assert!(r.selected("A3"));
    assert_eq!(r.app().library.channel_copy(3), Some(p));
}

#[test]
fn panel_tab_selects_with_live_on() {
    let mut r = rig("panel-tab").connected().live();
    r.click("PANEL");
    assert_eq!(r.jobs(), vec![Job::Select(0)]);
}

#[test]
fn tab_click_quick_saves_unsaved_edits() {
    let mut r = rig("tab-dirty").connected();
    r.app().edit(find_param(GAIN).unwrap(), 5).unwrap();
    r.click("A2");
    assert_eq!(names_in(&r.root.join("quick-saves")).len(), 1);
    assert!(r.selected("A2"));
}

#[test]
fn panel_tweak_is_quick_saved_before_the_next_replacement() {
    let mut r = rig("tweak-saved").connected().live();
    let def = find_param(GAIN).unwrap();
    r.event(Event::PanelChange { addr: def.addr, data: vec![66] });
    assert!(r.app().editor.is_dirty());
    r.event(Event::Current(2, patch("Next", 2)));
    assert_eq!(names_in(&r.root.join("quick-saves")).len(), 1);
    assert_eq!(gain(&mut r), Some(2));
}

#[test]
fn open_with_live_on_writes_patch_not_store() {
    let mut r = rig("open-live").connected().live();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().request_open(path);
    let jobs = r.jobs();
    assert_eq!(jobs, vec![Job::WritePatch(patch("Riff", 20))]);
}

#[test]
fn open_with_live_off_sends_nothing_and_clears_the_tab() {
    let mut r = rig("open-off").connected();
    r.click("A2");
    r.jobs();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().request_open(path);
    r.h.run();
    assert_eq!(r.jobs(), vec![]);
    assert_eq!(r.app().channel_tab, None);
}

#[test]
fn open_with_live_on_keeps_the_tab() {
    let mut r = rig("open-live-tab").connected().live();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().request_open(path);
    assert_eq!(r.app().channel_tab, Some(1));
}

#[test]
fn quick_save_and_save_as_clear_the_tab_with_live_off() {
    let now = chrono::Local::now().naive_local();
    let mut r = rig("save-tab").connected();
    r.app().channel_tab = Some(2);
    r.app().quick_save(now);
    assert_eq!(r.app().channel_tab, None);
    r.app().channel_tab = Some(2);
    r.app().save_as("Named");
    assert_eq!(r.app().channel_tab, None);
}

#[test]
fn load_to_amp_writes_patch() {
    let mut r = rig("load").connected();
    let p = r.app().editor.patch().clone();
    r.click("Load to amp");
    assert_eq!(r.jobs(), vec![Job::WritePatch(p)]);
    assert_eq!(r.app().amp_on, Some(1), "the amp's channel doesn't change");
}

#[test]
fn load_to_amp_waits_while_busy() {
    let mut r = rig("load-busy").connected();
    r.event(Event::Busy(Some("Reading channel\u{2026}".into())));
    assert!(r.disabled("Load to amp"));
    r.h.get_by_label("Reading channel\u{2026}");
    r.event(Event::Busy(None));
    assert!(!r.disabled("Load to amp"));
}

#[test]
fn panel_change_updates_editor_and_status() {
    let mut r = rig("panel-change").connected().live();
    let def = find_param(GAIN).unwrap();
    r.event(Event::PanelChange { addr: def.addr, data: vec![66] });
    assert_eq!(gain(&mut r), Some(66));
    assert!(r.app().editor.is_dirty());
    assert!(r.status().contains("panel change gain "), "{}", r.status());
    assert_eq!(r.jobs(), vec![], "a panel change is not sent back");
}

#[test]
fn panel_change_outside_known_params_shows_the_address() {
    let mut r = rig("panel-hex").connected().live();
    let addr = katana_proto::device::addr([0x20, 0x01, 0x1C, 0x00]);
    r.event(Event::PanelChange { addr, data: vec![7] });
    let (what, _) = r.app().last_panel_change.clone().unwrap();
    assert_eq!(what, format!("{}", katana_proto::Address(addr)));
}

#[test]
fn amp_switch_follows_with_live_on() {
    let mut r = rig("follow").connected().live();
    let p = patch("Sixth", 62);
    r.event(Event::Current(6, p.clone()));
    assert!(r.selected("B2"));
    assert_eq!(r.app().editor.patch(), &p);
    assert_eq!(r.app().amp_on, Some(6));
}

#[test]
fn amp_on_moves_the_tab_only_with_live_on() {
    let mut r = rig("amp-on").connected();
    r.event(Event::AmpOn(4));
    assert_eq!(r.app().amp_on, Some(4));
    assert_eq!(r.app().channel_tab, None);
    let mut r = r.live();
    r.event(Event::AmpOn(7));
    assert_eq!(r.app().channel_tab, Some(7));
}

#[test]
fn disconnect_disables_and_keeps_editor() {
    let mut r = rig("disconnect").connected().live();
    let before = r.app().editor.patch().clone();
    r.event(Event::Busy(Some("x".into())));
    r.event(Event::Disconnected);
    assert_eq!(r.app().editor.patch(), &before);
    for label in ["Load to amp", "Live edit", "A4"] {
        assert!(r.disabled(label), "{label}");
    }
    r.h.get_by_label("not connected");
    assert_eq!(r.app().busy, None);
    assert_eq!(r.app().amp_on, None);
    assert!(r.app().live, "Live keeps its position");
}

#[test]
fn edit_while_disconnected_sends_nothing() {
    let mut r = rig("edit-offline").connected().live();
    r.event(Event::Disconnected);
    r.app().edit(find_param(GAIN).unwrap(), 9).unwrap();
    assert_eq!(r.jobs(), vec![]);
}

#[test]
fn reconnect_with_live_on_syncs_again() {
    let mut r = rig("reconnect").connected().live();
    r.event(Event::Disconnected);
    r.app().edit(find_param(GAIN).unwrap(), 9).unwrap();
    r.jobs();
    r.event(Event::Connected);
    assert_eq!(r.jobs(), vec![Job::LiveOn]);
    assert_eq!(names_in(&r.root.join("quick-saves")).len(), 1);
}

#[test]
fn reconnect_with_live_off_stays_off() {
    let mut r = rig("reconnect-off").connected();
    r.event(Event::Disconnected);
    r.event(Event::Connected);
    assert_eq!(r.jobs(), vec![]);
}

#[test]
fn offline_tab_shows_channel_copy() {
    let mut r = rig("offline-tab");
    let p = patch("Copy", 12);
    r.app().library.save_channel_copy(2, &p).unwrap();
    r.h.run();
    assert!(!r.disabled("A2"));
    assert!(r.disabled("A3"));
    r.click("A2");
    assert_eq!(r.app().editor.patch(), &p);
    assert!(r.selected("A2"));
    assert_eq!(r.jobs(), vec![]);
}

#[test]
fn replaced_note_shown() {
    let mut r = rig("note").connected();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().editor.load(patch("Riff", 20), Origin::Patch(path));
    r.click("B1");
    r.event(Event::Slot(5, patch("Stored", 5)));
    r.h.get_by_label("Saving will overwrite library preset Riff.syx");
}

#[test]
fn failed_job_shows_in_the_status_line() {
    let mut r = rig("failed").connected();
    r.event(Event::Failed("no reply".into()));
    assert_eq!(r.app().error.as_deref(), Some("no reply"));
}

#[test]
fn no_link_means_not_connected() {
    let app = KatanaApp::new();
    assert!(!app.connected);
}

fn lib_snapshot(root: &Path) -> Vec<Vec<String>> {
    ["patches", "quick-saves", "sets"].iter().map(|d| names_in(&root.join(d))).collect()
}

/// Opens the dialog on A3 and answers its peek.
fn store_asking(test: &str) -> Rig {
    let mut r = rig(test).connected();
    r.app().editor.load(patch("New sound", 7), Origin::None);
    r.app().channel_tab = Some(3);
    r.h.run();
    r.click("Store to channel…");
    assert_eq!(r.jobs(), vec![Job::Peek(Channel::A3)]);
    r.event(Event::Peeked(Channel::A3, patch("Old sound", 1)));
    r
}

#[test]
fn store_asks_with_old_and_new_names() {
    let r = store_asking("store-asks");
    r.h.get_by_label("A3 currently 'Old sound' -> replace with 'New sound'?");
    r.h.get_by_label("Store");
    r.h.get_by_label("Cancel");
}

#[test]
fn store_shows_reading_until_the_peek_answers() {
    let mut r = rig("store-reading").connected();
    r.app().channel_tab = Some(3);
    r.h.run();
    r.click("Store to channel…");
    r.h.get_by_label("Reading A3\u{2026}");
    assert!(r.h.query_by_label("Store").is_none());
}

#[test]
fn store_without_channel_tab_asks_for_one() {
    let mut r = rig("store-pick").connected();
    r.app().editor.load(patch("New sound", 7), Origin::None);
    r.h.run();
    r.click("Store to channel…");
    assert_eq!(r.jobs(), vec![], "no channel chosen yet");
    r.click("Channel");
    r.click_in_popup("B2");
    assert_eq!(r.jobs(), vec![Job::Peek(Channel::B2)]);
    r.event(Event::Peeked(Channel::B2, patch("Old", 1)));
    r.h.get_by_label("B2 currently 'Old' -> replace with 'New sound'?");
}

#[test]
fn panel_tab_preselects_no_channel() {
    let mut r = rig("store-panel").connected();
    r.app().channel_tab = Some(0);
    r.h.run();
    r.click("Store to channel…");
    assert_eq!(r.jobs(), vec![], "PANEL is not a store target");
}

#[test]
fn store_cancel_sends_nothing() {
    let mut r = store_asking("store-cancel");
    r.click("Cancel");
    assert_eq!(r.jobs(), vec![]);
    assert!(r.h.query_by_label("Cancel").is_none());
    assert!(r.app().channel_tab == Some(3));
}

#[test]
fn store_escape_is_cancel() {
    let mut r = store_asking("store-escape");
    r.h.key_press(egui::Key::Escape);
    r.h.run();
    assert_eq!(r.jobs(), vec![]);
    assert!(r.h.query_by_label("Cancel").is_none());
}

#[test]
fn store_sends_store_with_backup_dir() {
    let mut r = store_asking("store-sends");
    r.click("Store");
    let jobs = r.jobs();
    let [Job::Store { ch, patch: p, backup_dir, stamp }] = jobs.as_slice() else { panic!("{jobs:?}") };
    assert_eq!(*ch, Channel::A3);
    assert_eq!(p, &patch("New sound", 7));
    assert_eq!(backup_dir, &r.root.join("backups"));
    assert!(chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d_%H-%M-%S").is_ok(), "{stamp}");
    assert!(r.h.query_by_label("Cancel").is_none(), "the dialog closes");
}

fn stored(solo_delay_stored: bool) -> Event {
    Event::Stored {
        ch: Channel::A4,
        channel_backup: "/b/channel.syx".into(),
        current_backup: "/b/current.syx".into(),
        solo_delay_stored,
    }
}

#[test]
fn stored_moves_tab_and_reports_solo_delay() {
    let mut r = store_asking("store-done");
    r.click("Store");
    r.event(stored(false));
    assert_eq!(r.app().channel_tab, Some(4));
    assert_eq!(r.app().amp_on, Some(4));
    r.h.get_by_label("stored 'New sound' in A4; the amp is now on A4 · solo delay not stored");
}

#[test]
fn stored_with_solo_delay_says_nothing_about_it() {
    let mut r = store_asking("store-solo");
    r.click("Store");
    r.event(stored(true));
    r.h.get_by_label("stored 'New sound' in A4; the amp is now on A4");
}

#[test]
fn store_failure_shows_message() {
    let mut r = store_asking("store-fail");
    r.click("Store");
    r.event(Event::StoreFailed("Storing into A3 failed: x".into()));
    assert_eq!(r.app().error.as_deref(), Some("Storing into A3 failed: x"));
}

#[test]
fn store_does_not_save_file() {
    let mut r = store_asking("store-no-file");
    r.app().edit(find_param(GAIN).unwrap(), 9).unwrap();
    r.jobs();
    let before = lib_snapshot(&r.root);
    r.click("Store");
    r.event(stored(false));
    assert_eq!(lib_snapshot(&r.root), before);
    assert!(r.app().editor.is_dirty(), "a store is not a save");
    assert_eq!(gain(&mut r), Some(9));
}

#[test]
fn no_store_without_a_click() {
    let mut r = rig("no-store").connected().live();
    r.app().edit(find_param(GAIN).unwrap(), 9).unwrap();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().request_open(path);
    r.app().quick_save(chrono::Local::now().naive_local());
    r.h.run();
    assert!(!r.jobs().iter().any(|j| matches!(j, Job::Store { .. })));
}

#[test]
fn failed_peek_lets_the_user_choose_again() {
    let mut r = rig("store-peek-fail").connected();
    r.app().channel_tab = Some(3);
    r.h.run();
    r.click("Store to channel…");
    r.event(Event::Failed("no reply".into()));
    assert!(r.h.query_by_label("Reading A3\u{2026}").is_none());
    r.h.get_by_label("Cancel");
    r.jobs();
    r.click("Channel");
    r.click_in_popup("A3");
    assert_eq!(r.jobs(), vec![Job::Peek(Channel::A3)]);
}

#[test]
fn disconnect_closes_the_store_dialog() {
    let mut r = store_asking("store-unplug");
    r.event(Event::Disconnected);
    assert!(r.h.query_by_label("Cancel").is_none());
    assert_eq!(r.jobs(), vec![]);
}

#[test]
fn store_disabled_when_busy() {
    let mut r = rig("store-busy").connected();
    assert!(!r.disabled("Store to channel…"));
    r.event(Event::Busy(Some("x".into())));
    assert!(r.disabled("Store to channel…"));
}

#[test]
fn store_disabled_without_library() {
    let (link, end) = AmpLink::pair();
    let app = KatanaApp::new().with_amp(link);
    let mut h =
        Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    end.events.send(Event::Connected).unwrap();
    h.run();
    assert!(h.get_by_label("Store to channel…").accesskit_node().is_disabled());
    assert!(!h.get_by_label("Load to amp").accesskit_node().is_disabled());
}

#[test]
fn quit_signal_closes_without_prompt() {
    let root = tmp("quit-signal");
    let mut app = KatanaApp::with_library(Library::open(root.clone()));
    app.editor.set(find_param(GAIN).unwrap(), 5).unwrap();
    let flag = app.quit_signal.clone();
    let mut h =
        Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    h.run();
    let saved = names_in(&root.join("autosave")).iter().filter(|n| n.ends_with(".syx")).count();
    h.state_mut().editor.set(find_param(GAIN).unwrap(), 6).unwrap();

    flag.store(true, std::sync::atomic::Ordering::SeqCst);
    h.step();
    let close = h.output().viewport_output.values().any(|v| v.commands.contains(&egui::ViewportCommand::Close));
    assert!(close, "the window closes");
    assert!(h.query_by_label("Discard").is_none());
    let now = names_in(&root.join("autosave")).iter().filter(|n| n.ends_with(".syx")).count();
    assert_eq!(now, saved + 1, "the unsaved edit is in a new auto-save");
    let restored = KatanaApp::start(Library::open(root));
    assert_eq!(gain_of(restored.editor.patch()), Some(6));
}

#[test]
fn no_quit_without_the_signal() {
    let root = tmp("quit-none");
    let app = KatanaApp::with_library(Library::open(root));
    let mut h =
        Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    h.step();
    let close = h.output().viewport_output.values().any(|v| v.commands.contains(&egui::ViewportCommand::Close));
    assert!(!close);
}

#[test]
fn closing_turns_the_link_off() {
    let (link, end) = AmpLink::pair();
    let mut app = KatanaApp::new().with_amp(link);
    app.shutdown_amp();
    assert_eq!(end.jobs.try_iter().collect::<Vec<_>>(), vec![Job::Shutdown]);
}
