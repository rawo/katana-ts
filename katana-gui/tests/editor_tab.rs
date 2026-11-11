use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::app::{KatanaApp, MIN_WINDOW};
use katana_proto::{find_param, Address, Patch};

fn harness_sized(size: [f32; 2], app: KatanaApp) -> Harness<'static, KatanaApp> {
    Harness::builder().with_size(size).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app)
}

fn harness() -> Harness<'static, KatanaApp> {
    harness_sized([1600.0, 960.0], KatanaApp::new())
}

fn get(h: &Harness<KatanaApp>, name: &str) -> Option<u32> {
    h.state().editor.get(find_param(name).unwrap())
}

#[test]
fn chain_shows_blocks_in_order() {
    let mut h = harness();
    set(&mut h, "other-chain", 1); // BOOSTER, MOD, AMP, FX, DELAY
    let xs: Vec<f32> = ["BOOSTER", "MOD", "AMP", "FX", "DELAY", "DELAY 2 / REVERB"]
        .iter()
        .map(|t| h.get_by_label(t).rect().center().x)
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "chain out of order: {xs:?}");
    h.get_by_label("CLEAN");
}

#[test]
fn booster_selected_by_default_shows_drive() {
    let h = harness();
    h.get_by_label_contains("BOOSTER · GREEN variation");
    h.get_by_label("DRIVE");
}

#[test]
fn clicking_led_switches_variation() {
    let mut h = harness();
    h.get_by_label("BOOSTER ORANGE").click();
    h.run();
    assert_eq!(get(&h, "booster-color"), Some(2));
    h.get_by_label_contains("BOOSTER · ORANGE variation");
}

#[test]
fn delay_led_switches_variation_and_its_settings() {
    let mut h = harness();
    h.get_by_label("DELAY RED").click();
    h.run();
    assert_eq!(get(&h, "delay-color"), Some(1));
    h.get_by_label("DELAY").click();
    h.run();
    h.get_by_label_contains("DELAY · RED variation");
    for k in ["DELAY TIME", "FEEDBACK", "EFFECT LEVEL"] {
        h.get_by_label(k);
    }
}

#[test]
fn reverb_led_switches_variation_and_its_level() {
    let mut h = harness();
    h.get_by_label("DELAY 2 / REVERB ORANGE").click();
    h.run();
    assert_eq!(get(&h, "reverb-color"), Some(2));
    h.get_by_label("DELAY 2 / REVERB").click();
    h.run();
    h.get_by_label_contains("DELAY 2 / REVERB · ORANGE variation");
    // One EFFECT LEVEL for DELAY 2, one for REVERB.
    assert_eq!(h.get_all_by_label("EFFECT LEVEL").count(), 2);
}

