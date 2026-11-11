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
use katana_proto::{encode_param, find_param, Channel, Patch};

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
    let def = find_param("delay-sw").unwrap();
    r.click("DELAY OFF");
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
    r.event(Event::Slot(2, patch("Second", 2)));
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
fn delay_colour_goes_to_the_amp_with_live_on() {
    let mut r = rig("delay-colour-out").connected().live();
    let def = find_param("delay-color").unwrap();
    r.click("DELAY ORANGE");
    assert_eq!(r.jobs(), vec![Job::SetParam { addr: def.addr, data: vec![2] }]);
}

#[test]
fn delay_colour_and_tap_from_the_panel_follow() {
    let mut r = rig("delay-colour-in").connected().live();
    let color = find_param("delay-color").unwrap();
    r.event(Event::PanelChange { addr: color.addr, data: vec![1] });
    let time = find_param("delay2-time").unwrap();
    r.event(Event::PanelChange { addr: time.addr, data: encode_param(time, 526).unwrap() });
    assert_eq!(r.app().editor.get(color), Some(1));
    assert_eq!(r.app().editor.get(time), Some(526));
    r.click("DELAY");
    r.h.get_by_label_contains("DELAY · RED variation");
    assert_eq!(r.jobs(), vec![], "panel changes are not sent back");
}

#[test]
fn reverb_colour_both_ways_with_live_on() {
    let mut r = rig("reverb-colour").connected().live();
    let color = find_param("reverb-color").unwrap();
    r.click("DELAY 2 / REVERB RED");
    assert_eq!(r.jobs(), vec![Job::SetParam { addr: color.addr, data: vec![1] }]);
    r.event(Event::PanelChange { addr: color.addr, data: vec![0] });
    assert_eq!(r.app().editor.get(color), Some(0));
    r.click("DELAY 2 / REVERB");
    r.h.get_by_label_contains("DELAY 2 / REVERB · GREEN variation");
    assert_eq!(r.jobs(), vec![], "panel changes are not sent back");
}

#[test]
fn mod_colour_goes_to_the_amp_with_live_on() {
    let mut r = rig("mod-colour-out").connected().live();
    let def = find_param("mod-color").unwrap();
    r.click("MOD RED");
    assert_eq!(r.jobs(), vec![Job::SetParam { addr: def.addr, data: vec![1] }]);
}

