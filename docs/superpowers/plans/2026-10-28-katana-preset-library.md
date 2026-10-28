# KATANA Preset Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the stage 3 preset library. It covers Tone Exchange `.tsl` import, My patches with a Quick saves group, Save, Save as… and Quick save tied to the file the editor came from, read-only, delete, search and sort, and a hidden auto-save that restores the last state at start. Everything works offline.

**Architecture:**
- **`katana-proto`:**
  - Patch blocks get names.
  - A new `tsl` module parses Tone Exchange sets into `Patch`es.
  - `Patch::set_name` writes the inner name.
- **`katana-gui`:**
  - A UI-free `library` module owns the folder: listing, index, import, pick, save, delete, names, search and sort, auto-save files.
  - `Editor` records where its patch came from (`Origin`).
  - `KatanaApp` holds a `Library` and wires the actions.
  - The Library tab draws layout C.
  - Auto-save runs from the frame loop and restores at start.

**Tech Stack:** Rust 2021, eframe/egui 0.37, egui_kittest 0.37, rfd 0.17. New dependencies:
- `serde_json = "1"` (in `katana-proto` and `katana-gui`);
- `serde = { version = "1", features = ["derive"] }` (in `katana-gui`);
- `directories = "6"`;
- `chrono = { version = "0.4", default-features = false, features = ["clock"] }`.

**Spec:** `docs/superpowers/specs/2026-10-28-katana-preset-library-design.md` (approved). Parent: `docs/superpowers/specs/2026-10-22-katana-gui-design.md`.

## Global Constraints

- Linux is the only target; development also happens on macOS. Portable code only: find the library folder with `directories`, never by hard-coding XDG paths.
- No amp I/O. Load to amp, Store to channel…, Live edit and the channel tabs stay disabled.
- Patches on disk are `.syx` in the `katana-cli dump` format (`Patch::to_syx` / `Patch::from_syx`).
- A downloaded `.tsl` is never changed. Nothing is deleted without asking. Broken files or index never stop the app.
- Library folder: `ProjectDirs::from("", "", "katana-ts").data_dir()`. On Linux that's `~/.local/share/katana-ts/`. It has the subfolders `patches/`, `quick-saves/`, `sets/` and `autosave/`, plus `index.json`.
- Names:
  - The library, search, sort, the delete question and the top bar show the **file name without extension**.
  - Save as… also writes the typed name into the patch, cut to 16 ASCII characters.
  - File names keep spaces and parentheses. Only `/ \ : * ? " < > |` and control characters become `_`.
  - Numbered names are `Name (2).syx`, `Name (3).syx`, and "taken" ignores letter case.
- Quick-save names are `quick-save_YYYY-MM-DD_HH-MM-SS.syx`, in local time.
- Auto-save:
  - The last 10 are kept as `autosave/autosave_YYYY-MM-DD_HH-MM-SS.syx`.
  - It runs every 300 s if the editor changed, plus on close and after every Save, Save as… and Quick save.
  - After Discard at close, the saved state is the origin file's content.
  - Start reopens it, with its origin.
- egui's default fonts have no "●", "→", "🔒" or emoji. Use words or painted shapes. Every widget needs an accessible name.
- `cargo fmt` (max_width 120). No new clippy warnings.
- Tests never touch the real library folder: they use `tmp(...)` folders and `Library::open(root)`.
- The real Tone Exchange samples in `samples/` are git-ignored, so tests build their own `.tsl` JSON. One test also reads `samples/*.tsl` and returns early when there are none.
- Work on branch `feat/preset-library`. Check the UI in the Linux VM with `scripts/vm.sh run` and `scripts/vm.sh shot`.

## Review Focus

1. Picking a set patch or opening a patch while the editor has unsaved changes. The discard prompt must come first, and no `.syx` may be created if the user cancels (Task 5 test).
2. Saving after a restart. The restored auto-save must point at the right origin, and Save must not overwrite newer work with older (Task 7 tests).
3. A `.tsl` with lowercase hex, a `memo` object, a wrong `device`, or one bad patch among good ones. The rules: import the set, refuse only that patch, and refuse a wrong device at import (Task 2 and Task 4 tests).
4. Two quick saves in the same second, or Save as… with a name that differs only in case from an existing one. Both must get distinct files and lose nothing (Task 3 and Task 4 tests).
5. A broken `index.json` or an unreadable `.syx` in `patches/`. The app must still start, list everything else, and name the skipped item (Task 3 test).