#[test]
fn delay_card_has_a_tap_light_dark_with_delay_off() {
    let mut h = harness();
    h.state_mut().edit(find_param("delay-sw").unwrap(), 0).unwrap();
    h.run();
    assert_eq!(h.get_by_label("DELAY TAP").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
    assert!(h.query_by_label("BOOSTER TAP").is_none());
}

#[test]
fn clicking_a_cards_type_text_selects_the_card() {
    let mut h = harness();
    h.get_by_label("CLEAN").click();
    h.run();
    assert_eq!(h.state().selected_block, "amp");
}

#[test]
fn mod_knob_setting_with_value_names_picks_by_name() {
    let mut h = harness();
    h.state_mut().edit(find_param("mod1-type").unwrap(), 12).unwrap();
    h.get_by_label("MOD").click();
    h.run();
    h.get_by_label("MOD · GREEN variation · type HARMONIST");
    h.get_by_label("HARMONY 1").click();
    h.run();
    // The drop-down shows ~200 px; items below need scrolling.
    h.get_by_label("-1oct").click();
    h.run();
    assert_eq!(get(&h, "mod1-harmonist-harmony1"), Some(7));
}

#[test]
fn fx_shows_the_knob_settings_of_its_colours_type() {
    let mut h = harness();
    h.state_mut().edit(find_param("fx1-type").unwrap(), 16).unwrap();
    h.get_by_label("FX").click();
    h.run();
    h.get_by_label("FX · GREEN variation · type TREMOLO");
    h.get_by_label("RATE");
    h.get_by_label("FX RED").click();
    h.run();
    assert_eq!(get(&h, "fx-color"), Some(1));
    h.get_by_label("FX · RED variation · type T.WAH");
    h.get_by_label("SENS");
}

#[test]
fn mod_shows_the_knob_settings_of_its_colours_type() {
    let mut h = harness();
    h.state_mut().edit(find_param("mod1-type").unwrap(), 23).unwrap();
    h.state_mut().edit(find_param("mod2-type").unwrap(), 25).unwrap();
    h.get_by_label("MOD").click();
    h.run();
    h.get_by_label("CHORUS");
    h.get_by_label("LOW RATE");
    h.get_by_label("HIGH RATE");
    h.get_by_label("MOD RED").click();
    h.run();
    assert_eq!(get(&h, "mod-color"), Some(1));
    h.get_by_label("MOD · RED variation · type PHASER 90E");
    h.get_by_label("SPEED");
}

#[test]
fn amp_detail_shows_type_heading() {
    let mut h = harness();
    h.get_by_label("AMP").click();
    h.run();
    h.get_by_label("AMP · type CLEAN");
    h.get_by_label("CAB RESONANCE");
}

#[test]
fn amp_strip_always_visible() {
    let mut h = harness();
    h.get_by_label("DELAY").click();
    h.run();
    for k in ["GAIN", "VOLUME", "BASS", "MIDDLE", "TREBLE", "PRESENCE"] {
        h.get_by_label(k);
    }
}

#[test]
fn pill_toggles_switch() {
    let mut h = harness();
    h.get_by_label("DELAY OFF").click();
    h.run();
    assert_eq!(get(&h, "delay-sw"), Some(1));
    h.get_by_label("DELAY ON");
    h.get_by_label("unsaved");
}

#[test]
fn out_of_range_value_renders() {
    let mut p = Patch::default_preset();
    p.mem.apply(Address(find_param("booster1-drive").unwrap().addr), &[127]);
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&p.to_syx(), "odd.syx".into()).unwrap();
    let h = harness_sized([1600.0, 960.0], app);
    assert_eq!(h.get_by_label("DRIVE").accesskit_node().numeric_value(), Some(127.0));
}

#[test]
fn editor_renders_in_small_window() {
    let h = harness_sized([1280.0, 720.0], KatanaApp::new());
    h.get_by_label("PRESENCE");
}

#[test]
fn hovering_out_of_range_knob_changes_nothing() {
    let mut p = Patch::default_preset();
    p.mem.apply(Address(find_param("booster1-drive").unwrap().addr), &[127]);
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&p.to_syx(), "odd.syx".into()).unwrap();
    let mut h = harness_sized([1600.0, 960.0], app);
    h.get_by_label("DRIVE").hover();
    h.run();
    assert_eq!(get(&h, "booster1-drive"), Some(127));
    assert!(!h.state().editor.is_dirty());
}

const TOP_ITEMS: [&str; 18] = [
    "not connected",
    "PANEL",
    "A1",
    "A2",
    "A3",
    "A4",
    "B1",
    "B2",
    "B3",
    "B4",
    "Live edit",
    "Import…",
    "Export .syx…",
    "Quick save",
    "Save",
    "Save as…",
    "Load to amp",
    "Store to channel…",
];

fn assert_no_overlap(h: &Harness<KatanaApp>, labels: &[&str]) {
    let rects: Vec<_> = labels.iter().map(|l| (l, h.get_by_label(l).rect())).collect();
    for (i, (a, ra)) in rects.iter().enumerate() {
        assert!(ra.right() <= MIN_WINDOW[0], "{a} ends at {} past the window", ra.right());
        for (b, rb) in &rects[i + 1..] {
            assert!(!ra.intersects(*rb), "{a} {ra:?} overlaps {b} {rb:?}");
        }
    }
}

#[test]
fn top_bar_fits_minimum_width() {
    let h = harness_sized(MIN_WINDOW, KatanaApp::new());
    let mut labels = TOP_ITEMS.to_vec();
    labels.push("DEFAULT");
    assert_no_overlap(&h, &labels);
}

#[test]
fn top_bar_fits_longest_name() {
    // The top bar shows the file name, which can be much longer than a patch name.
    let wide = "W".repeat(60);
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&Patch::default_preset().to_syx(), format!("{wide}.syx").into()).unwrap();
    app.editor.set(find_param("gain").unwrap(), 1).unwrap(); // shows "unsaved" too
    let h = harness_sized(MIN_WINDOW, app);
    let mut labels = TOP_ITEMS.to_vec();
    labels.extend([wide.as_str(), "unsaved"]);
    assert_no_overlap(&h, &labels);
}