#[test]
fn fx_colour_type_and_knob_setting_from_the_panel_follow() {
    let mut r = rig("fx-colour-in").connected().live();
    let color = find_param("fx-color").unwrap();
    let kind = find_param("fx2-type").unwrap();
    let rate = find_param("fx2-tremolo-rate").unwrap();
    r.event(Event::PanelChange { addr: kind.addr, data: vec![16] });
    r.event(Event::PanelChange { addr: color.addr, data: vec![1] });
    r.event(Event::PanelChange { addr: rate.addr, data: vec![77] });
    assert_eq!(r.app().editor.get(color), Some(1));
    assert_eq!(r.app().editor.get(rate), Some(77));
    r.click("FX");
    r.h.get_by_label("FX · RED variation · type TREMOLO");
    assert_eq!(r.h.get_by_label("RATE").accesskit_node().numeric_value(), Some(77.0));
    assert_eq!(r.jobs(), vec![], "panel changes are not sent back");
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

// --- Automatic quick saves (final review C1, I8, I9) ---

fn quick_save_gains(root: &Path) -> Vec<u32> {
    let dir = root.join("quick-saves");
    let mut gains: Vec<u32> = names_in(&dir)
        .iter()
        .map(|n| gain_of(&Patch::from_syx(&std::fs::read(dir.join(n)).unwrap()).unwrap()).unwrap())
        .collect();
    gains.sort();
    gains
}

fn tweak(r: &mut Rig, value: u8) {
    let def = find_param(GAIN).unwrap();
    r.event(Event::PanelChange { addr: def.addr, data: vec![value] });
}

#[test]
fn automatic_quick_saves_never_overwrite_each_other() {
    let mut r = rig("rescue-two").connected().live();
    tweak(&mut r, 11);
    r.event(Event::Current(2, patch("Second", 2)));
    tweak(&mut r, 21);
    r.event(Event::Current(3, patch("Third", 3)));
    assert_eq!(quick_save_gains(&r.root), vec![11, 21]);
}

#[test]
fn automatic_quick_save_leaves_an_opened_quick_save_alone() {
    let mut r = rig("rescue-opened").connected().live();
    let path = r.root.join("quick-saves/mine.syx");
    std::fs::write(&path, patch("Mine", 55).to_syx()).unwrap();
    r.app().editor.load(patch("Mine", 55), Origin::QuickSave(path.clone()));
    tweak(&mut r, 11);
    r.event(Event::Current(2, patch("Second", 2)));
    assert_eq!(gain_of(&Patch::from_syx(&std::fs::read(&path).unwrap()).unwrap()), Some(55));
    assert_eq!(quick_save_gains(&r.root), vec![11, 55]);
}

#[test]
fn automatic_quick_save_keeps_the_origin() {
    let mut r = rig("rescue-origin").connected().live();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().editor.load(patch("Riff", 20), Origin::Patch(path.clone()));
    tweak(&mut r, 11);
    r.event(Event::Current(2, patch("Second", 2)));
    assert_eq!(r.app().editor.path(), Some(path.as_path()));
    r.h.get_by_label("Saving will overwrite library preset Riff.syx");
}

#[test]
fn unsaved_edits_are_kept_once_per_replacement() {
    let mut r = rig("rescue-once").connected();
    r.app().edit(find_param(GAIN).unwrap(), 5).unwrap();
    r.click("A2");
    r.event(Event::Slot(2, patch("Second", 2)));
    assert_eq!(quick_save_gains(&r.root), vec![5]);
}

#[test]
fn quick_save_button_makes_a_new_file_while_replaced() {
    let mut r = rig("button-replaced").connected();
    let path = r.root.join("quick-saves/mine.syx");
    std::fs::write(&path, patch("Mine", 55).to_syx()).unwrap();
    r.app().editor.load(patch("Mine", 55), Origin::QuickSave(path.clone()));
    r.click("A2");
    r.event(Event::Slot(2, patch("Second", 2)));
    r.app().quick_save(chrono::Local::now().naive_local());
    assert_eq!(gain_of(&Patch::from_syx(&std::fs::read(&path).unwrap()).unwrap()), Some(55));
    assert_eq!(quick_save_gains(&r.root), vec![2, 55]);
}

#[test]
fn quick_save_button_still_updates_its_own_quick_save() {
    let mut r = rig("button-own").connected();
    let path = r.root.join("quick-saves/mine.syx");
    std::fs::write(&path, patch("Mine", 55).to_syx()).unwrap();
    r.app().editor.load(patch("Mine", 55), Origin::QuickSave(path.clone()));
    r.app().edit(find_param(GAIN).unwrap(), 56).unwrap();
    r.app().quick_save(chrono::Local::now().naive_local());
    assert_eq!(quick_save_gains(&r.root), vec![56]);
}

/// Makes every new quick save fail.
fn break_quick_saves(root: &Path) {
    let dir = root.join("quick-saves");
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(&dir, b"a file, not a folder").unwrap();
}

#[test]
fn failed_rescue_keeps_the_edits_and_pauses_live() {
    let mut r = rig("rescue-fails").connected().live();
    tweak(&mut r, 11);
    break_quick_saves(&r.root);
    r.event(Event::Current(2, patch("Second", 2)));
    assert_eq!(gain(&mut r), Some(11), "the edits stay in the editor");
    assert!(!r.app().live);
    assert_eq!(r.jobs(), vec![Job::LiveOff]);
    assert!(r.app().error.as_deref().unwrap_or_default().contains("unsaved"), "{:?}", r.app().error);
}

#[test]
fn failed_rescue_stops_a_tab_click() {
    let mut r = rig("rescue-tab").connected();
    r.app().edit(find_param(GAIN).unwrap(), 5).unwrap();
    break_quick_saves(&r.root);
    r.click("A2");
    assert_eq!(r.jobs(), vec![]);
    assert_eq!(r.app().channel_tab, None);
    assert_eq!(gain(&mut r), Some(5));
}

#[test]
fn failed_rescue_stops_live_on() {
    let mut r = rig("rescue-live").connected();
    r.app().edit(find_param(GAIN).unwrap(), 5).unwrap();
    break_quick_saves(&r.root);
    r.click("Live edit");
    assert_eq!(r.jobs(), vec![]);
    assert!(!r.app().live);
}

fn at(sec: u32) -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap().and_hms_opt(20, 0, sec).unwrap()
}

