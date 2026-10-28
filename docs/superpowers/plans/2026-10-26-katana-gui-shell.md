# KATANA GUI Shell and Layout B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A runnable egui desktop app (`katana-gui`) showing layout B (top bar, channel tabs, Editor | Library tabs, signal chain, detail panel, amp strip, status line) that edits a patch held in memory and opens/saves it as `.syx`. It does not talk to the amp yet.

**Architecture:** New workspace crate `katana-gui` (lib + bin) on top of `katana-proto`. The editor is a `Patch` plus a saved baseline (dirty = differs from baseline). What to show for each block comes from a small layout description in `katana-proto` (`layout.rs`) built only from the verified `PARAMS`, so the GUI has no model-specific tables of its own. Controls are generic: on/off → toggle, named values → dropdown, anything else → knob. Amp-dependent actions are visible but disabled, with a tooltip naming the stage that brings them.

**Tech Stack:** Rust 2021, `eframe`/`egui` 0.37, `egui_kittest` 0.37 (headless UI tests, dev-dependency), `rfd` 0.17 (native file dialogs).

**Spec:** `docs/superpowers/specs/2026-10-22-katana-gui-design.md` (layout B, "Controls and look", "Decided: how it works"). Mockup: `docs/superpowers/mockups/2026-10-22-main-layout.html` option B.

## Global Constraints

- Linux is the only target. The code must also build and run on macOS for development: no `cfg(target_os)` code, no Linux-only crates, and no macOS packaging or features.
- No MIDI or amp I/O in this plan. `katana-gui` must not use `MidirTransport`, and it must not enable `katana-proto`'s `midi` feature beyond what `default` already does.
- Clean-room rule: show only parameters that are in `katana_proto::PARAMS` (verified on the amp). Don't add parameters, effect types or colours from the official app's tables.
- Anti-goals from the spec that apply now: never silently lose edits (opening a file or closing the window with unsaved changes asks first). Never show "saved/stored/synced" unless it happened. Copy no graphics from the official app.
- Dark theme by default. Palette from the mockup `:root`: bg `#16181c`, panel `#1f2329`, line `#333a44`, text `#d8dde4`, dim `#8a93a0`, accent `#e0a030`, green `#3fbf5a`, red `#e0443c`, orange `#f08a24`.
- Knobs: drag up/down, mouse wheel = 1 raw step, double-click = type a value, value always shown.
- Formatting: `cargo fmt` with the repo's `rustfmt.toml` (max_width 120). `cargo clippy --workspace --all-targets` clean.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`.

## Review Focus

1. **Unsaved edits, then Open .syx or close the window.** Expect a "Discard unsaved changes?" prompt, with the editor unchanged until the user confirms. (Task 6 `open_while_dirty_asks_first`, `close_while_dirty_is_cancelled`)
2. **Opening a file that isn't a full KATANA patch** (truncated, other model, random bytes). Expect the editor unchanged, the error in the status line and no panic. (Task 3 `open_rejects_non_patch_keeps_editor`, Task 6 `open_bad_file_shows_error`)
3. **A loaded patch with a byte outside a parameter's range** (e.g. a `.syx` made by other tools). Expect the raw value shown, no panic, and the first drag clamped into range. (Task 4 `drag_from_out_of_range_clamps`, Task 5 `out_of_range_value_renders`)
4. **Typed knob value that is non-numeric or out of range.** Expect it rejected and the old value kept. (Task 4 `parse_typed_rejects_bad_input`)
5. **Window smaller than 1920×1080** (the Mac laptop during development). Expect the editor to scroll rather than clip controls. (Task 5 `editor_renders_in_small_window`)

---

### Task 1: Patch editing helpers and default preset in `katana-proto`

**Files:**
- Modify: `katana-proto/src/params.rs` (the `impl Patch` block)
- Modify: `katana-proto/src/patch.rs` (`impl Patch`)
- Test: inline `#[cfg(test)]` modules of those files