---

### Task 1: Named patch blocks and `Patch::set_name`

**Files:**
- Modify: `katana-proto/src/device.rs`, `katana-proto/src/patch.rs`

**Interfaces:**
- Produces: `pub const PATCH_BLOCK_NAMES: [&str; 80]` in `device.rs`. It has the same order and length as `PATCH_BLOCKS`, with names as in the existing comments (`"COM"`, `"OTHER"`, … `"PATCH_KNOB_SOLO_DELAY_READONLY"`).
- Produces: `impl Patch { pub fn set_name(&mut self, name: &str) }`. It writes 16 bytes at the COM address: printable ASCII kept, anything else becomes `?`, cut to 16, space-padded.

- [ ] **Step 1: Write the failing tests** in `device.rs` / `patch.rs` test modules

```rust
#[test]
fn block_names_line_up_with_blocks() {
    assert_eq!(PATCH_BLOCK_NAMES.len(), PATCH_BLOCKS.len());
    assert_eq!(PATCH_BLOCK_NAMES[0], "COM");
    assert_eq!(PATCH_BLOCK_NAMES[3], "AMP");
    assert_eq!(PATCH_BLOCK_NAMES[79], "PATCH_KNOB_SOLO_DELAY_READONLY");
    let mut sorted = PATCH_BLOCK_NAMES.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 80, "names are unique");
}

#[test]
fn set_name_pads_cuts_and_masks() {
    let mut p = Patch::default_preset();
    p.set_name("Iron Man");
    assert_eq!(p.name(), "Iron Man");
    p.set_name("A very long patch name");
    assert_eq!(p.name(), "A very long patc");
    p.set_name("Café");
    assert_eq!(p.name(), "Caf?");
}
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-proto names`. Expected: FAIL to compile.
- [ ] **Step 3: Implement both.** Keep the existing comments on `PATCH_BLOCKS` or drop them; either is fine.
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-proto`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(proto): named patch blocks and Patch::set_name`.

---

### Task 2: Parse Tone Exchange `.tsl` sets

**Files:**
- Create: `katana-proto/src/tsl.rs` (`pub mod tsl;` in `lib.rs`)
- Modify: `katana-proto/Cargo.toml` (add `serde_json = "1"`)
- Test: unit tests in `tsl.rs`; `katana-proto/tests/tsl_samples.rs`

**Interfaces:**
- Produces:

```rust
pub struct ToneSet { pub name: String, pub patches: Vec<SetPatch> }
pub struct SetPatch { pub name: String, pub patch: Result<Patch, TslError> }
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TslError { /* NotJson(String), WrongDevice(String), Malformed(String),
                       UnknownBlock(String), MissingBlock(&'static str),
                       BlockSize { block: String, expected: u32, got: usize },
                       BadByte { block: String, value: String } */ }
impl ToneSet { pub fn parse(bytes: &[u8]) -> Result<ToneSet, TslError>; }
```

- Rules (from the spec, observed in the samples):
  - **Device:** `device` must be `"KATANA Gen3"`, else `WrongDevice`.
  - **Patches:** they are read from `data[0]`; a missing or empty list is `Malformed`.
  - **Blocks:** each patch's `paramSet` maps `"PATCH%<NAME>"` to its block in `PATCH_BLOCK_NAMES`. Every one of the 80 blocks is required (`MissingBlock`), an unknown key is `UnknownBlock`, and each size must match (`BlockSize`).
  - **Bytes:** values are hex strings of either case. Each must be ≤ 0x7F, else `BadByte`.
  - **Patch name:** comes from `PATCH%COM`, trimmed like `Patch::name`. If COM itself is unusable, the name is `Patch <n>` (1-based).
  - **Memo:** `memo` may be a string or an object; it is ignored.
  - **Bad patches:** a bad patch gives `SetPatch { patch: Err(..) }`; the set still parses.