#[test]
fn restart_keeps_the_note_and_the_clean_state() {
    let mut r = rig("restart-note").connected();
    let path = lib_file(&r.root.clone(), "Riff", 20);
    r.app().editor.load(patch("Riff", 20), Origin::Patch(path.clone()));
    r.click("A2");
    let stored = patch("Second", 2);
    r.event(Event::Slot(2, stored.clone()));
    r.app().autosave_tick(1000.0, at(0));
    let root = r.root.clone();

    let mut app = KatanaApp::start(Library::open(root));
    assert_eq!(app.editor.patch(), &stored);
    assert!(!app.editor.is_dirty(), "browsing a channel is not an edit");
    assert_eq!(app.editor.replaced_note().as_deref(), Some("Saving will overwrite library preset Riff.syx"));
    assert_eq!(app.editor.path(), Some(path.as_path()));
    app.edit(find_param(GAIN).unwrap(), 3).unwrap();
    assert!(app.editor.is_dirty());
}

#[test]
fn restart_keeps_edits_made_after_a_replacement() {
    let mut r = rig("restart-edit").connected();
    r.click("A2");
    let stored = patch("Second", 2);
    r.event(Event::Slot(2, stored.clone()));
    r.app().edit(find_param(GAIN).unwrap(), 9).unwrap();
    r.app().autosave_tick(1000.0, at(0));
    let root = r.root.clone();

    let app = KatanaApp::start(Library::open(root));
    assert_eq!(gain_of(app.editor.patch()), Some(9));
    assert_eq!(app.editor.baseline(), &stored);
    assert!(app.editor.is_dirty());
}

#[test]
fn restart_without_a_quick_save_of_a_restored_quick_save() {
    let mut r = rig("restart-qs").connected();
    let path = r.root.join("quick-saves/mine.syx");
    std::fs::write(&path, patch("Mine", 55).to_syx()).unwrap();
    r.app().editor.load(patch("Mine", 55), Origin::QuickSave(path.clone()));
    r.click("A2");
    r.event(Event::Slot(2, patch("Second", 2)));
    r.app().autosave_tick(1000.0, at(0));
    let root = r.root.clone();

    let mut app = KatanaApp::start(Library::open(root));
    app.quick_save(at(1));
    assert_eq!(gain_of(&Patch::from_syx(&std::fs::read(&path).unwrap()).unwrap()), Some(55));
}

// --- Nothing conflicting while the amp is working (final review I1, I3, I4, I11, I13) ---

#[test]
fn nothing_conflicting_while_a_read_runs() {
    let mut r = rig("lock-read").connected();
    r.click("A3");
    r.jobs();
    for label in ["A1", "B4", "PANEL", "Live edit", "Load to amp", "Store to channel…"] {
        assert!(r.disabled(label), "{label}");
    }
    r.event(Event::Slot(3, patch("Third", 3)));
    for label in ["A1", "Live edit", "Load to amp", "Store to channel…"] {
        assert!(!r.disabled(label), "{label}");
    }
}