**Interfaces:**
- Produces:
  - `Patch::set(&mut self, p: &ParamDef, value: u32) -> Result<(), ProtoError>`: uses `encode_param` (same range check) and then `mem.apply`.
  - `Patch::name(&self) -> String`: the 16 bytes at `20 00 00 00` (PATCH COM, verified in `docs/protocol.md`), trailing spaces trimmed, any byte outside `0x20..=0x7E` shown as `?`. Returns `""` if the bytes are missing.
  - `Patch::default_preset() -> Patch`: every byte of `PATCH_BLOCKS` is `0`, then every `PARAMS` entry is set to its `min`, then `amp-type` = 1 (CLEAN), then the name = `"DEFAULT"` padded with spaces to 16 bytes.

- [ ] **Step 1: Write the failing tests**

```rust
// params.rs tests
#[test]
fn patch_set_then_get_round_trips() {
    let mut p = Patch::default_preset();
    let gain = find_param("gain").unwrap();
    p.set(gain, 77).unwrap();
    assert_eq!(p.get(gain), Some(77));
}

#[test]
fn patch_set_rejects_out_of_range_and_keeps_value() {
    let mut p = Patch::default_preset();
    let gain = find_param("gain").unwrap();
    assert!(matches!(p.set(gain, 101), Err(ProtoError::OutOfRange { .. })));
    assert_eq!(p.get(gain), Some(0));
}

// patch.rs tests
#[test]
fn default_preset_covers_all_blocks_and_is_clean() {
    let p = Patch::default_preset();
    assert!(Patch::from_syx(&p.to_syx()).is_ok());
    assert_eq!(p.get(crate::find_param("amp-type").unwrap()), Some(1));
    assert_eq!(p.get(crate::find_param("booster-sw").unwrap()), Some(0));
    assert_eq!(p.get(crate::find_param("delay2-time").unwrap()), Some(1));
    assert_eq!(p.name(), "DEFAULT");
}

#[test]
fn name_trims_padding_and_masks_non_ascii() {
    let mut p = Patch::default_preset();
    p.mem.apply(Address(PATCH_BLOCKS[0].0), b"Bloom\x01Crunch    ");
    assert_eq!(p.name(), "Bloom?Crunch");
}
```

- [ ] **Step 2: Run them and check they fail**

Run: `cargo test -p katana-proto`
Expected: compile errors, because `set`, `name` and `default_preset` don't exist yet.

- [ ] **Step 3: Implement the three methods** with the signatures above. `set` lives next to `get` in `params.rs`; `name` and `default_preset` go in `patch.rs`.

- [ ] **Step 4: Run the tests and check they pass**

Run: `cargo test -p katana-proto`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add katana-proto/src/params.rs katana-proto/src/patch.rs
git commit -m "feat(proto): patch set/name and default preset for the editor"
```

---

### Task 2: Block layout description in `katana-proto`

This is the seed of the spec's "device description" for the UI: which verified parameters each block in the signal chain shows. The chain order is fixed for now (booster → mod → amp → fx → delay → reverb, as in the mockup) because the chain parameter isn't verified yet. Say so in a doc comment.

**Files:**
- Create: `katana-proto/src/layout.rs`
- Modify: `katana-proto/src/lib.rs` (`pub mod layout;`)

**Interfaces:**
- Produces:

```rust
pub struct Control { pub param: &'static str, pub label: &'static str }

pub struct BlockDef {
    pub id: &'static str,            // "booster"
    pub title: &'static str,         // "BOOSTER"
    pub switch: Option<&'static str>,  // on/off param
    pub kind: Option<&'static str>,    // param whose value name is the block's type line
    pub color: Option<&'static str>,   // variation colour param
    pub common: &'static [Control],
    pub per_color: &'static [&'static [Control]], // indexed by the colour value
}

impl BlockDef {
    /// `common` followed by `per_color[colour]`; just `common` when the colour is missing or out of range.
    pub fn controls(&self, patch: &Patch) -> Vec<&'static Control>;
    /// Value name of `kind`, e.g. "CRUNCH".
    pub fn type_label(&self, patch: &Patch) -> Option<&'static str>;
}

