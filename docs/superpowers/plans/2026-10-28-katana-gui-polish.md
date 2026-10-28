# KATANA GUI Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the seven minor issues left from the GUI-shell review, plus the too-small heading and status text seen on Linux at 1920×1080.

**Architecture:** Small, independent changes inside `katana-gui`. The theme sets readable text sizes. The top bar splits into two rows, and the window gets a minimum size the chain fits in. The discard prompt honours Escape and backdrop clicks. On/off controls show unknown raw values honestly. Save writes a temp file and renames it. The knob lets the mouse wheel through while the page is scrolling.

**Tech Stack:** Rust 2021, eframe/egui 0.37, egui_kittest 0.37, rfd 0.17.

**Spec:** `docs/superpowers/specs/2026-10-22-katana-gui-design.md`. Previous plan: `docs/superpowers/plans/2026-10-26-katana-gui-shell.md`.

## Global Constraints

- Linux is the only target; development also happens on macOS. No OS-specific code.
- "Knobs: drag up/down; wheel = fine step; double-click = type a value; value always shown."
- "Something on screen always states: connected or not, live on/off, unsaved changes, last panel change."
- Top bar: "connection status; preset name + "unsaved" marker; **Live edit** switch; Quick save, Save, Save as…, Load to amp, Store to channel…"; "Channel tabs (always visible): PANEL, A1–A4, B1–B4".
- Only parameters verified on the amp are shown (`katana_proto::PARAMS`).
- egui's default fonts have no "●", "→" or emoji: paint them as shapes.
- Every widget needs an accessible name (egui_kittest checks it).
- `cargo fmt` (max_width 120); no new clippy warnings.
- Work on branch `feat/gui-polish`; check each layout change in the Linux VM with `scripts/vm.sh run` and `scripts/vm.sh shot`.

## Review Focus

1. A 16-character preset name made of wide letters at the minimum window width: the top bar must still not overlap (test in Task 2).
2. A long error message in the status line: it must not cover the Theme picker (test in Task 2).
3. Switching to the Light theme must keep the larger text sizes (test in Task 1).
4. A failed save must leave the existing file untouched and no temp file behind (tests in Task 5).
5. The page scrolls under a resting pointer and a knob slides under it: that knob must not turn (test in Task 6).

---

### Task 1: Readable small text

The section headings ("SIGNAL CHAIN", "AMP KNOBS", detail headings) and the status line use `TextStyle::Small`, egui's 9 pt default. That is too small at 1920×1080.

**Files:**
- Modify: `katana-gui/src/theme.rs`
- Test: `katana-gui/tests/shell.rs`

**Interfaces:**
- Produces: `pub const SMALL_TEXT: f32 = 11.0;` in `theme.rs`. `theme::apply` sets `TextStyle::Small` to `FontId::proportional(SMALL_TEXT)` for both the dark and light styles.

- [ ] **Step 1: Write the failing tests** in `tests/shell.rs`

```rust
fn small_size(h: &Harness<KatanaApp>) -> f32 {
    h.ctx.global_style().text_styles[&egui::TextStyle::Small].size
}

#[test]
fn small_text_is_readable() {
    let h = harness();
    assert_eq!(small_size(&h), katana_gui::theme::SMALL_TEXT);
}

#[test]
fn light_theme_keeps_text_sizes() {
    let mut h = harness();
    h.get_by_label("Theme").click();
    h.run();
    h.get_by_label("Light").click();
    h.run();
    assert_eq!(small_size(&h), katana_gui::theme::SMALL_TEXT);
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p katana-gui --test shell`
Expected: FAIL to compile, because `SMALL_TEXT` is not defined.

- [ ] **Step 3: Implement**