- [ ] **Step 1: Write the failing tests** in `tsl.rs`. A test helper builds JSON from `Patch`es:

```rust
/// A .tsl text holding `patches`, built from their bytes and PATCH_BLOCK_NAMES.
fn tsl(set: &str, device: &str, patches: &[Patch]) -> String { /* serde_json::json! */ }

#[test]
fn parses_set_and_patches() {
    let mut a = Patch::default_preset();
    a.set_name("Iron Man");
    a.set(crate::find_param("gain").unwrap(), 70).unwrap();
    let mut b = Patch::default_preset();
    b.set_name("Paranoid");
    let set = ToneSet::parse(tsl("Black Sabbath", "KATANA Gen3", &[a.clone(), b]).as_bytes()).unwrap();
    assert_eq!(set.name, "Black Sabbath");
    assert_eq!(set.patches.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Iron Man", "Paranoid"]);
    assert_eq!(set.patches[0].patch.as_ref().unwrap(), &a);
}

#[test]
fn lowercase_hex_and_memo_object_are_fine() { /* lowercase every byte string, memo = {"memo":"","isToneCentralPatch":false} → same Patch */ }

#[test]
fn wrong_device_is_refused() {
    let e = ToneSet::parse(tsl("X", "KATANA MkII", &[Patch::default_preset()]).as_bytes()).unwrap_err();
    assert_eq!(e, TslError::WrongDevice("KATANA MkII".into()));
}

#[test]
fn one_bad_patch_keeps_the_rest() { /* 2nd patch: drop "PATCH%AMP" → patches[1].patch == Err(MissingBlock("AMP")), patches[0] Ok, names both present */ }

#[test]
fn wrong_block_size_and_byte_are_refused() { /* AMP with 9 bytes → BlockSize; a byte "80" → BadByte */ }

#[test]
fn not_json_is_refused() { assert!(matches!(ToneSet::parse(b"junk"), Err(TslError::NotJson(_)))); }
```

`katana-proto/tests/tsl_samples.rs` checks the real downloads when they exist:

```rust
#[test]
fn real_samples_parse_completely() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "tsl")) {
        let set = katana_proto::tsl::ToneSet::parse(&std::fs::read(e.path()).unwrap()).unwrap();
        assert!(!set.patches.is_empty());
        for p in &set.patches {
            assert!(p.patch.is_ok(), "{}: {}: {:?}", e.path().display(), p.name, p.patch.as_ref().err());
        }
    }
}
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-proto tsl`. Expected: FAIL to compile (no `tsl` module).
- [ ] **Step 3: Implement** `ToneSet::parse` with `serde_json::Value`.
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-proto`. Expected: all pass, including `real_samples_parse_completely` on this Mac, where `samples/` holds 5 sets (21 patches).
- [ ] **Step 5: Commit** `feat(proto): parse Tone Exchange .tsl sets`.

---

### Task 3: Library folder, listing, index, names, search and sort

**Files:**
- Create: `katana-gui/src/library.rs` (`pub mod library;` in `lib.rs`)
- Modify: `katana-gui/Cargo.toml` (`serde`, `serde_json`, `directories`, `chrono`)
- Test: `katana-gui/tests/library.rs`

**Interfaces:**
- Produces, in `library.rs`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Patch, QuickSave }
#[derive(Clone, Debug)]
pub struct Entry { pub kind: Kind, pub path: PathBuf, pub name: String /* file stem */,
                   pub modified: SystemTime, pub read_only: bool }
#[derive(Clone, Debug)]
pub struct SetEntry { pub path: PathBuf, pub name: String /* set name */, pub modified: SystemTime,
                      pub patches: Vec<String> /* patch names */ }
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort { NewestFirst, OldestFirst, NameAz, NameZa }
pub struct Library { /* root, index, entries, sets */ pub problems: Vec<String> }
impl Library {
    pub fn default_root() -> Option<PathBuf>;           // ProjectDirs data_dir
    pub fn open(root: PathBuf) -> Library;               // creates subfolders, reads index, lists; never fails
    pub fn detached() -> Library;                        // no folder: lists nothing, every write fails
    pub fn root(&self) -> Option<&Path>;
    pub fn refresh(&mut self);
    pub fn patches(&self) -> &[Entry];                   // both kinds
    pub fn sets(&self) -> &[SetEntry];
    pub fn is_read_only(&self, path: &Path) -> bool;
    pub fn set_read_only(&mut self, path: &Path, on: bool) -> Result<(), String>;
}
pub fn safe_stem(name: &str) -> String;                  // replaces / \ : * ? " < > | and controls with _, trims; "" → "patch"
pub fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf; // "stem.ext", "stem (2).ext", … case-insensitive
pub fn matches(query: &str, name: &str) -> bool;         // empty query or any word contained, case-insensitive
pub fn sort_entries(v: &mut [Entry], sort: Sort);        // NameAz/Za case-insensitive
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()>; // moved from app.rs unchanged
```