#[test]
fn locked_from_the_click_not_from_the_workers_answer() {
    let mut r = rig("lock-click").connected();
    r.click("A3");
    assert!(r.app().busy.is_none(), "the worker hasn't answered");
    assert!(r.disabled("A4"));
}

#[test]
fn open_during_a_read_is_refused_and_the_read_lands_cleanly() {
    let mut r = rig("lock-open").connected();
    let path = lib_file(&r.root.clone(), "Riff", 77);
    r.click("A3");
    r.jobs();
    r.app().request_open(path.clone());
    assert_ne!(r.app().editor.path(), Some(path.as_path()));
    assert!(r.app().error.is_some());
    r.event(Event::Slot(3, patch("Third", 33)));
    assert_eq!(gain(&mut r), Some(33));
    assert_eq!(r.app().editor.path(), None, "the channel's content is not labelled Riff");
    assert_eq!(gain_of(&Patch::from_syx(&std::fs::read(&path).unwrap()).unwrap()), Some(77));
}

#[test]
fn no_edit_while_live_is_syncing() {
    let mut r = rig("lock-edit").connected();
    r.click("Live edit");
    r.jobs();
    r.app().edit(find_param(GAIN).unwrap(), 90).unwrap();
    assert_eq!(r.jobs(), vec![], "the amp gets nothing the editor is about to lose");
    r.event(Event::Current(1, patch("On amp", 10)));
    assert_eq!(gain(&mut r), Some(10));
    r.app().edit(find_param(GAIN).unwrap(), 90).unwrap();
    assert_eq!(r.jobs().len(), 1);
}

#[test]
fn no_edit_during_a_read_the_amp_started() {
    let mut r = rig("lock-panel").connected().live();
    r.event(Event::Busy(Some("Reading channel\u{2026}".into())));
    r.app().edit(find_param(GAIN).unwrap(), 90).unwrap();
    assert_eq!(r.jobs(), vec![]);
    r.event(Event::Current(4, patch("Fourth", 4)));
    r.event(Event::Busy(None));
    assert_eq!(gain(&mut r), Some(4));
}

#[test]
fn controls_are_disabled_during_a_read() {
    let mut r = rig("lock-ui").connected().live();
    r.event(Event::Busy(Some("x".into())));
    assert!(r.disabled("DELAY OFF"));
    r.event(Event::Busy(None));
    assert!(!r.disabled("DELAY OFF"));
}

#[test]
fn failed_job_unlocks() {
    let mut r = rig("lock-failed").connected();
    r.click("A3");
    r.event(Event::Failed("no reply".into()));
    assert!(!r.disabled("A4"));
}

#[test]
fn disconnect_unlocks() {
    let mut r = rig("lock-unplug").connected();
    r.app().library.save_channel_copy(4, &patch("Copy", 4)).unwrap();
    r.click("A3");
    r.event(Event::Disconnected);
    assert!(!r.disabled("A4"));
}

#[test]
fn failed_live_on_turns_the_checkbox_off() {
    let mut r = rig("live-failed").connected();
    r.click("Live edit");
    r.event(Event::LiveFailed("Live edit couldn't start: no reply".into()));
    assert!(!r.app().live);
    assert!(r.status().contains("Live edit off"), "{}", r.status());
    assert_eq!(r.app().error.as_deref(), Some("Live edit couldn't start: no reply"));
    assert!(!r.disabled("Live edit"));
}