#[test]
fn chain_fits_minimum_width() {
    let h = harness_sized(MIN_WINDOW, KatanaApp::new());
    assert!(h.get_by_label("OUT").rect().right() <= MIN_WINDOW[0]);
}

#[test]
fn long_error_leaves_theme_picker_visible() {
    let mut app = KatanaApp::new();
    app.error = Some(format!("Couldn't save {}.syx: Permission denied", "x".repeat(200)));
    let h = harness_sized(MIN_WINDOW, app);
    let theme = h.get_by_label("Theme").rect();
    let error = h.get_by_label_contains("Couldn't save").rect();
    assert!(theme.right() <= MIN_WINDOW[0]);
    assert!(error.right() <= theme.left(), "error {error:?} runs into Theme {theme:?}");
}

#[test]
fn unknown_switch_value_shows_as_unknown() {
    let mut p = Patch::default_preset();
    p.mem.apply(Address(find_param("delay-sw").unwrap().addr), &[5]);
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&p.to_syx(), "odd.syx".into()).unwrap();
    let mut h = harness_sized([1600.0, 960.0], app);
    h.get_by_label("DELAY ? (5)").click();
    h.run();
    assert_eq!(get(&h, "delay-sw"), Some(1));
    h.get_by_label("DELAY ON");
}

fn wheel(h: &mut Harness<KatanaApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
}

fn stepped(app: KatanaApp) -> Harness<'static, KatanaApp> {
    Harness::builder()
        .with_size([1600.0, 960.0])
        .with_step_dt(0.05)
        .build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app)
}

#[test]
fn wheel_scrolling_page_passes_over_knob() {
    let mut h = stepped(KatanaApp::new());
    let empty = egui::pos2(1400.0, 800.0);
    let drive = h.get_by_label("DRIVE").rect().center();
    wheel(&mut h, empty);
    wheel(&mut h, drive);
    assert_eq!(get(&h, "booster1-drive"), Some(0));
}

#[test]
fn wheel_after_pause_turns_knob() {
    let mut h = stepped(KatanaApp::new());
    let empty = egui::pos2(1400.0, 800.0);
    let drive = h.get_by_label("DRIVE").rect().center();
    wheel(&mut h, empty);
    for _ in 0..12 {
        h.step(); // 0.6 s
    }
    wheel(&mut h, drive);
    assert_eq!(get(&h, "booster1-drive"), Some(1));
}

fn set(h: &mut Harness<KatanaApp>, name: &str, v: u32) {
    h.state_mut().editor.set(find_param(name).unwrap(), v).unwrap();
    h.run();
}

fn select(h: &mut Harness<KatanaApp>, card: &str) {
    h.get_by_label(card).click();
    h.run();
}

#[test]
fn colour_tab_shows_that_colour_without_changing_the_amp() {
    let mut h = harness();
    set(&mut h, "mod2-type", 23);
    select(&mut h, "MOD");
    h.get_by_label("RED · CHORUS").click();
    h.run();
    assert_eq!(get(&h, "mod-color"), Some(0), "the amp still plays GREEN");
    h.get_by_label("MOD · RED variation · type CHORUS");
    h.get_by_label("LOW RATE");
}

#[test]
fn mod_type_drop_down_changes_the_shown_colours_type() {
    let mut h = harness();
    select(&mut h, "MOD");
    h.get_by_label_contains("RED ·").click();
    h.run();
    h.get_by_label("MOD TYPE").click();
    h.run();
    // The drop-down shows ~200 px; COMP is near the top.
    h.get_all_by_label("COMP").last().unwrap().click();
    h.run();
    assert_eq!(get(&h, "mod2-type"), Some(3));
    assert_eq!(get(&h, "mod1-type"), Some(0));
    h.get_by_label("SUSTAIN");
}

#[test]
fn knob_settings_are_marked() {
    let mut h = harness();
    set(&mut h, "mod1-type", 23);
    select(&mut h, "MOD");
    h.get_by_label("Panel knob: LOW RATE");
    assert!(h.query_by_label("Panel knob: DIRECT MIX").is_none());
}

#[test]
fn picked_tab_stays_when_the_playing_colour_changes() {
    let mut h = harness();
    select(&mut h, "MOD");
    h.get_by_label_contains("ORANGE ·").click();
    h.run();
    set(&mut h, "mod-color", 1);
    h.get_by_label_contains("MOD · ORANGE variation");
}