In `theme::apply`, add `SMALL_TEXT` and set the Small style with `ctx.all_styles_mut(|s| { s.text_styles.insert(TextStyle::Small, FontId::proportional(SMALL_TEXT)); })`.

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add katana-gui/src/theme.rs katana-gui/tests/shell.rs
git commit -m "fix(gui): readable small text (11 pt)"
```

---

### Task 2: Top bar in two rows, minimum window size

Below about 1300 px the top bar's two halves overlap. Below about 1165 px the chain's OUT label is cut off (measured: OUT ends at x = 1131 with the window's margins). The fix:
- Split the top bar into two rows.
- Give the window a minimum size the chain fits in.
- Truncate the status-line error so it can't cover the Theme picker.

New layout:
- **Row 1** (`views::top_bar`): connection dot, channel tabs, and, right-aligned, the preset name, "unsaved" marker and Live edit.
- **Row 2** (`views::action_bar`, in the existing `"tabs"` panel): the Editor/Library tabs on the left. Right-aligned, in this order from left to right: Open .syx…, Save .syx…, Quick save, Save, Save as…, Load to amp, Store to channel….

**Files:**
- Modify: `katana-gui/src/views/top_bar.rs`, `katana-gui/src/views/mod.rs`, `katana-gui/src/views/status.rs`, `katana-gui/src/app.rs`, `katana-gui/src/main.rs`
- Test: `katana-gui/tests/editor_tab.rs`

**Interfaces:**
- Produces: `pub const MIN_WINDOW: [f32; 2] = [1200.0, 720.0];` in `app.rs`, used by `main.rs` (`with_min_inner_size(MIN_WINDOW)`) and by the tests. `pub fn action_bar(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette)` in a new `views/action_bar.rs`, re-exported from `views`. The `"tabs"` panel in `KatanaApp::show` calls it instead of drawing the two tab buttons itself.

- [ ] **Step 1: Write the failing tests** in `tests/editor_tab.rs`

```rust
const TOP_ITEMS: [&str; 18] = [
    "not connected", "PANEL", "A1", "A2", "A3", "A4", "B1", "B2", "B3", "B4", "Live edit",
    "Open .syx…", "Save .syx…", "Quick save", "Save", "Save as…", "Load to amp", "Store to channel…",
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
    let mut p = Patch::default_preset();
    p.mem.apply(Address(katana_proto::device::addr([0x20, 0, 0, 0])), b"WWWWWWWWWWWWWWWW");
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&p.to_syx(), "wide.syx".into()).unwrap();
    app.editor.set(find_param("gain").unwrap(), 1).unwrap(); // shows "unsaved" too
    let h = harness_sized(MIN_WINDOW, app);
    let mut labels = TOP_ITEMS.to_vec();
    labels.extend(["WWWWWWWWWWWWWWWW", "unsaved"]);
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
```

Import `katana_gui::app::MIN_WINDOW`.

- [ ] **Step 2: Run them**

Run: `cargo test -p katana-gui --test editor_tab`
Expected: FAIL to compile (`MIN_WINDOW` is missing). Once it compiles:
- `top_bar_fits_minimum_width`, `top_bar_fits_longest_name` and `long_error_leaves_theme_picker_visible` fail on overlap before Step 3.
- `chain_fits_minimum_width` already passes (OUT ends at x ≈ 1131), which pins it.

- [ ] **Step 3: Implement**

- Lay out the two rows as described above. The Editor/Library tab buttons move unchanged into `action_bar`.
- In `main.rs`, set `.with_min_inner_size(MIN_WINDOW)`.
- In `status.rs`:
  - Draw the Theme picker first (right to left), then the status text and error in the remaining width.
  - The error label uses `.truncate()`, with the full text on hover.
- If `chain_fits_minimum_width` still fails, reduce the card minimum width in `views/chain.rs` (now 120) until OUT fits, but not below 100.

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass, including the existing `shell.rs` tests that look up "Editor", "Library" and the disabled buttons.

- [ ] **Step 5: Look at it on Linux**

Run: `scripts/vm.sh run && scripts/vm.sh shot`
Expected: two tidy rows at 1920×1080. Then un-maximise the VM window and drag it as narrow as it will go: it stops at 1200 px wide, and nothing overlaps.

- [ ] **Step 6: Commit**

```bash
git add katana-gui/src katana-gui/tests/editor_tab.rs
git commit -m "fix(gui): two-row top bar and a minimum window size the chain fits"
```

---

### Task 3: Escape or a click outside cancels the discard prompt

**Files:**
- Modify: `katana-gui/src/app.rs` (`discard_prompt`)
- Test: `katana-gui/tests/files.rs`

**Interfaces:**
- Consumes: `egui::ModalResponse::should_close()`, which is true for Escape or a backdrop click.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn escape_cancels_discard_prompt() {
    let d = tmp("escape");
    let path = patch_file(&d);
    let mut h = harness();
    set_gain(&mut h, 5);
    h.state_mut().request_open(path.clone());
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label("Discard unsaved changes to DEFAULT?").is_none());
    assert_eq!(gain(&h), Some(5));
    // Nothing is left pending: asking again shows the prompt again.
    h.state_mut().request_open(path);
    h.run();
    h.get_by_label("Discard unsaved changes to DEFAULT?");
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p katana-gui --test files escape`
Expected: FAIL, because the prompt is still shown after Escape.