pub const CHAIN: &[BlockDef];
pub const AMP_STRIP: &[Control];
```

Contents (verified parameters only):

| id | title | switch | kind | color | common | per_color |
|---|---|---|---|---|---|---|
| booster | BOOSTER | booster-sw | — | booster-color | — | `[booster1-drive "DRIVE"]`, `[booster2-drive "DRIVE"]`, `[booster3-drive "DRIVE"]` |
| mod | MOD | mod-sw | — | — | — | — |
| amp | AMP | — | amp-type | — | amp-type "TYPE", cab-resonance "CAB RESONANCE", contour "CONTOUR", solo-sw "SOLO" | — |
| fx | FX | fx-sw | — | — | — | — |
| delay | DELAY | delay-sw | — | — | delay2-time "TIME (DELAY 2)", delay2-feedback "FEEDBACK (DELAY 2)", delay2-level "EFFECT LEVEL (DELAY 2)" | — |
| reverb | REVERB | reverb-sw | — | — | reverb1-level "EFFECT LEVEL (REVERB 1)" | — |

`AMP_STRIP`: gain "GAIN", volume "VOLUME", bass "BASS", middle "MIDDLE", treble "TREBLE", presence "PRESENCE".

- [ ] **Step 1: Write the failing tests** (in `layout.rs`)

```rust
#[test]
fn every_control_names_a_verified_param() {
    let all = CHAIN.iter().flat_map(|b| b.common.iter().chain(b.per_color.iter().flat_map(|c| c.iter())));
    for c in all.chain(AMP_STRIP) {
        assert!(find_param(c.param).is_some(), "{} not in PARAMS", c.param);
    }
    for b in CHAIN {
        for p in [b.switch, b.kind, b.color].into_iter().flatten() {
            assert!(find_param(p).is_some(), "{p} not in PARAMS");
        }
    }
}

#[test]
fn per_color_has_one_entry_per_colour_value() {
    for b in CHAIN.iter().filter(|b| b.color.is_some()) {
        let n = find_param(b.color.unwrap()).unwrap().values.unwrap().len();
        assert_eq!(b.per_color.len(), n, "{}", b.id);
    }
}

#[test]
fn booster_controls_follow_colour() {
    let booster = CHAIN.iter().find(|b| b.id == "booster").unwrap();
    let mut p = Patch::default_preset();
    p.set(find_param("booster-color").unwrap(), 2).unwrap();
    assert_eq!(booster.controls(&p)[0].param, "booster3-drive");
}

#[test]
fn amp_type_label_comes_from_value_names() {
    let amp = CHAIN.iter().find(|b| b.id == "amp").unwrap();
    assert_eq!(amp.type_label(&Patch::default_preset()), Some("CLEAN"));
}

#[test]
fn block_ids_are_unique() { /* collect ids, sort, dedup, compare lengths */ }
```

- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p katana-proto layout`. Expected: compile error (no `layout` module).
- [ ] **Step 3: Implement `layout.rs`** with the table above.
- [ ] **Step 4: Run them and check they pass.** Run `cargo test -p katana-proto`. Expected: all pass.
- [ ] **Step 5: Commit**

```bash
git add katana-proto/src/layout.rs katana-proto/src/lib.rs
git commit -m "feat(proto): block layout description for the editor"
```

---

### Task 3: `katana-gui` crate, theme, editor state, and the empty shell

**Files:**
- Modify: `Cargo.toml` (workspace members add `"katana-gui"`)
- Create: `katana-gui/Cargo.toml`, `katana-gui/src/lib.rs`, `katana-gui/src/main.rs`, `katana-gui/src/app.rs`, `katana-gui/src/theme.rs`, `katana-gui/src/editor.rs`, `katana-gui/src/views/mod.rs`, `katana-gui/src/views/top_bar.rs`, `katana-gui/src/views/status.rs`, `katana-gui/src/views/library.rs`
- Test: `katana-gui/tests/shell.rs`; unit tests in `editor.rs`
- Modify: `docs/superpowers/specs/2026-10-22-katana-gui-design.md` (Constraints: add "Development also happens on macOS; code stays portable, Linux remains the only target.")

**Dependencies:** `katana-proto = { path = "../katana-proto" }`, `eframe = "0.37"`, `rfd = "0.17"`. Dev: `egui_kittest = "0.37"`. For `rfd` on Linux, use the xdg-portal backend instead of GTK (no GTK dev package needed). Check the exact feature names with `cargo info rfd`, and keep macOS building.