#[test]
fn store_stores_what_the_question_showed() {
    let mut r = store_asking("store-frozen");
    r.end.events.send(Event::Busy(None)).unwrap();
    r.app().editor.replace_from_amp(patch("Something else", 99));
    r.h.run();
    r.h.get_by_label("A3 currently 'Old sound' -> replace with 'New sound'?");
    r.click("Store");
    let jobs = r.jobs();
    let [Job::Store { patch: p, .. }] = jobs.as_slice() else { panic!("{jobs:?}") };
    assert_eq!(p, &patch("New sound", 7));
    r.event(stored(false));
    r.h.get_by_label("stored 'New sound' in A4; the amp is now on A4 · solo delay not stored");
}

#[test]
fn close_waits_for_a_running_store() {
    let mut r = store_asking("close-store");
    r.click("Store");
    assert!(!r.app().on_close_requested());
    assert!(r.h.query_by_label("Discard").is_none());
    assert!(r.app().error.as_deref().unwrap_or_default().contains("Storing"), "{:?}", r.app().error);
    r.event(stored(false));
    assert!(r.app().on_close_requested());
}

#[test]
fn quit_signal_waits_for_a_running_store() {
    let mut r = store_asking("quit-store");
    r.click("Store");
    r.app().quit_signal.store(true, std::sync::atomic::Ordering::SeqCst);
    r.h.step();
    let closes = |h: &Harness<KatanaApp>| {
        h.output().viewport_output.values().any(|v| v.commands.contains(&egui::ViewportCommand::Close))
    };
    assert!(!closes(&r.h));
    r.end.events.send(stored(false)).unwrap();
    r.h.step();
    assert!(closes(&r.h));
}

// --- The app with the real worker and the simulated amp (final review I14) ---

mod with_worker {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use katana_gui::amp::Worker;
    use katana_proto::device::{self, PATCH_BLOCKS};
    use katana_proto::patch::slot_offset;
    use katana_proto::{read_patch, read_slot, Address, FakeAmp, ProtoError, Transport};

    use super::*;

    const T: Duration = Duration::from_millis(200);
    const GAIN_ADDR: u32 = device::addr([0x20, 0x00, 0x06, 0x00]);

    struct SharedFake(Arc<Mutex<FakeAmp>>);

    impl Transport for SharedFake {
        fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError> {
            self.0.lock().unwrap().send(msg)
        }

        fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError> {
            self.0.lock().unwrap().recv(timeout)
        }
    }

    struct Bench {
        h: Harness<'static, KatanaApp>,
        fake: Arc<Mutex<FakeAmp>>,
        port: Arc<AtomicBool>,
        root: PathBuf,
    }

    fn bench(test: &str) -> Bench {
        let root = tmp(&format!("bench-{test}"));
        let mut amp = FakeAmp::new(device::header(), 128);
        for &(a, s) in PATCH_BLOCKS {
            amp.mem.apply(Address(a), &vec![0x40; s as usize]);
            for n in 1..=9u8 {
                amp.mem.apply(Address(a + slot_offset(n)), &vec![n; s as usize]);
            }
        }
        amp.mem.apply(Address(device::SELECTOR), &[0, 4]);
        let fake = Arc::new(Mutex::new(amp));
        let port = Arc::new(AtomicBool::new(true));
        let (f, p) = (fake.clone(), port.clone());
        let worker = Worker::new(
            Box::new(move || Ok(Box::new(SharedFake(f.clone())) as Box<dyn Transport + Send>)),
            Box::new(move || p.load(Ordering::SeqCst)),
        );
        let link = AmpLink::spawn(worker, Box::new(|| {}));
        let app = KatanaApp::with_library(Library::open(root.clone())).with_amp(link);
        let h =
            Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
        let mut b = Bench { h, fake, port, root };
        b.wait("connected", |app| app.connected && app.amp_idle());
        b
    }

    impl Bench {
        fn wait(&mut self, what: &str, done: impl Fn(&KatanaApp) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !done(self.h.state()) {
                assert!(Instant::now() < deadline, "waited too long for: {what}");
                std::thread::sleep(Duration::from_millis(5));
                self.h.step();
            }
            self.h.step();
        }