#[test]
fn every_setting_of_the_type_is_shown() {
    let mut h = harness();
    set(&mut h, "delay1-type", 7);
    select(&mut h, "DELAY");
    for l in ["DELAY TIME", "FEEDBACK", "EFFECT LEVEL", "DIRECT MIX", "MOD SW", "RANGE", "FILTER"] {
        h.get_by_label(l);
    }
}

#[test]
fn harmonist_panel_fits_the_smallest_window() {
    let mut h = harness_sized(MIN_WINDOW, KatanaApp::new());
    set(&mut h, "mod1-type", 12);
    select(&mut h, "MOD");
    h.get_by_label("MASTER KEY");
    h.get_by_label("GAIN");
}

#[test]
fn delay2_reverb_card_follows_delay() {
    let h = harness();
    let x = |t: &str| h.get_by_label(t).rect().center().x;
    assert!(x("DELAY") < x("DELAY 2 / REVERB"));
}

#[test]
fn delay2_reverb_pill_turns_off_both_switches_in_dly_rev() {
    let mut h = harness();
    set(&mut h, "reverb1-layer-mode", 1);
    set(&mut h, "reverb-sw", 1);
    set(&mut h, "sw-delay2-sw", 1);
    h.get_by_label("DELAY 2 / REVERB ON").click();
    h.run();
    assert_eq!((get(&h, "reverb-sw"), get(&h, "sw-delay2-sw")), (Some(0), Some(0)));
    h.get_by_label("DELAY 2 / REVERB OFF").click();
    h.run();
    assert_eq!((get(&h, "reverb-sw"), get(&h, "sw-delay2-sw")), (Some(1), Some(1)));
}

#[test]
fn delay2_reverb_panel_shows_both_halves_in_reverb_mode() {
    let mut h = harness();
    set(&mut h, "reverb1-layer-mode", 2);
    select(&mut h, "DELAY 2 / REVERB");
    h.get_by_label_contains("DELAY 2 / REVERB · GREEN variation");
    h.get_by_label("DELAY 2 TYPE");
    h.get_by_label("REVERB TYPE");
    let time = h.get_by_label("DELAY TIME");
    assert!(!time.accesskit_node().is_disabled(), "the dimmed half stays editable");
    h.get_by_label("REVERB TIME");
}

#[test]
fn changing_the_mode_while_on_moves_the_switches() {
    let mut h = harness();
    set(&mut h, "reverb1-layer-mode", 2);
    set(&mut h, "reverb-sw", 1);
    select(&mut h, "DELAY 2 / REVERB");
    h.get_by_label("MODE").click();
    h.run();
    h.get_all_by_label("DELAY").last().unwrap().click();
    h.run();
    assert_eq!(get(&h, "reverb1-layer-mode"), Some(0));
    assert_eq!((get(&h, "sw-delay2-sw"), get(&h, "reverb-sw")), (Some(1), Some(0)));
}

#[test]
fn eq_cards_sit_around_the_amp_and_solo_last() {
    let mut h = harness();
    set(&mut h, "eq1-position", 0);
    set(&mut h, "eq2-position", 1);
    let x = |t: &str| h.get_by_label(t).rect().center().x;
    assert!(x("EQ 1") < x("AMP") && x("AMP") < x("EQ 2"));
    assert!(x("DELAY 2 / REVERB") < x("SOLO"));
}

#[test]
fn eq_panel_shows_position_type_and_ge10_faders() {
    let mut h = stepped(KatanaApp::new());
    set(&mut h, "eq1-type", 1);
    select(&mut h, "EQ 1");
    h.get_by_label("POSITION");
    h.get_by_label("TYPE");
    let k1 = h.get_by_label("1kHz");
    assert_eq!(k1.accesskit_node().role(), egui::accesskit::Role::Slider);
    h.get_by_label("16kHz");
    let before = get(&h, "geq1-1khz").unwrap();
    let at = h.get_by_label("1kHz").rect().center();
    for _ in 0..12 {
        h.step();
    }
    wheel(&mut h, at);
    assert_eq!(get(&h, "geq1-1khz"), Some(before + 1));
}

#[test]
fn eq_parametric_shows_knobs() {
    let mut h = harness();
    set(&mut h, "eq2-type", 0);
    select(&mut h, "EQ 2");
    h.get_by_label("LOW-MID FREQ");
    assert!(h.query_by_label("1kHz").is_none());
}