- [ ] **Step 3: Implement**

In `discard_prompt`, keep the `ModalResponse`. If `should_close()` is true and no button was clicked, treat it as Cancel (`choice = Some(false)`).

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add katana-gui/src/app.rs katana-gui/tests/files.rs
git commit -m "fix(gui): Escape or a click outside cancels the discard prompt"
```

---

### Task 4: Unknown on/off values are shown as unknown

An on/off byte other than 0 or 1 (possible in an opened `.syx`) now shows as OFF, both in the detail panel's toggle and in the chain's pill. Both should show `? (n)`. Clicking still turns it ON (1), as before.

**Files:**
- Modify: `katana-gui/src/widgets/control.rs`, `katana-gui/src/widgets/mod.rs`, `katana-gui/src/views/chain.rs` (`pill`)
- Test: unit tests in `control.rs`; `katana-gui/tests/editor_tab.rs`

**Interfaces:**
- Produces: `pub fn on_off_text(value: u32) -> String` in `widgets/control.rs`, re-exported from `widgets`: 0 → `"OFF"`, 1 → `"ON"`, anything else → `"? (n)"`. Both the toggle and the pill use it. The pill's accessible name stays `"<TITLE> <text>"`.

- [ ] **Step 1: Write the failing tests**

In `control.rs`:

```rust
#[test]
fn on_off_text_names_unknown_values() {
    assert_eq!(on_off_text(0), "OFF");
    assert_eq!(on_off_text(1), "ON");
    assert_eq!(on_off_text(5), "? (5)");
}
```

In `tests/editor_tab.rs`:

```rust
#[test]
fn unknown_switch_value_shows_as_unknown() {
    let mut p = Patch::default_preset();
    p.mem.apply(Address(find_param("reverb-sw").unwrap().addr), &[5]);
    let mut app = KatanaApp::new();
    app.editor.open_bytes(&p.to_syx(), "odd.syx".into()).unwrap();
    let mut h = harness_sized([1600.0, 960.0], app);
    h.get_by_label("REVERB ? (5)").click();
    h.run();
    assert_eq!(get(&h, "reverb-sw"), Some(1));
    h.get_by_label("REVERB ON");
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p katana-gui unknown`
Expected: FAIL to compile (`on_off_text` is missing). Once it compiles, the editor-tab test fails because no "REVERB ? (5)" node exists.

- [ ] **Step 3: Implement**

- The toggle's button text and the pill's text both come from `on_off_text`.
- The pill's width is the larger of 34 px and the text width plus 8 px.
- The pill is filled green only for 1. Unknown values use the OFF colours.

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add katana-gui/src katana-gui/tests/editor_tab.rs
git commit -m "fix(gui): show unknown on/off values instead of OFF"
```

---

### Task 5: Atomic save; `saved_to` returns nothing

Saving now writes the file in place, so a failed write can leave a half-written file. Write `<file name>.tmp` in the same folder, then rename it over the target. Also, `Editor::saved_to` returns bytes that nothing uses.

**Files:**
- Modify: `katana-gui/src/app.rs` (`save_to`), `katana-gui/src/editor.rs` (`saved_to` and its unit test)
- Test: `katana-gui/tests/files.rs`

**Interfaces:**
- Produces: `fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()>` (private, `app.rs`). The temp file is the target's file name plus `.tmp` (e.g. `out.syx.tmp`). On a failed rename it removes the temp file. `Editor::saved_to(&mut self, path: PathBuf)` now returns `()`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn failed_save_keeps_existing_file() {
    let d = tmp("save-keep");
    let out = d.join("out.syx");
    std::fs::write(&out, b"old").unwrap();
    std::fs::create_dir(d.join("out.syx.tmp")).unwrap(); // the temp file can't be written
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().save_to(out.clone());
    assert_eq!(std::fs::read(&out).unwrap(), b"old");
    assert!(h.state().editor.is_dirty());
    assert!(h.state().error.as_deref().unwrap().starts_with("Couldn't save out.syx"));
}