        fn click(&mut self, label: &str) {
            self.h.get_by_label(label).click();
            self.h.step();
            self.h.step();
        }

        fn live(mut self) -> Bench {
            self.click("Live edit");
            self.wait("Live", |app| app.amp_on.is_some() && app.channel_tab.is_some() && app.amp_idle());
            self
        }

        fn amp_gain(&self) -> Option<u8> {
            self.fake.lock().unwrap().mem.read(Address(GAIN_ADDR), 1).map(|b| b[0])
        }

        fn current(&self) -> Patch {
            read_patch(&mut *self.fake.lock().unwrap(), T).unwrap()
        }

        fn slot(&self, n: u8) -> Patch {
            read_slot(&mut *self.fake.lock().unwrap(), n, T).unwrap()
        }
    }

    #[test]
    fn live_shows_the_amps_settings() {
        let b = bench("live").live();
        let expected = b.current();
        assert_eq!(b.h.state().editor.patch(), &expected);
        assert_eq!(b.h.state().channel_tab, Some(4));
        assert!(b.fake.lock().unwrap().editor);
    }

    #[test]
    fn app_edit_reaches_the_amp_and_is_not_reported_back() {
        let mut b = bench("edit").live();
        b.h.state_mut().edit(find_param(GAIN).unwrap(), 71).unwrap();
        let fake = b.fake.clone();
        let deadline = Instant::now() + Duration::from_secs(5);
        while fake.lock().unwrap().mem.read(Address(GAIN_ADDR), 1) != Some(vec![71]) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(100));
        b.h.step();
        assert_eq!(b.amp_gain(), Some(71));
        assert_eq!(b.h.state().last_panel_change, None);
    }

    #[test]
    fn panel_knob_reaches_the_editor() {
        let mut b = bench("panel").live();
        b.fake.lock().unwrap().turn_panel(GAIN_ADDR, &[66]);
        b.wait("panel change", |app| gain_of(app.editor.patch()) == Some(66));
        assert!(b.h.state().editor.is_dirty());
        assert!(b.h.state().last_panel_change.is_some());
    }

    #[test]
    fn panel_channel_presses_end_on_the_last_one() {
        let mut b = bench("channels").live();
        let expected = b.slot(8);
        {
            let mut amp = b.fake.lock().unwrap();
            amp.press_channel(6);
            amp.at_rq1(25, |a| a.press_channel(7));
        }
        b.wait("the last channel", |app| app.channel_tab == Some(7) && app.amp_idle());
        assert_eq!(b.h.state().editor.patch(), &expected);
        assert!(!b.h.state().editor.is_dirty());
        assert_eq!(b.h.state().library.channel_copy(7), Some(expected));
        assert_eq!(b.h.state().library.channel_copy(6), None, "no mixed copy under the first channel");
    }

    #[test]
    fn tab_with_live_off_leaves_the_amp_alone() {
        let b = bench("tab").connected_tab();
        assert_eq!(b.h.state().editor.patch(), &b.slot(3));
        assert!(!b.fake.lock().unwrap().editor);
        assert_eq!(b.h.state().amp_on, Some(4));
    }

    impl Bench {
        fn connected_tab(mut self) -> Bench {
            self.click("A2");
            self.wait("the channel", |app| app.editor.patch().name() != "DEFAULT" && app.amp_idle());
            self
        }
    }

    #[test]
    fn store_end_to_end() {
        let mut b = bench("store");
        b.h.state_mut().editor.load(patch("New sound", 7), Origin::None);
        b.h.state_mut().channel_tab = Some(7);
        b.h.step();
        let old_b3 = b.slot(Channel::B3.slot());
        b.click("Store to channel…");
        b.wait("the question", |app| app.amp_idle());
        b.h.step();
        b.click("Store");
        b.wait("the store", |app| app.store_note.is_some() && app.amp_idle());
        assert_eq!(b.slot(Channel::B3.slot()).name(), "New sound");
        assert_eq!(b.h.state().amp_on, Some(Channel::B3.selector()));
        assert!(!b.fake.lock().unwrap().editor, "editor mode off again");
        let files = names_in(&b.root.join("backups"));
        assert_eq!(files.len(), 2, "{files:?}");
        let channel_backup = files.iter().find(|n| n.contains("_B3_")).unwrap();
        let bytes = std::fs::read(b.root.join("backups").join(channel_backup)).unwrap();
        assert_eq!(Patch::from_syx(&bytes).unwrap(), old_b3);
    }

    #[test]
    fn store_with_live_on_keeps_following() {
        let mut b = bench("store-live").live();
        b.click("Store to channel…");
        b.wait("the question", |app| app.amp_idle());
        b.h.step();
        b.click("Store");
        b.wait("the store", |app| app.store_note.is_some() && app.amp_idle());
        assert!(b.fake.lock().unwrap().editor, "Live goes on");
        let name = b.h.state().editor.patch().clone();
        b.fake.lock().unwrap().turn_panel(GAIN_ADDR, &[33]);
        b.wait("panel change", |app| gain_of(app.editor.patch()) == Some(33));
        assert_ne!(b.h.state().editor.patch(), &name);
    }

    #[test]
    fn unplug_and_return_with_live_on() {
        let mut b = bench("unplug").live();
        let before = b.h.state().editor.patch().clone();
        b.port.store(false, Ordering::SeqCst);
        b.wait("not connected", |app| !app.connected);
        assert_eq!(b.h.state().editor.patch(), &before);
        assert!(b.h.state().live);
        b.h.state_mut().edit(find_param(GAIN).unwrap(), 9).unwrap();
        b.port.store(true, Ordering::SeqCst);
        b.wait("back", |app| app.connected && app.amp_idle() && gain_of(app.editor.patch()) != Some(9));
        assert_eq!(names_in(&b.root.join("quick-saves")).len(), 1, "the edit was kept");
        assert!(b.fake.lock().unwrap().editor);
    }
}