#[test]
fn solo_panel_shows_the_solo_delay_read_only() {
    let mut h = harness();
    select(&mut h, "SOLO");
    h.get_by_label("SOLO LEVEL");
    h.get_by_label("SOLO EQ SW");
    h.get_by_label("MID FREQ");
    h.get_by_label_contains("doesn't accept changes to the solo delay");
    assert!(h.query_by_label("FEEDBACK").is_none(), "no editable solo delay knob");
    h.get_by_label_contains("FEEDBACK:");
}


#[test]
fn amp_strip_has_the_hidden_amp_settings() {
    let h = harness();
    for l in ["RESONANCE", "BLOOM", "VARIATION"] {
        h.get_by_label(l);
    }
}

#[test]
fn contour_buttons_pick_a_contour_and_show_its_settings() {
    let mut h = harness();
    set(&mut h, "contour", 0);
    assert!(h.query_by_label("FREQ SHIFT").is_none());
    h.get_by_label("CONTOUR 2").click();
    h.run();
    assert_eq!(get(&h, "contour"), Some(2));
    h.get_by_label("CONTOUR TYPE");
    let shift = h.get_by_label("FREQ SHIFT");
    assert_eq!(shift.accesskit_node().numeric_value(), Some(-50.0), "raw 0 in the default preset");
    set(&mut h, "contour2-freq-shift", 60);
    assert_eq!(h.get_by_label("FREQ SHIFT").accesskit_node().numeric_value(), Some(10.0));
}

fn press(h: &mut Harness<KatanaApp>, at: egui::Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
}

/// Presses on `card`, moves the pointer 40 px, then calls `then` while still pressed.
fn start_drag(h: &mut Harness<KatanaApp>, card: &str) -> egui::Pos2 {
    let from = h.get_by_label(card).rect().center();
    h.event(egui::Event::PointerMoved(from));
    h.run();
    press(h, from, true);
    for dx in [10.0, 20.0, 40.0] {
        h.event(egui::Event::PointerMoved(from + egui::vec2(dx, 0.0)));
        h.run();
    }
    from
}

fn drop_on(h: &mut Harness<KatanaApp>, spot: &str) {
    let to = h.get_by_label(spot).rect().center();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    press(h, to, false);
}

#[test]
fn chain_follows_the_patch_order() {
    let mut h = harness();
    set(&mut h, "other-chain", 6);
    let x = |t: &str| h.get_by_label(t).rect().center().x;
    let xs: Vec<f32> = ["MOD", "BOOSTER", "FX", "DELAY", "AMP"].iter().map(|t| x(t)).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
}

#[test]
fn dragging_mod_offers_only_allowed_spots_and_drop_sets_chain() {
    let mut h = harness();
    set(&mut h, "other-chain", 0);
    set(&mut h, "eq1-position", 1);
    set(&mut h, "eq2-position", 1);
    start_drag(&mut h, "MOD");
    h.get_by_label("Drop MOD before AMP");
    h.get_by_label("Drop MOD before BOOSTER");
    assert!(h.query_by_label("Drop MOD before DELAY").is_none());
    drop_on(&mut h, "Drop MOD before BOOSTER");
    assert_eq!(get(&h, "other-chain"), Some(4));
    assert!(h.query_by_label("Drop MOD before AMP").is_none(), "spots go away after the drop");
}

#[test]
fn dropping_elsewhere_changes_nothing() {
    let mut h = harness();
    set(&mut h, "other-chain", 0);
    start_drag(&mut h, "MOD");
    let empty = egui::pos2(1400.0, 800.0);
    h.event(egui::Event::PointerMoved(empty));
    h.run();
    press(&mut h, empty, false);
    assert_eq!(get(&h, "other-chain"), Some(0));
}

#[test]
fn eq_drag_sets_position() {
    let mut h = harness();
    set(&mut h, "eq1-position", 0);
    set(&mut h, "eq2-position", 1);
    start_drag(&mut h, "EQ 1");
    drop_on(&mut h, "Drop EQ 1 before EQ 2");
    assert_eq!(get(&h, "eq1-position"), Some(1));
}

#[test]
fn a_click_still_selects_a_card() {
    let mut h = harness();
    let at = h.get_by_label("FX").rect().center();
    h.event(egui::Event::PointerMoved(at));
    press(&mut h, at, true);
    press(&mut h, at, false);
    assert_eq!(h.state().selected_block, "fx");
}