**Interfaces:**
- Produces:

```rust
// editor.rs
pub struct Editor { /* patch: Patch, baseline: Patch, path: Option<PathBuf> */ }
impl Editor {
    pub fn new() -> Editor;                                  // Patch::default_preset(), baseline = same, no path
    pub fn patch(&self) -> &Patch;
    pub fn get(&self, p: &ParamDef) -> Option<u32>;
    pub fn set(&mut self, p: &ParamDef, value: u32) -> Result<(), ProtoError>;
    pub fn is_dirty(&self) -> bool;                          // patch != baseline
    pub fn name(&self) -> String;                            // patch.name(), or "(unnamed)" if empty
    pub fn path(&self) -> Option<&Path>;
    pub fn open_bytes(&mut self, bytes: &[u8], path: PathBuf) -> Result<(), ProtoError>; // Patch::from_syx; on Err nothing changes
    pub fn saved_to(&mut self, path: PathBuf) -> Vec<u8>;   // returns patch.to_syx(), sets baseline = patch and path; the caller writes the bytes and calls this only after the write succeeded
}

// theme.rs
pub struct Palette { pub bg: Color32, pub panel: Color32, pub line: Color32, pub text: Color32,
                     pub dim: Color32, pub accent: Color32, pub green: Color32, pub red: Color32, pub orange: Color32 }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum ThemeId { Dark, Light }
impl ThemeId { pub const ALL: [ThemeId; 2]; pub fn name(self) -> &'static str; pub fn palette(self) -> Palette; }
pub fn apply(ctx: &egui::Context, p: &Palette);             // sets egui Visuals (dark_mode, panel/window fill, strokes, selection = accent)

// app.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Tab { Editor, Library }
pub struct KatanaApp { pub editor: Editor, pub tab: Tab, pub theme: ThemeId, pub selected_block: &'static str, pub error: Option<String>, /* private: pending, close_confirmed */ }
impl KatanaApp {
    pub fn new() -> KatanaApp;                               // Editor::new(), Tab::Editor, Dark, selected_block "booster"
    pub fn show(&mut self, ctx: &egui::Context);             // whole UI; eframe::App's required method only delegates here
}
```

The Light palette is the builder's choice, but it must be readable: text on bg has a contrast ratio of at least 4.5.

Shell content (Task 5 fills the Editor tab, Task 6 wires the file buttons):
- **Top bar** (`TopBottomPanel::top`, mockup order): `● not connected` (dim); channel tabs PANEL A1 A2 A3 A4 B1 B2 B3 B4, all disabled, tooltip "Needs amp control (stage 1)"; flexible space; preset name; `● unsaved` in accent when `is_dirty()`; a disabled **Live edit** toggle (tooltip "Needs amp control (stage 1)"); buttons **Open .syx…**, **Save .syx…** (enabled; no-ops until Task 6); **Quick save**, **Save**, **Save as…** disabled (tooltip "Preset library (stage 3)"); **Load to amp**, **Store to channel…** disabled (tooltip "Needs amp control (stage 1)").
- **Tab strip:** Editor | Library (Tone Central is left out until stage 4).
- **Status line** (`TopBottomPanel::bottom`): `Amp: not connected · Live edit off · last auto-save — · panel change —`, then `self.error` in red if set, and on the right a theme combo box listing `ThemeId::ALL` by name.
- **Library tab:** a centred label "Preset library comes in stage 3."
- `main.rs`: `eframe::run_native("KATANA Tone Studio", …)` with initial inner size 1600×960 and the title "KATANA Tone Studio".

- [ ] **Step 1: Write the failing tests**