- Index: `index.json` at the root, `{"read_only": ["patches/a.syx", …], "source": {"patches/a.syx": {"set": "…", "patch": "…"}}}`. Paths are relative to the root, and it's written with `write_atomic`.
  - A missing index means an empty one.
  - A broken index also means an empty one, plus a problem `"index.json is damaged; read-only flags reset"`. The damaged file is kept as `index.json.bad`.
- Listing:
  - `patches/*.syx` gives `Kind::Patch`, and `quick-saves/*.syx` gives `Kind::QuickSave`.
  - A `.syx` that fails `Patch::from_syx` is skipped with the problem `"Skipped <file>: <error>"`.
  - `sets/*.tsl` is parsed with `ToneSet::parse`; a failing set is skipped with a problem.
  - Dates come from the file's modified time.
- Search for sets: a set matches if its name or any patch name matches. The UI applies it.

- [ ] **Step 1: Write the failing tests** in `tests/library.rs` (with the same `tmp()` helper as `files.rs`)

```rust
#[test]
fn open_creates_folders_and_lists_both_kinds() { /* write patches/A.syx, quick-saves/quick-save_x.syx, sets/s.tsl
    (tsl text from a helper building JSON like Task 2's); open → patches() has A (Patch) and quick-save_x (QuickSave),
    sets() has one with its patch names; all four subfolders exist */ }

#[test]
fn broken_files_are_skipped_and_named() { /* patches/bad.syx = b"junk", sets/bad.tsl = b"{" → not listed;
    problems mention "bad.syx" and "bad.tsl"; good files still listed */ }

#[test]
fn broken_index_resets_read_only_and_says_so() { /* index.json = "{" → no read-only, problem contains "index.json",
    index.json.bad exists */ }

#[test]
fn read_only_flag_survives_reopen() { /* set_read_only(A, true); Library::open again → is_read_only(A) */ }

#[test]
fn free_path_numbers_case_insensitively() {
    // dir holds "Iron Man.syx" → "Iron Man (2).syx"; then also "iron man (2).SYX" → "Iron Man (3).syx"
}

#[test]
fn safe_stem_keeps_readable_names() {
    assert_eq!(safe_stem("Iron Man (live)"), "Iron Man (live)");
    assert_eq!(safe_stem("AC/DC: Back?"), "AC_DC_ Back_");
    assert_eq!(safe_stem("   "), "patch");
}

#[test]
fn search_matches_any_word() {
    assert!(matches("zeppelin black", "Black Dog"));
    assert!(matches("", "anything"));
    assert!(!matches("metallica", "Black Dog"));
}

#[test]
fn sort_orders() { /* NameAz case-insensitive ("b" after "A"), NewestFirst by modified */ }
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-gui --test library`. Expected: FAIL to compile.
- [ ] **Step 3: Implement.** `app.rs` now calls `library::write_atomic`; `files.rs` tests keep passing.
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(gui): library folder, listing, index, names, search, sort`.

---

### Task 4: Library actions: import, pick, save, quick save, delete

**Files:**
- Modify: `katana-gui/src/library.rs`
- Test: `katana-gui/tests/library.rs`

**Interfaces:**
- Produces, on `Library` (every write uses `write_atomic` and then `refresh()`, and returns the new path; errors are user-facing strings):

```rust
pub fn import(&mut self, from: &Path) -> Result<PathBuf, String>;
    // .syx (checked with Patch::from_syx) → patches/<stem>.syx (free_path)
    // .tsl (checked with ToneSet::parse; wrong device refused) → sets/<file name>.tsl (free_path), bytes unchanged
    // other extensions → Err("Can't import <file>: not a .syx or .tsl")