#[test]
fn amp_reapplying_the_knob_updates_the_panel_and_sends_nothing() {
    let mut r = rig("knob-reapply").connected().live();
    let low_cut = katana_proto::settings::setting("REVERB", 1, "low-cut").unwrap();
    let color = find_param("reverb-color").unwrap();
    r.event(Event::PanelChange { addr: color.addr, data: vec![1] });
    r.event(Event::PanelChange { addr: low_cut.addr, data: vec![14, 9, 5, 18] });
    r.click("DELAY 2 / REVERB");
    assert_eq!(r.app().editor.get(low_cut), Some(14));
    assert_eq!(r.h.get_by_label("LOW CUT").accesskit_node().numeric_value(), Some(14.0));
    assert_eq!(r.jobs(), vec![], "the amp's own changes are not sent back");
}

#[test]
fn drop_while_amp_busy_is_ignored() {
    let mut r = rig("drop-busy").connected().live();
    let chain = find_param("other-chain").unwrap();
    r.app().editor.set(chain, 0).unwrap();
    r.h.run();
    let from = r.h.get_by_label("MOD").rect().center();
    let ev = |r: &mut Rig, e: egui::Event| {
        r.h.event(e);
        r.h.run();
    };
    ev(&mut r, egui::Event::PointerMoved(from));
    ev(&mut r, egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    for dx in [10.0, 20.0, 40.0] {
        ev(&mut r, egui::Event::PointerMoved(from + egui::vec2(dx, 0.0)));
    }
    let to = r.h.get_by_label("Drop MOD before BOOSTER").rect().center();
    r.event(Event::Busy(Some("reading".into())));
    ev(&mut r, egui::Event::PointerMoved(to));
    ev(&mut r, egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    assert_eq!(r.app().editor.get(chain), Some(0));
    assert_eq!(r.jobs(), vec![]);
}