```rust
// editor.rs
#[test] fn new_editor_is_clean_default() { let e = Editor::new(); assert!(!e.is_dirty()); assert_eq!(e.name(), "DEFAULT"); }
#[test] fn set_marks_dirty_and_setting_back_clears_it() {
    let mut e = Editor::new(); let g = find_param("gain").unwrap();
    e.set(g, 5).unwrap(); assert!(e.is_dirty()); e.set(g, 0).unwrap(); assert!(!e.is_dirty());
}
#[test] fn open_rejects_non_patch_keeps_editor() {
    let mut e = Editor::new(); e.set(find_param("gain").unwrap(), 5).unwrap();
    assert!(e.open_bytes(b"not sysex", "x.syx".into()).is_err());
    assert_eq!(e.get(find_param("gain").unwrap()), Some(5)); assert!(e.is_dirty()); assert!(e.path().is_none());
}
#[test] fn open_then_saved_round_trip() {
    let mut src = Patch::default_preset(); src.set(find_param("gain").unwrap(), 60).unwrap();
    let mut e = Editor::new(); e.open_bytes(&src.to_syx(), "a.syx".into()).unwrap();
    assert!(!e.is_dirty()); assert_eq!(e.get(find_param("gain").unwrap()), Some(60));
    e.set(find_param("gain").unwrap(), 61).unwrap();
    let bytes = e.saved_to("b.syx".into()); assert!(!e.is_dirty());
    assert_eq!(Patch::from_syx(&bytes).unwrap().get(find_param("gain").unwrap()), Some(61));
}

// tests/shell.rs (egui_kittest Harness::new_state(|ctx, app: &mut KatanaApp| app.show(ctx), KatanaApp::new()))
#[test] fn shell_shows_offline_state() {
    // get_by_label for "Editor", "Library", "● not connected", "DEFAULT", "Open .syx…"
    // query_by_label("● unsaved") is None
}
#[test] fn amp_actions_are_disabled() {
    // for "Store to channel…", "Load to amp", "Quick save", "Save", "Save as…", "A1", "PANEL": node.is_disabled()
}
#[test] fn library_tab_shows_placeholder() {
    // click "Library", run, get_by_label("Preset library comes in stage 3.")
}
#[test] fn theme_switch_changes_visuals() {
    // set app.theme = ThemeId::Light via the status-line combo (click combo, click "Light"), run; assert !harness.ctx.style().visuals.dark_mode
}
```

- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p katana-gui`. Expected: compile errors.
- [ ] **Step 3: Implement** the crate, the theme, `Editor`, the shell views and `main.rs` as specified. Edit the spec's Constraints line.
- [ ] **Step 4: Run them and check they pass.** Run `cargo test -p katana-gui && cargo clippy --workspace --all-targets`. Expected: pass, no warnings.
- [ ] **Step 5: Check it by eye.** Run `cargo run -p katana-gui`. Expected: a dark window matching the mockup's top bar, tabs and status line, with greyed amp buttons that show tooltips.
- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock katana-gui docs/superpowers/specs/2026-10-22-katana-gui-design.md
git commit -m "feat(gui): egui shell with top bar, tabs, status line and themes"
```

---

### Task 4: Knob widget and generic parameter control

**Files:**
- Create: `katana-gui/src/widgets/mod.rs`, `katana-gui/src/widgets/knob.rs`, `katana-gui/src/widgets/control.rs`
- Modify: `katana-gui/src/lib.rs`
- Test: unit tests in `knob.rs`; `katana-gui/tests/controls.rs`

**Interfaces:**
- Consumes: `Editor` (Task 3), `ParamDef` (proto), `Palette` (Task 3).
- Produces:

```rust
// knob.rs
pub const DRAG_PX_FULL_RANGE: f32 = 200.0;
pub fn drag_value(start: u32, dy: f32, min: u32, max: u32) -> u32; // dy > 0 = mouse moved down = smaller; result = clamp(start + round(-dy * (max-min) / DRAG_PX_FULL_RANGE))
pub fn wheel_value(v: u32, notches: i32, min: u32, max: u32) -> u32; // ±1 raw per notch, clamped
pub fn parse_typed(text: &str, min: u32, max: u32) -> Option<u32>; // trimmed decimal within range, else None
pub struct Knob<'a> { pub label: &'a str, pub value: &'a mut u32, pub min: u32, pub max: u32, pub palette: &'a Palette }
impl egui::Widget for Knob<'_> { … }  // ~48 px circle; arc from min to value in accent (fraction clamped to 0..=1); label above, value below

// control.rs
/// Draws the right widget for `def` and writes changes into the editor.
pub fn param_control(ui: &mut egui::Ui, editor: &mut Editor, def: &'static ParamDef, label: &str, palette: &Palette) -> egui::Response;
```