#[test]
fn save_overwrites_and_leaves_no_temp_file() {
    let d = tmp("save-over");
    let out = d.join("out.syx");
    std::fs::write(&out, b"old").unwrap();
    let mut h = harness();
    set_gain(&mut h, 7);
    h.state_mut().save_to(out.clone());
    let saved = Patch::from_syx(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(saved.get(find_param("gain").unwrap()), Some(7));
    let names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["out.syx"]);
}
```

- [ ] **Step 2: Run them**

Run: `cargo test -p katana-gui --test files save`
Expected: `failed_save_keeps_existing_file` FAILS, because the direct write succeeds and replaces "old". `save_overwrites_and_leaves_no_temp_file` already passes, which pins today's behaviour.

- [ ] **Step 3: Implement**

- `write_atomic`, used by `save_to`. The error message format stays the same, with the target's file name.
- Change `saved_to` to return `()`. Its unit test `open_then_saved_round_trip` reads the bytes with `e.patch().to_syx()` instead.

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add katana-gui/src/app.rs katana-gui/src/editor.rs katana-gui/tests/files.rs
git commit -m "fix(gui): save through a temp file and rename"
```

---

### Task 6: The wheel scrolls the page past knobs

The spec says wheel = fine step on a knob. But when the page is scrolling (in a small window) and a knob slides under the pointer, the knob takes the wheel and stops the scroll. The rule:
- A knob ignores the wheel while the page has been scrolled within the last `WHEEL_LATCH_S` = 0.5 s, and each wheel event it lets through extends that window.
- After a pause, the wheel turns the knob as before.

**Files:**
- Modify: `katana-gui/src/widgets/knob.rs`, `katana-gui/src/widgets/mod.rs`, `katana-gui/src/app.rs` (end of `show`)
- Test: `katana-gui/tests/editor_tab.rs`

**Interfaces:**
- Produces, in `knob.rs`, re-exported from `widgets`:
  - `pub const WHEEL_LATCH_S: f64 = 0.5;`
  - `pub fn track_page_wheel(ctx: &egui::Context)`. Call it once at the end of each frame (the last line of `KatanaApp::show`). If this frame had wheel input and no knob took it, it stores the frame's time (`i.time`) as the last page scroll. A knob that takes the wheel marks the frame with `ctx.cumulative_pass_nr()`; both values live in `ctx.data` under fixed `Id`s.
- The knob takes the wheel only if `now - last_page_scroll >= WHEEL_LATCH_S`. When it lets the wheel through, it doesn't touch `smooth_scroll_delta` and it refreshes the last page scroll to `now`. Without `track_page_wheel` (the `controls.rs` harness), knobs behave as today.

- [ ] **Step 1: Write the failing tests** in `tests/editor_tab.rs`

```rust
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
```

- [ ] **Step 2: Run them**

Run: `cargo test -p katana-gui --test editor_tab wheel`
Expected: `wheel_scrolling_page_passes_over_knob` FAILS (drive is 1). `wheel_after_pause_turns_knob` passes, which pins the spec's wheel = fine step.

- [ ] **Step 3: Implement** as described under Interfaces.

- [ ] **Step 4: Run them**

Run: `cargo test -p katana-gui`
Expected: all pass, including `controls.rs::knob_wheel_steps_gain` and `editor_tab.rs::hovering_out_of_range_knob_changes_nothing`.

- [ ] **Step 5: Try it on Linux**

Run: `scripts/vm.sh run`, then shrink the window to its minimum height and scroll with the mouse over the Editor tab.
Expected: the page scrolls past the knobs. Rest the pointer on a knob for a moment and the wheel turns it by one step.

- [ ] **Step 6: Commit**

```bash
git add katana-gui/src katana-gui/tests/editor_tab.rs
git commit -m "fix(gui): the wheel keeps scrolling the page past knobs"
```

---

### Task 7: Wrap up

- [ ] **Step 1:** `cargo fmt --check && cargo clippy --workspace --all-targets` → no new warnings (the 3 old ones in `capture.rs` and `katana-cli` stay).
- [ ] **Step 2:** `cargo test --workspace` on the Mac and `scripts/vm.sh test` → all pass.
- [ ] **Step 3:** Update the README's GUI section:
  - The two-row top bar.
  - The minimum window size 1200×720.
  - Escape cancels the prompt.
  - The wheel behaviour: one step when resting on a knob; scrolling passes over knobs.
- [ ] **Step 4:** Commit: `git commit -am "docs: README for the GUI polish"`.