pub fn pick(&mut self, set: &Path, index: usize) -> Result<(PathBuf, Patch), String>;
    // patch Ok → patches/<safe_stem(patch name)>.syx (free_path); index source = {set name, patch name}
    // patch Err → Err("Can't use <patch name> from <set name>: <error>") and no file
pub fn save_as(&mut self, name: &str, patch: &Patch) -> Result<PathBuf, String>;
    // patches/<safe_stem(name)>.syx (free_path); writes a copy with set_name(name)
pub fn new_quick_save(&mut self, patch: &Patch, now: chrono::NaiveDateTime) -> Result<PathBuf, String>;
    // quick-saves/quick-save_YYYY-MM-DD_HH-MM-SS.syx (free_path)
pub fn overwrite(&mut self, path: &Path, patch: &Patch) -> Result<(), String>;
    // refuses a read-only path
pub fn delete(&mut self, path: &Path) -> Result<(), String>;
    // patch, quick save or set; refuses read-only; drops its index entries
pub fn load(&self, path: &Path) -> Result<Patch, String>;
pub fn kind_of(&self, path: &Path) -> Option<Kind>;   // by folder
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn import_syx_and_tsl_land_in_their_lists() { /* import a.syx from outside → patches has "a";
    import s.tsl → sets has it, bytes identical to the source; import twice → "a (2)" */ }

#[test]
fn import_refuses_wrong_device_and_other_files() { /* MkII .tsl → Err contains "KATANA Gen3"; x.txt → Err; nothing written */ }

#[test]
fn pick_creates_numbered_patch_and_records_source() {
    // set with "Iron Man"; pick twice → "Iron Man.syx", "Iron Man (2).syx"; load(..) equals the set's patch;
    // the set file is byte-identical afterwards
}

#[test]
fn pick_bad_patch_is_refused_without_a_file() { /* set whose 2nd patch lacks AMP → Err names the patch; patches/ unchanged */ }

#[test]
fn save_as_names_file_and_patch() {
    // save_as("Blues Lead", p) → patches/Blues Lead.syx whose Patch::name() == "Blues Lead";
    // save_as("blues lead", p) → "blues lead (2).syx"
}

#[test]
fn quick_saves_in_the_same_second_are_distinct() {
    // new_quick_save twice with the same now → two files, second ends " (2).syx"; both Kind::QuickSave
}

#[test]
fn read_only_blocks_overwrite_and_delete() { /* set_read_only → overwrite Err, delete Err, file intact; unlock → delete Ok */ }

#[test]
fn delete_set_keeps_picked_patches() { /* pick from set, delete set → set gone, picked patch still listed */ }
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-gui --test library`. Expected: FAIL to compile.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(gui): library import, pick, save, quick save, delete`.

---

### Task 5: Editor origin and the app's library actions

Wire the library into the app. The editor knows its origin, and the top bar's actions work.

**Files:**
- Modify: `katana-gui/src/editor.rs`, `katana-gui/src/app.rs`, `katana-gui/src/views/action_bar.rs`, `katana-gui/src/views/top_bar.rs`, `katana-gui/src/main.rs`
- Test: `katana-gui/tests/files.rs` (reworked), `katana-gui/tests/shell.rs`, `katana-gui/tests/editor_tab.rs` (`TOP_ITEMS`)

**Interfaces:**
- `editor.rs`:
  - Add `#[derive(Clone, Debug, PartialEq, Eq)] pub enum Origin { None, Patch(PathBuf), QuickSave(PathBuf) }`. `path: Option<PathBuf>` is replaced by `origin: Origin`.
  - `pub fn load(&mut self, patch: Patch, origin: Origin)` sets the baseline.
  - `pub fn origin(&self) -> &Origin`.
  - `pub fn saved_as(&mut self, patch: Patch, origin: Origin)` sets the baseline to the saved patch.
  - `pub fn display_name(&self) -> String`: the origin's file stem, else `name()`.
  - `open_bytes(bytes, path)` stays for tests and sets `Origin::Patch(path)`. `path()` returns the origin's path.