Knob behaviour: the drag start value is kept in `ui.memory` temp data under the widget id, so the value follows the total drag distance and doesn't build up rounding errors. While hovered, `smooth_scroll_delta.y` turns into notches. Double-click opens a `TextEdit` in place: Enter commits through `parse_typed` (a `None` result keeps the old value), Escape cancels. The knob reports `changed()` only when the value really changed. Each knob has an AccessKit label equal to its `label`, so tests can find it.

`param_control` dispatch: `values == Some([(0,"OFF"),(1,"ON")])` → a toggle button showing "ON"/"OFF". Other `Some(values)` → `ComboBox` with the value names; a raw value with no name shows `"? (n)"`. `None` → `Knob`. A missing value (`editor.get` is `None`) shows the label with "—" and no control. Every change goes through `editor.set`. The widgets only produce in-range values, so an `Err` here is a bug: use `debug_assert!`.

- [ ] **Step 1: Write the failing tests**

```rust
// knob.rs
#[test] fn drag_full_range_over_200px() { assert_eq!(drag_value(0, -200.0, 0, 100), 100); assert_eq!(drag_value(100, 200.0, 0, 100), 0); }
#[test] fn drag_small_moves_round() { assert_eq!(drag_value(50, -3.0, 0, 100), 52); }
#[test] fn drag_from_out_of_range_clamps() { assert_eq!(drag_value(130, -1.0, 0, 120), 120); }
#[test] fn wheel_steps_one_and_clamps() { assert_eq!(wheel_value(50, 1, 0, 100), 51); assert_eq!(wheel_value(0, -3, 0, 100), 0); }
#[test] fn parse_typed_accepts_in_range() { assert_eq!(parse_typed(" 42 ", 0, 100), Some(42)); }
#[test] fn parse_typed_rejects_bad_input() {
    assert_eq!(parse_typed("abc", 0, 100), None); assert_eq!(parse_typed("101", 0, 100), None);
    assert_eq!(parse_typed("-1", 0, 100), None); assert_eq!(parse_typed("", 0, 100), None);
}

// tests/controls.rs (harness over a small UI that draws param_control for gain, booster-sw and amp-type on an Editor in state)
#[test] fn toggle_flips_switch() { /* click "OFF" under booster-sw → editor.get(booster-sw) == Some(1), is_dirty */ }
#[test] fn combo_selects_named_value() { /* open amp-type combo, click "CRUNCH" → Some(3) */ }
#[test] fn knob_drag_up_increases_gain() { /* harness drag on node "GAIN" upward by 20 px → Some(10) */ }
#[test] fn knob_double_click_types_value() { /* double-click "GAIN", type "77", press Enter → Some(77) */ }
#[test] fn knob_typed_garbage_keeps_value() { /* double-click, type "x", Enter → Some(0), !is_dirty */ }
```

- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p katana-gui`. Expected: compile errors.
- [ ] **Step 3: Implement** `knob.rs` and `control.rs` as specified.
- [ ] **Step 4: Run them and check they pass.** Run `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Commit**

```bash
git add katana-gui
git commit -m "feat(gui): knob widget and generic parameter controls"
```

---

### Task 5: Editor tab: signal chain, detail panel, amp strip

**Files:**
- Create: `katana-gui/src/views/chain.rs`, `katana-gui/src/views/detail.rs`, `katana-gui/src/views/amp_strip.rs`
- Modify: `katana-gui/src/views/mod.rs`, `katana-gui/src/app.rs`
- Test: `katana-gui/tests/editor_tab.rs`

**Interfaces:**
- Consumes: `layout::{CHAIN, AMP_STRIP, BlockDef}` (Task 2), `param_control` (Task 4), `KatanaApp.selected_block` (Task 3).
- Produces: `views::editor_tab(ui: &mut egui::Ui, app: &mut KatanaApp, palette: &Palette)`, called from `KatanaApp::show` when `tab == Tab::Editor`. It is wrapped in a vertical `ScrollArea`, so smaller windows scroll.

