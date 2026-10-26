use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use katana_gui::editor::Editor;
use katana_gui::theme::ThemeId;
use katana_gui::widgets::param_control;
use katana_proto::find_param;

fn harness() -> Harness<'static, Editor> {
    Harness::builder().with_size([600.0, 300.0]).with_step_dt(0.05).with_max_steps(40).build_ui_state(
        |ui, editor: &mut Editor| {
            let p = ThemeId::Dark.palette();
            ui.horizontal(|ui| {
                param_control(ui, editor, find_param("gain").unwrap(), "GAIN", &p);
                param_control(ui, editor, find_param("booster-sw").unwrap(), "BOOSTER", &p);
                param_control(ui, editor, find_param("amp-type").unwrap(), "TYPE", &p);
            });
        },
        Editor::new(),
    )
}

fn get(h: &Harness<Editor>, name: &str) -> Option<u32> {
    h.state().get(find_param(name).unwrap())
}

#[test]
fn toggle_flips_switch() {
    let mut h = harness();
    h.get_by_label("BOOSTER").click();
    h.run();
    assert_eq!(get(&h, "booster-sw"), Some(1));
    assert!(h.state().is_dirty());
}

#[test]
fn combo_selects_named_value() {
    let mut h = harness();
    h.get_by_label("TYPE").click();
    h.run();
    h.get_by_label("CRUNCH").click();
    h.run();
    assert_eq!(get(&h, "amp-type"), Some(3));
}

#[test]
fn knob_drag_up_increases_gain() {
    let mut h = harness();
    let c = h.get_by_label("GAIN").rect().center();
    h.drag_at(c);
    h.run();
    h.hover_at(c - egui::vec2(0.0, 20.0));
    h.run();
    h.drop_at(c - egui::vec2(0.0, 20.0));
    h.run();
    assert_eq!(get(&h, "gain"), Some(10));
}

#[test]
fn knob_wheel_steps_gain() {
    let mut h = harness();
    let c = h.get_by_label("GAIN").rect().center();
    h.hover_at(c);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(get(&h, "gain"), Some(1));
}

fn type_into_gain(h: &mut Harness<Editor>, text: &str) {
    // Two clicks within egui's double-click delay (step_dt is 0.05 s).
    h.get_by_label("GAIN").click();
    h.step();
    h.get_by_label("GAIN").click();
    h.run();
    h.get_by_role(Role::TextInput).type_text(text);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
}

#[test]
fn knob_double_click_types_value() {
    let mut h = harness();
    type_into_gain(&mut h, "77");
    assert_eq!(get(&h, "gain"), Some(77));
}

#[test]
fn knob_typed_garbage_keeps_value() {
    let mut h = harness();
    type_into_gain(&mut h, "x");
    assert_eq!(get(&h, "gain"), Some(0));
    assert!(!h.state().is_dirty());
}