- `app.rs`:
  - `KatanaApp { pub library: Library, … }`.
  - `KatanaApp::new()` uses `Library::detached()`.
  - `pub fn with_library(library: Library) -> KatanaApp`.
  - `main.rs` uses `with_library(Library::open(Library::default_root()…))`, falling back to `detached()` with an error message.
  - Library problems at start go to `error`, joined with "; ".
  - New actions:

```rust
pub fn request_open(&mut self, path: PathBuf);        // load a library file into the editor (prompt if dirty); origin by kind_of
pub fn request_pick(&mut self, set: PathBuf, index: usize); // prompt if dirty, then Library::pick + load (origin Patch)
pub fn import(&mut self, from: PathBuf);              // Library::import; never touches the editor
pub fn export_to(&mut self, path: PathBuf);           // old save_to: adds .syx, write_atomic; editor state unchanged
pub fn quick_save(&mut self, now: chrono::NaiveDateTime); // origin QuickSave and not read-only → overwrite; else new_quick_save
pub fn save(&mut self);                               // origin path, not read-only → overwrite; Origin::None → opens the Save as prompt
pub fn save_as(&mut self, name: &str);                // Library::save_as, then editor.saved_as(.., Origin::Patch)
pub fn can_save(&self) -> bool;                       // false when the origin is read-only
```

  - `Pending` gains `Pick(PathBuf, usize)`. The library file is created only after the user confirms Discard.
  - A **Save as… prompt**: a `Modal` with a `TextEdit` labelled "Name", plus "Save" and "Cancel". Escape or a click outside cancels. It's prefilled with `display_name()`, and an empty name is refused.
- Action bar labels: **Import…** (rfd picker with filters `syx` and `tsl`), **Export .syx…**, then Quick save, Save and Save as… (now enabled; Save is disabled when `!can_save()`), then Load to amp and Store to channel… (still disabled). The top bar shows `display_name()`.

- [ ] **Step 1: Rework and add the tests**
  - In `files.rs`:
    - Its harness gains `fn lib_harness(root) -> Harness<KatanaApp>`, which uses `KatanaApp::with_library(Library::open(root))`.
    - The old `save_*` tests become `export_*` tests. Export writes the file, keeps the editor dirty, and leaves `origin` unchanged.
    - `save_adds_missing_extension` → `export_adds_missing_extension`. The atomic-save, symlink and failure tests now call `export_to`.
  - New tests:

```rust
#[test]
fn pick_while_dirty_asks_and_cancel_creates_nothing() {
    // library with a set "S" holding "Iron Man"; set gain 5; request_pick(S, 0); run → prompt;
    // Cancel → patches/ is empty, gain still 5; request_pick again, Discard → "Iron Man.syx" exists,
    // editor origin Patch(that), display_name "Iron Man", not dirty
}

#[test]
fn quick_save_from_default_creates_then_overwrites() {
    // quick_save(t1) → one file in quick-saves, origin QuickSave; set gain 9; quick_save(t2) → still one file, holds gain 9
}

#[test]
fn quick_save_from_named_patch_never_overwrites_it() {
    // open patches/A.syx, change gain, quick_save → A.syx unchanged, new quick save holds the change
}

#[test]
fn quick_save_from_locked_quick_save_makes_a_new_one() { /* lock it; quick_save → two quick saves */ }

#[test]
fn save_overwrites_origin_and_is_blocked_when_read_only() {
    // open A, gain 7, save → A holds 7, clean; lock A, gain 8 → can_save() false, Save button disabled
}

#[test]
fn save_without_origin_opens_name_prompt() {
    // default editor; click "Save" → prompt with "Name" field; type "Blues"; click "Save" in the prompt →
    // patches/Blues.syx, origin Patch, top bar shows "Blues", patch name "Blues"
}

#[test]
fn import_adds_to_library_without_touching_editor() { /* import a .tsl → sets has it; editor still DEFAULT, clean */ }
```

  - In `shell.rs`: the labels list uses "Import…" and "Export .syx…". In `amp_actions_are_disabled`, remove "Quick save", "Save" and "Save as…", since they are now enabled.
  - In `editor_tab.rs`: `TOP_ITEMS` gets the new labels.

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-gui`. Expected: FAIL to compile (`with_library`, `Origin` and the rest are missing).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-gui`. Expected: all pass, including `top_bar_fits_minimum_width` with the new labels.
- [ ] **Step 5: Commit** `feat(gui): editor origin, save, save as, quick save, import, export`.