Layout (mockup B, top to bottom):
- **SIGNAL CHAIN** section: `🎸 →` then one card per `CHAIN` entry separated by `→`, then `→ 🔊`. A card shows the title (bold); an ON/OFF pill (green fill when on, grey when off; clicking toggles `switch`; no pill when `switch` is `None`); `type_label` in dim text if any; and, when `color` is set, three LED dots in green/red/orange (palette) with the current one at full opacity and the others at 25%. Clicking a dot sets the colour. Clicking elsewhere on the card sets `selected_block`. The selected card gets an accent border.
- **Detail** section: the heading is `TITLE`, plus ` · <COLOUR NAME> variation` when the block has a colour, plus ` · type <type_label>` when it has one. Then `param_control` for each `controls(patch)` entry, laid out horizontally with wrapping. When there are no controls, it shows: "No parameters verified on the amp for this block yet."
- **AMP** strip: section titled "AMP" with `param_control` for each `AMP_STRIP` entry, always visible.

- [ ] **Step 1: Write the failing tests**

```rust
// tests/editor_tab.rs, harness over KatanaApp::show
#[test] fn chain_shows_blocks_in_order() { /* get_by_label for BOOSTER, MOD, AMP, FX, DELAY, REVERB; AMP card shows "CLEAN" */ }
#[test] fn booster_selected_by_default_shows_drive() { /* heading "BOOSTER · GREEN variation" exists; "DRIVE" knob exists */ }
#[test] fn clicking_led_switches_variation() {
    // click the ORANGE LED on BOOSTER (give each LED an AccessKit label "BOOSTER ORANGE"), run
    // app.editor.get(booster-color) == Some(2); heading "BOOSTER · ORANGE variation"
}
#[test] fn selecting_block_without_params_says_so() { /* click "MOD" card → "No parameters verified on the amp for this block yet." */ }
#[test] fn amp_strip_always_visible() { /* select DELAY; GAIN … PRESENCE still present */ }
#[test] fn pill_toggles_switch() { /* click REVERB's pill (label "REVERB OFF") → reverb-sw == Some(1), "● unsaved" visible */ }
#[test] fn out_of_range_value_renders() {
    // app.editor.open_bytes of a patch whose booster1-drive byte is forced to 127 (mem.apply directly); run; no panic; "127" label shown
}
#[test] fn editor_renders_in_small_window() {
    // Harness builder with_size(1280×720); run; "PRESENCE" node exists (reachable through the scroll area)
}
```

- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p katana-gui editor_tab`. Expected: compile errors or missing nodes.
- [ ] **Step 3: Implement** `chain.rs`, `detail.rs`, `amp_strip.rs` and the `editor_tab` wiring.
- [ ] **Step 4: Run them and check they pass.** Run `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Check it by eye.** Run `cargo run -p katana-gui`, then click through the blocks and turn knobs. Expected: it looks like mockup B, and `● unsaved` appears after an edit and disappears when the value is set back.
- [ ] **Step 6: Commit**

```bash
git add katana-gui
git commit -m "feat(gui): editor tab with signal chain, detail panel and amp strip"
```

---

### Task 6: Open/Save `.syx` and the unsaved-changes guard

**Files:**
- Modify: `katana-gui/src/app.rs`, `katana-gui/src/views/top_bar.rs`
- Test: `katana-gui/tests/files.rs`

**Interfaces:**
- Consumes: `Editor::open_bytes`, `Editor::saved_to` (Task 3).
- Produces:

```rust
impl KatanaApp {
    /// Opens now if the editor is clean; otherwise stores the request and shows the discard prompt.
    pub fn request_open(&mut self, path: PathBuf);
    /// Writes the editor to `path`; on success clears dirty; on failure sets `error`.
    pub fn save_to(&mut self, path: PathBuf);
    /// Called when the window asks to close. true = close now; false = prompt shown, close cancelled.
    pub fn on_close_requested(&mut self) -> bool;
}
```

