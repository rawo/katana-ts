use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::app::KatanaApp;
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
    let h = harness();
    let xs: Vec<f32> = ["BOOSTER", "MOD", "AMP", "FX", "DELAY", "REVERB"]
        .iter()
        .map(|t| h.get_by_label(t).rect().center().x)
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "chain out of order: {xs:?}");
    h.get_by_label("CLEAN");
}

#[test]
fn booster_selected_by_default_shows_drive() {
    let h = harness();
    h.get_by_label("BOOSTER · GREEN variation");
    h.get_by_label("DRIVE");
}

#[test]
fn clicking_led_switches_variation() {
    let mut h = harness();
    h.get_by_label("BOOSTER ORANGE").click();
    h.run();
    assert_eq!(get(&h, "booster-color"), Some(2));
    h.get_by_label("BOOSTER · ORANGE variation");
}

#[test]
fn selecting_block_without_params_says_so() {
    let mut h = harness();
    h.get_by_label("MOD").click();
    h.run();
    h.get_by_label("No parameters verified on the amp for this block yet.");
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
    h.get_by_label("REVERB OFF").click();
    h.run();
    assert_eq!(get(&h, "reverb-sw"), Some(1));
    h.get_by_label("REVERB ON");
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