---

### Task 6: Library tab (layout C)

**Files:**
- Modify: `katana-gui/src/views/library.rs` (replaces the placeholder), `katana-gui/src/app.rs` (UI state)
- Test: `katana-gui/tests/library_tab.rs`; `katana-gui/tests/shell.rs` (remove `library_tab_shows_placeholder`)

**Interfaces:**
- UI state on `KatanaApp`:
  - `pub search: String`;
  - `pub sort: library::Sort` (default `NewestFirst`);
  - a pending delete `Option<PathBuf>` with its own Modal "Delete <name>?" with "Delete" and "Cancel".
- Layout:
  - **Top bar:**
    - A search `TextEdit` labelled "Search".
    - A `ComboBox::from_label("Sort")` with "Newest first", "Oldest first", "Name A–Z" and "Name Z–A". Use a real en dash; the default fonts have it.
    - An "Import…" button.
  - **Left column, "MY PATCHES":** named patches, then a "QUICK SAVES" subheading with the quick saves, each filtered by `matches` and sorted. Each row has:
    - the name as a button (click: `request_open`);
    - a toggle labelled "read-only" with accessible name `"Lock <name>"`;
    - a "Delete" button with accessible name `"Delete <name>"`, disabled while read-only.
  - **Right column, "DOWNLOADED SETS":** each matching set as an `egui::CollapsingHeader` with the set name. Inside, each patch has a row with its name and an "Open" button with accessible name `"Open <patch name>"` (`request_pick`). There is also a "Delete set" button with accessible name `"Delete set <set name>"`.
  - **Empty lists** say "No patches yet." or "No downloaded sets yet. Import a .tsl from Tone Exchange."

- [ ] **Step 1: Write the failing tests** in `tests/library_tab.rs` (library in a `tmp` root, harness 1600×960)

