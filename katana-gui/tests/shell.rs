use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::app::KatanaApp;

fn harness() -> Harness<'static, KatanaApp> {
    Harness::builder()
        .with_size([1600.0, 960.0])
        .build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), KatanaApp::new())
}

#[test]
fn shell_shows_offline_state() {
    let h = harness();
    for label in ["Editor", "Library", "not connected", "DEFAULT", "Open .syx…", "Save .syx…"] {
        h.get_by_label(label);
    }
    assert!(h.query_by_label("unsaved").is_none());
}

#[test]
fn amp_actions_are_disabled() {
    let h = harness();
    for label in
        ["Store to channel…", "Load to amp", "Quick save", "Save", "Save as…", "PANEL", "A1", "B4", "Live edit"]
    {
        assert!(h.get_by_label(label).accesskit_node().is_disabled(), "{label} should be disabled");
    }
}

#[test]
fn library_tab_shows_placeholder() {
    let mut h = harness();
    h.get_by_label("Library").click();
    h.run();
    h.get_by_label("Preset library comes in stage 3.");
}

#[test]
fn theme_switch_changes_visuals() {
    let mut h = harness();
    assert!(h.ctx.global_style().visuals.dark_mode);
    h.get_by_label("Theme").click();
    h.run();
    h.get_by_label("Light").click();
    h.run();
    assert!(!h.ctx.global_style().visuals.dark_mode);
}