Behaviour:
- **Open .syx…**: `rfd::FileDialog::new().add_filter("SysEx", &["syx"]).pick_file()`, then `request_open`. Reading the file or `open_bytes` failing sets `error = "Couldn't open <file name>: <error>"` and leaves the editor unchanged. A successful open clears `error` and keeps `selected_block`.
- **Save .syx…**: `save_file()` with the default file name `<name>.syx` (characters other than letters, digits, `-` and `_` become `_`), then `save_to`. Write with `std::fs::write`, and call `editor.saved_to` only after the write succeeds. On failure, set `error = "Couldn't save <file name>: <error>"` and leave the editor dirty. Never show a "saved" message for a write that failed.
- **Discard prompt** (`egui::Modal`): "Discard unsaved changes to <name>?" with **Discard** and **Cancel** buttons. Discard carries out the pending Open or Close; Cancel drops it and leaves the editor untouched.
- **Close:** in `show`, when `ctx.input(|i| i.viewport().close_requested())` and `on_close_requested()` returns false, send `ViewportCommand::CancelClose`. After Discard on a pending close, mark it confirmed and send `ViewportCommand::Close`.

- [ ] **Step 1: Write the failing tests** (use a `tempfile`-style directory under `std::env::temp_dir()` with a unique name, or add `tempfile` as a dev-dependency)

```rust
#[test] fn open_valid_file_loads_and_is_clean() { /* write Patch with gain 60 to a.syx; request_open; gain == 60; !dirty; error None */ }
#[test] fn open_bad_file_shows_error() { /* write b"junk" to bad.syx; request_open; editor unchanged; error starts with "Couldn't open bad.syx" ; harness shows it */ }
#[test] fn open_while_dirty_asks_first() {
    // set gain 5; request_open(a.syx); run; "Discard unsaved changes to DEFAULT?" visible; gain still 5
    // click "Cancel" → gain 5, prompt gone; request_open again, click "Discard" → gain 60, !dirty
}
#[test] fn save_writes_and_clears_dirty() { /* set gain 7; save_to(out.syx); Patch::from_syx(read(out.syx)).get(gain) == 7; !dirty */ }
#[test] fn save_failure_keeps_dirty() { /* set gain 7; save_to(dir_that_does_not_exist/out.syx); is_dirty; error starts with "Couldn't save" */ }
#[test] fn close_while_dirty_is_cancelled() { /* set gain 5; assert!(!app.on_close_requested()); prompt visible; clean app: assert!(on_close_requested()) */ }
```

- [ ] **Step 2: Run them and check they fail.** Run `cargo test -p katana-gui files`. Expected: compile errors.
- [ ] **Step 3: Implement** the methods, the dialogs and the modal.
- [ ] **Step 4: Run them and check they pass.** Run `cargo test --workspace && cargo clippy --workspace --all-targets`. Expected: everything passes, no warnings.
- [ ] **Step 5: Check it by hand** with a real dump. Run `cargo run -p katana-gui`, open `backups/slot5-A-CH4.syx`, and check that the name reads "Bloom Crunch" and the values look plausible. Change gain, try Open again (expect the prompt), save to the scratch directory, reopen the saved file and check that gain kept the change. Close with unsaved changes (expect the prompt).
- [ ] **Step 6: Commit**

```bash
git add katana-gui
git commit -m "feat(gui): open and save .syx with unsaved-changes guard"
```

---

### Task 7: Check on Linux

No code unless something breaks. Run on the user's Linux machine (the target).

- [x] **Step 1:** Run `cargo test --workspace` on Linux. Expected: everything passes.
- [x] **Step 2:** Run `cargo run -p katana-gui --release` on the 1920×1080 screen with the window maximised. Expected: the whole Editor tab (chain, detail, amp strip) is visible with no scrollbar. File dialogs open through the portal. The dark theme matches the mockup.
- [x] **Step 3:** Repeat Task 6 Step 5 on Linux.
- [x] **Step 4:** Note what was confirmed and where in the spec's stage list ("Stage 2 shell: verified on Linux <date>"), then commit:

```bash
git add docs/superpowers/specs/2026-10-22-katana-gui-design.md
git commit -m "docs: GUI shell verified on Linux"
```

## Deferred (decided here, not in this plan)

- Reopening the last edited preset at app start: this needs auto-save and the library (stage 3). For now the app starts with the default preset.
- Chain order from the patch: comes once the chain parameter is verified (stage 2 coverage work).
- All the remaining ~350 parameters, effect types and colours: added to `PARAMS`/`layout.rs` as they are verified on the amp.
- Remembering the chosen theme between runs.