```rust
#[test]
fn lists_my_patches_quick_saves_and_sets() { /* A.syx, a quick save, set "Black Sabbath" → labels "A", "QUICK SAVES",
    quick save name, "Black Sabbath" visible */ }

#[test]
fn clicking_patch_opens_it() { /* A.syx with gain 60 → click "A" → editor gain 60, top bar "A" */ }

#[test]
fn opening_set_patch_picks_it() { /* expand "Black Sabbath", click "Open Iron Man" → patches has "Iron Man", editor shows it */ }

#[test]
fn search_filters_both_lists() { /* type "iron" into "Search" → "A" gone, set still listed (patch name matches) */ }

#[test]
fn delete_asks_and_respects_read_only() {
    // click "Lock A" → "Delete A" disabled; unlock; "Delete A" → prompt "Delete A?"; Cancel → still there;
    // again → Delete → gone from list and disk
}

#[test]
fn empty_library_says_so() { /* "No patches yet.", "No downloaded sets yet. Import a .tsl from Tone Exchange." */ }
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-gui --test library_tab`. Expected: FAIL (the labels aren't found).
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Look at it on Linux.** Copy two real sets into the VM's library. For example:
  - `scripts/vm.sh ssh 'mkdir -p ~/.local/share/katana-ts/sets'`;
  - then `scp -P 2222 samples/Black\ Sabbath.tsl "samples/Led Zeppelin Collection.tsl" vboxuser@127.0.0.1:.local/share/katana-ts/sets/`.

  Then run `scripts/vm.sh run`, switch to the Library tab and take a screenshot with `scripts/vm.sh shot`. Selecting the tab needs the user, or start on it temporarily. Expected: two columns as in mockup C, with the sets listed by set name.
- [ ] **Step 6: Commit** `feat(gui): library tab`.

---

### Task 7: Auto-save and restore at start

**Files:**
- Modify: `katana-gui/src/library.rs`, `katana-gui/src/app.rs`, `katana-gui/src/main.rs`
- Test: `katana-gui/tests/autosave.rs`

**Interfaces:**
- On `Library`:
  - `pub fn autosave(&mut self, patch: &Patch, origin: &Origin, now: NaiveDateTime) -> Result<(), String>`. It writes `autosave/autosave_<ts>.syx` (via `free_path`), deletes the oldest beyond `AUTOSAVE_KEEP = 10`, and writes `autosave/state.json` = `{"latest": "<file>", "origin": null | {"kind": "patch" | "quick-save", "path": "<relative>"}}`.
  - `pub fn last_autosave(&self) -> Option<(Patch, Origin)>`. `None` if missing or broken. A broken state gets a problem.
- On `KatanaApp`:
  - `pub const AUTOSAVE_SECS: f64 = 300.0`.
  - `pub fn autosave_tick(&mut self, now_secs: f64, now: NaiveDateTime)` auto-saves if `now_secs` is at least 300 s past the last auto-save and the editor differs from what was last auto-saved. It is called from `show` with `i.time` and `chrono::Local::now().naive_local()`, and `show` calls `ctx.request_repaint_after` so the timer fires while idle.
  - Auto-save is also written after a successful `save`, `save_as` and `quick_save`, and when the window really closes:
    - when it's clean;
    - after Discard, with the **baseline** patch (the origin file's content), not the discarded edits.
  - `pub fn start(library: Library) -> KatanaApp` is `with_library` plus restoring `last_autosave()`:
    - The patch loads with its origin.
    - The baseline is the origin file's content when it exists, so the unsaved marker is right.
    - If the origin file is gone, the origin becomes `None`, with a problem `"<file> no longer exists"`.
  - `main.rs` uses `start`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn autosave_keeps_last_ten() { /* 12 autosaves with distinct times → 10 .syx files; last_autosave = the 12th */ }

#[test]
fn tick_saves_only_after_interval_and_when_changed() {
    // tick(0) with a change → 1 file; change again, tick(100) → still 1; tick(301) → 2; tick(700) without change → 2
}

#[test]
fn restart_restores_state_and_origin() {
    // open A, gain 33 (dirty); close (on_close_requested → true path writes autosave);
    // start(Library::open(same root)) → gain 33, origin Patch(A), dirty (A on disk has the old gain)
}

#[test]
fn save_then_quit_restores_saved_not_older() {
    // open A, gain 40, tick(0) (autosave gain 40), gain 41, save, quit within 300 s →
    // start → gain 41, clean (Save after restart writes 41, never 40)
}

#[test]
fn discard_at_close_restores_origin_content() {
    // open A (gain 10 on disk), gain 50, close → prompt → Discard → start → gain 10, clean
}

#[test]
fn restore_with_missing_origin_falls_back() { /* autosave origin A, delete A → start: origin None, error names A */ }
```

- [ ] **Step 2: Run them.** Run: `cargo test -p katana-gui --test autosave`. Expected: FAIL to compile.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them.** Run: `cargo test -p katana-gui`. Expected: all pass.
- [ ] **Step 5: Commit** `feat(gui): auto-save and restore at start`.

---

### Task 8: Wrap up

- [ ] **Step 1:** `cargo fmt --check && cargo clippy --workspace --all-targets`. Expected: only the 3 old warnings.
- [ ] **Step 2:** `cargo test --workspace` on the Mac and `scripts/vm.sh test`. Expected: all pass.
- [ ] **Step 3:** In the VM:
  - Import the five sample sets with Import….
  - Open a patch from each set and check the knobs show its values.
  - Quick save twice.
  - Save as… "Test".
  - Quit and start again.

  Expected: the same patch, name and unsaved state come back. This step needs the user's hands; ask for it.
- [ ] **Step 4:** README:
  - Replace "Open .syx… / Save .syx…" with Import… and Export .syx….
  - Describe the Library tab: two lists, sets, read-only, delete, search and sort.
  - Describe Quick save, Save and Save as… and the library folder `~/.local/share/katana-ts/` with its subfolders.
  - Describe auto-save, and that `katana-cli load ~/.local/share/katana-ts/patches/<name>.syx` works on library files.
- [ ] **Step 5:** Commit `docs: README for the preset library`.
