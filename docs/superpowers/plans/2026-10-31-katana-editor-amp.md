# Stage 2: Editor ↔ amp Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The GUI talks to the amp: connection status, Live edit (both directions), channel tabs, Load to amp, Open with Live on, Store to channel… with backups — never storing without an explicit click.

**Architecture:** One background worker thread in `katana-gui` owns the MIDI connection and runs one amp job at a time; the UI sends `Job`s and drains `Event`s every frame through `std::sync::mpsc` (`AmpLink`), and the worker wakes the UI with `Context::request_repaint`. The worker is a plain struct with synchronous `handle(job)` and `idle()` methods so tests drive it against `FakeAmp` without threads; the app is tested with a scripted `AmpLink::pair()` without a worker. Protocol pieces the worker needs (editor mode on/off, select by selector value, block reads, backups) move into or are exposed by `katana-proto`. A hardware check with the user, early, settles how panel changes are noticed.

**Tech Stack:** Rust 2021; `katana-proto`, `katana-gui` (eframe/egui 0.37, `egui_kittest`), `katana-cli`; new in the GUI: `ctrlc = { version = "3", features = ["termination"] }`.

**Spec:** `docs/superpowers/specs/2026-10-31-katana-editor-amp-design.md` (approved 2026-10-31). Background: `docs/superpowers/specs/2026-10-22-katana-gui-design.md` ("Editor ↔ amp model", "Channel tabs"), `docs/protocol.md` § "Amp control".

## Global Constraints

- **Nothing is stored into a channel without the user clicking Store** — not on Open, not on edits, not with Live on. Save (file) and Store (channel) never trigger each other.
- Store always asks first and backs up the target channel and the current sound to `<library root>/backups/` (the CLI's folder and names) before writing.
- Live edit is off at every app start; it keeps its position across a disconnect within a session.
- Before the amp's settings replace the editor (Live on, tab click, channel switch on the amp, reconnect with Live on), unsaved editor changes are quick-saved first.
- The UI thread never waits on MIDI. No polling of stored channels; the only timed amp traffic is the panel re-read of `device::PANEL_BLOCKS` (≈ once a second, Live on only) and the port-list check (≈ once a second, no amp traffic).
- Tests never touch the amp or the user's library (temp folders). Hardware: only scratch channels A3, A4, B3, B4, after backups; say what will be heard first; by-ear and final checks are the user's.
- egui default fonts: no "●", "→", "🔒" or emoji in UI text; every widget keeps an accessible name.
- Must still build on macOS. `cargo clippy --workspace --all-targets`: no new warnings (2 known `manual_is_multiple_of`, in `katana-proto/src/capture.rs` and `katana-cli/src/cmd/mod.rs`).
- Existing CLI commands and GUI behaviour without an amp stay as they are; existing tests pass (the `amp_actions_are_disabled` test is updated for "no amp connected", not removed).
- Branch `feature/editor-amp`; commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Amp unplugged in the middle of a job** (store, channel read, Live write): the app shows "not connected", disables amp actions, the editor keeps its content, a half-done store names its backups. Test in Task 3 (`unplug_mid_job_reports_disconnect`) and Task 7 (`store_failure_names_backups`).
2. **Knob dragged fast with Live on**: the amp gets the last value, not a backlog that keeps playing old values for seconds. Test in Task 3 (`coalesce_keeps_last_value_per_address`).
3. **Panel change arriving while the user edits the same parameter in the app** (worker shadow vs. pending app edits): an app edit the worker already sent is never reported back as a panel change. Test in Task 4 (`own_writes_are_not_panel_changes`).
4. **Library preset opened, then replaced by amp settings, then app closed and reopened**: the restart shows what was in the editor, not the library file. Test in Task 5 (`replaced_content_survives_restart`).
5. **Store while Live is on**: `store_patch` turns editor mode off at its end; Live must keep working afterwards (notifications, panel re-read). Test in Task 4 (`live_survives_store`).

---

### Task 1: Protocol groundwork for the worker

**Files:**
- Create: `katana-proto/src/backup.rs` (moved from `katana-cli/src/backup.rs`, minus `backup_dir`)
- Modify: `katana-proto/src/lib.rs`, `katana-proto/src/amp.rs`, `katana-proto/src/patch.rs`, `katana-proto/src/device.rs`, `katana-cli/src/backup.rs`, `katana-cli/src/cmd/store.rs`, `katana-cli/src/cmd/monitor.rs`, `katana-cli/src/main.rs`, `README.md` (monitor `--editor`)

**Interfaces:**
- Produces, `katana-proto`:
  - `backup::backup_name(stamp: &str, what: &str, patch_name: &str) -> String` (`stamp` already formatted `%Y-%m-%d_%H-%M-%S`; same output as today's CLI function), `backup::write_backup(dir: &Path, name: &str, p: &Patch) -> std::io::Result<PathBuf>` (never overwrites, `_2`, `_3`, …). The CLI keeps `backup_dir()` and thin wrappers; its existing backup tests move with the code.
  - `amp::set_editor_mode(t: &mut dyn Transport, on: bool) -> Result<(), ProtoError>`.
  - `amp::select(t: &mut dyn Transport, selector: u8, timeout: Duration) -> Result<(), ProtoError>` — today's `select_channel` body for any selector 0–8 (0 = PANEL), editor mode assumed on; `select_channel` calls it.
  - `patch::read_blocks(t, blocks: &[(u32, u32)], timeout) -> Result<Patch, ProtoError>` — today's `read_patch_blocks`, made `pub` and re-exported.
  - `device::PANEL_BLOCKS: &[(u32, u32)]` — current-patch blocks the panel can change: start with the blocks holding the parameters in `docs/protocol.md`'s panel table (AMP, SW, COLOR, BOOSTER(1–3), DELAY(2), REVERB(1), SOLO_COM, CONTOUR_COM, PATCH_KNOB_READONLY, the cabinet block). Doc comment: "starting set, settled by Task 2".
- Produces, `katana-cli`: `monitor --editor`: turns editor mode on, prints every message with `describe` as today plus `notification(..)` decoded, until Ctrl-C (via `ctrlc` flag, `recv` timeout 200 ms), then turns editor mode off and exits 0.

- [ ] **Step 1: Failing tests.** `amp.rs`: `select_reaches_panel` (FakeAmp in editor mode, `select(t, 0, T)` → selector reads 0); `set_editor_mode_toggles` (fake's `editor` true then false). `patch.rs`: `read_blocks_reads_only_given_blocks` (two blocks → `Patch.mem` holds exactly their bytes). `backup.rs`: the CLI's three backup tests, adapted to `stamp`. `device.rs`: `panel_blocks_are_patch_blocks` (every `PANEL_BLOCKS` entry is in `PATCH_BLOCKS`). `katana-cli main.rs` `cli_parses`: `["katana-cli","monitor","--editor"]` is Ok.
- [ ] **Step 2:** `cargo test -p katana-proto && cargo test -p katana-cli` → FAIL (missing items).
- [ ] **Step 3:** Implement; CLI `store` uses the moved functions (output unchanged).
- [ ] **Step 4:** `cargo test --workspace` → PASS; clippy → only the known warnings.
- [ ] **Step 5:** Commit `feat(proto): editor mode, select by selector, block reads, backups for the GUI`.

### Task 2: Hardware check with the user — panel reports, PANEL select (amp needed)

Tell the user first: "With editor mode on I'll log what the amp sends while you use the panel. Then the amp switches to PANEL and back to the channel it was on." Nothing is stored.

**Files:** Modify `katana-proto/tests/hardware.rs`, `katana-proto/src/device.rs`, `docs/protocol.md`

- [ ] **Step 1:** Run `katana-cli monitor --editor > backups/stage2/panel.log` (in the background) while the user turns every panel knob, presses every button (booster/mod/fx/delay/reverb on/off and colour, solo, contour, cabinet resonance, amp type) and switches channels A1→B4 and PANEL. Ask the user whether the panel behaved normally (sound, LEDs, channel switching) with editor mode on.
- [ ] **Step 2:** **If the panel did not behave normally: stop and tell the user** — the spec requires redesigning Live (Program Change select, selector reads) before the worker is built.
- [ ] **Step 3:** Hardware test `select_panel_and_back`: read selector `s`; editor mode on; `amp::select(t, 0, T)`; assert selector 0; `amp::select(t, s, T)`; editor mode off. Run with `--nocapture`; record whether PANEL can be selected.
- [ ] **Step 4:** From the log: list which panel actions arrive as DT1s (address, bytes) and which don't. Set `device::PANEL_BLOCKS` to the blocks of the **unreported** actions only (empty if all are reported); add `device::PANEL_SELECTABLE: bool` (Step 3 result). Evidence comments with date.
- [ ] **Step 5:** Document in `docs/protocol.md` § "Amp control": the reported/unreported list, PANEL select result, panel behaviour in editor mode.
- [ ] **Step 6:** `cargo test --workspace` → PASS. Commit `test(proto): panel reports and PANEL select on the amp`.

### Task 3: Amp worker — connection, jobs, coalescing

**Files:**
- Create: `katana-gui/src/amp/mod.rs`, `katana-gui/src/amp/worker.rs`, `katana-gui/src/amp/link.rs`
- Modify: `katana-gui/src/lib.rs` (`pub mod amp;`)

**Interfaces:**
- Produces (`katana_gui::amp`):
  - `pub enum Job { LiveOn, LiveOff, ReadSlot(u8), Select(u8), WritePatch(Patch), SetParam { addr: u32, data: Vec<u8> }, Peek(Channel), Store { ch: Channel, patch: Patch, backup_dir: PathBuf, stamp: String }, Shutdown }` (`u8` = selector 0–8).
  - `pub enum Event { Connected, Disconnected, AmpOn(u8), Slot(u8, Patch), Current(u8, Patch), PanelChange { addr: u32, data: Vec<u8> }, Peeked(Channel, Patch), Stored { ch: Channel, channel_backup: PathBuf, current_backup: PathBuf, solo_delay_stored: bool }, StoreFailed(String), Failed(String), Busy(Option<String>) }`.
  - `pub type Opener = Box<dyn FnMut() -> Result<Box<dyn Transport + Send>, ProtoError> + Send>;` `pub type PortCheck = Box<dyn FnMut() -> bool + Send>;`
  - `pub struct Worker` with `Worker::new(open: Opener, port_present: PortCheck) -> Worker`, `fn handle(&mut self, job: Job) -> Vec<Event>`, `fn idle(&mut self, now: Instant) -> Vec<Event>`, `fn run(self, jobs: Receiver<Job>, events: Sender<Event>, wake: Box<dyn Fn() + Send>)` (loop: drain jobs with `try_recv`, `coalesce`, `handle` each, else `idle`; send events, call `wake` after each batch; exits on `Shutdown` or a closed channel).
  - `pub fn coalesce(jobs: Vec<Job>) -> Vec<Job>` — a `SetParam` is dropped when a later `SetParam` to the same `addr` follows with only `SetParam`s between them.
  - `pub struct AmpLink` with `AmpLink::spawn(worker: Worker, wake: Box<dyn Fn() + Send>) -> AmpLink`, `fn send(&self, job: Job)`, `fn try_recv(&self) -> Option<Event>`, `fn shutdown(self)` (sends `Shutdown`, joins up to 2 s); `AmpLink::pair() -> (AmpLink, LinkEnd)` with `pub struct LinkEnd { pub jobs: Receiver<Job>, pub events: Sender<Event> }` for app tests.
- Behaviour:
  - Not connected: `idle` checks `port_present` at most once a second (`PORT_CHECK = 1 s`); when present, `open`, `set_editor_mode(false)` (clears a mode left on by a crash), read the selector → `Connected`, `AmpOn(sel)`.
  - Connected: `idle` checks `port_present` once a second; absent → drop the transport, `Disconnected`. A job failing with `ProtoError::Midi`/`Timeout` while the port is absent → `Disconnected`; while present → `Failed(<message>)`.
  - Jobs while not connected → `Failed("amp not connected")`.
  - `LiveOn`: editor mode on, read selector and current settings → `Current(sel, p)`; `LiveOff`: editor mode off. `ReadSlot(sel)`: `read_slot(sel + 1)` → `Slot(sel, p)`. `Select(sel)` (Live on only): `amp::select` then read current → `Current(sel, p)`. `WritePatch(p)`: `write_patch`, then read selector → `AmpOn(sel)`. `SetParam`: one DT1. Long jobs (`LiveOn`, `ReadSlot`, `Select`, `WritePatch`, `Peek`, `Store`) are wrapped in `Busy(Some("<what>…"))` / `Busy(None)`.
  - A `SharedFake(Arc<Mutex<FakeAmp>>)` implementing `Transport` lives in `worker.rs`'s test module.

- [ ] **Step 1: Failing tests** (`worker.rs`, against `SharedFake` set up like `amp.rs`'s `amp()`): `connects_when_port_appears` (port absent → `idle` yields nothing; present → `Connected`, `AmpOn(4)`, and the fake saw editor mode `00`); `unplug_mid_job_reports_disconnect` (fake `silent`, port check false → `handle(ReadSlot(3))` yields `Disconnected`); `live_on_reads_current` (`Current(4, p)` with `p` = fake's current patch; fake `editor` true); `read_slot_is_view_only` (`ReadSlot(2)` → `Slot(2, slot 3's patch)`; selector unchanged, no DT1 sent); `write_patch_reports_channel`; `job_without_amp_fails`; `coalesce_keeps_last_value_per_address` (`[Set(a,1), Set(b,1), Set(a,2), WritePatch, Set(a,3)]` → `[Set(b,1), Set(a,2), WritePatch, Set(a,3)]`). `link.rs`: `pair_round_trip` (send a job on the link, receive it on `LinkEnd`; send an event on `LinkEnd`, `try_recv` gets it).
- [ ] **Step 2:** `cargo test -p katana-gui amp::` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p katana-gui` → PASS.
- [ ] **Step 5:** Commit `feat(gui): amp worker with connection watch and job coalescing`.

### Task 4: Worker — Live tracking, store, shutdown

**Files:** Modify `katana-gui/src/amp/worker.rs`

**Interfaces:**
- Consumes: Task 1 (`amp::select`, `read_blocks`, `PANEL_BLOCKS`, `backup::*`, `store_patch`), Task 3 types.
- Produces: `pub fn store_failure_message(ch: Channel, error: &str, channel_backup: &Path, current_backup: &Path) -> String` → `"Storing into {ch} failed: {error}. The amp now plays {ch} with part of the new preset. Your previous sound: {current_backup} (Open it, then Load to amp). {ch}'s backup: {channel_backup}."`
- Behaviour (Live on = after `LiveOn`, until `LiveOff`/`Disconnected`):
  - The worker keeps `shadow: Option<Patch>` = the amp's current settings as last read or written by the worker; `WritePatch`/`SetParam` update it.
  - `idle` while Live on: `recv` with 20 ms timeout. `Selector(n)` notification → read current → `Current(n, p)` (shadow = p). A DT1 from the amp inside the current-patch area (`20 00 00 00`–`20 01 7F 7F`) → apply to shadow, `PanelChange`. Every `PANEL_REREAD = 1 s`, if `PANEL_BLOCKS` is not empty: `read_blocks(PANEL_BLOCKS)`, one `PanelChange` per contiguous run of bytes differing from the shadow, shadow updated.
  - Live off: `idle` drains and ignores amp messages, no re-read.
  - `Peek(ch)`: `read_slot` → `Peeked(ch, p)`.
  - `Store`: read slot and current, `write_backup` both (`what` = channel / `current`, names via `backup_name(stamp, ..)`), then `store_patch`; Ok → `Stored{..}` then `AmpOn(ch.selector())`; Err → `StoreFailed(store_failure_message(..))`. A backup that can't be written → `StoreFailed("Couldn't write the backup: …; nothing was stored")` and nothing sent to the amp. If Live was on: editor mode on again afterwards, shadow = re-read current.
  - `Shutdown` (and `Drop` of a connected worker): editor mode off.

- [ ] **Step 1: Failing tests:** `panel_dt1_becomes_panel_change` (Live on; fake queues a DT1 `20 00 06 00 ← 50` → `PanelChange{addr, [50]}`); `channel_switch_on_amp_rereads` (queued selector DT1 `00 06` → `Current(6, slot 7's patch)`); `rereads_panel_blocks_once_a_second` (fake's current gain changed behind the worker's back; `idle(t0)` nothing, `idle(t0 + 1.1 s)` → `PanelChange` for gain; count of RQ1s sent equals `PANEL_BLOCKS.len()` per re-read; skip the assertions with a comment if `PANEL_BLOCKS` is empty after Task 2 and use a test-only block list instead); `own_writes_are_not_panel_changes` (`SetParam` gain 70, then re-read → no `PanelChange`); `no_rereads_with_live_off`; `store_writes_backups_first` (temp dir; `Stored` paths exist and parse with `Patch::from_syx` as the slot's old patch and the old current patch) and `unwritable_backup_stores_nothing` (backup dir is a file, not a folder → `StoreFailed` containing "nothing was stored", and no DT1 to `PATCH_WRITE` or the patch area in the fake's `sent`); `store_failure_names_backups` (fake ignores the store, so read-back mismatches → message contains both paths and "Load to amp"); `live_survives_store` (Live on, `Store`, then queued panel DT1 → still `PanelChange`; fake `editor` true); `shutdown_turns_editor_mode_off`.
- [ ] **Step 2:** `cargo test -p katana-gui amp::` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p katana-gui` → PASS.
- [ ] **Step 5:** Commit `feat(gui): live tracking, store with backups, shutdown in the amp worker`.

### Task 5: Editor and library — amp content, notes, channel copies

**Files:** Modify `katana-gui/src/editor.rs`, `katana-gui/src/library.rs`, `katana-gui/src/app.rs` (autosave dirty flag, start); tests in `katana-gui/tests/autosave.rs`, `katana-gui/tests/library.rs`

**Interfaces:**
- Produces, `Editor`:
  - `fn replace_from_amp(&mut self, patch: Patch)` — patch and baseline = `patch`, origin kept, `replaced = origin.path().is_some()`.
  - `fn apply_amp_change(&mut self, addr: u32, data: &[u8])` — writes into the patch only (makes it dirty, like an app edit).
  - `fn replaced_note(&self) -> Option<String>` → `Some("Saving will overwrite library preset <file name>")` while `replaced`; `load`, `restore`, `saved_as`, `forget_origin` clear it.
  - `fn needs_restore_as_is(&self) -> bool` = `is_dirty() || replaced` — `KatanaApp::autosave` passes this as the `dirty` flag, so a restart restores the editor's content, not the origin file.
- Produces, `Library`:
  - `fn save_channel_copy(&self, selector: u8, p: &Patch) -> Result<(), String>` → `channels/<PANEL|A1…B4>.syx` (atomic); `fn channel_copy(&self, selector: u8) -> Option<Patch>`.
  - `autosave(..)` gains `channel: Option<u8>` stored in `state.json` (`#[serde(default)]`); `last_autosave()` returns it as a fourth tuple element.
- Produces, `KatanaApp`: `pub channel_tab: Option<u8>`; `start` restores it from the auto-save state.

- [ ] **Step 1: Failing tests:** editor `replace_keeps_origin_and_is_clean`, `amp_change_makes_dirty`, `note_only_for_library_origin` (default preset replaced → `None`), `load_clears_note`; library `channel_copy_round_trip` (A3 written, read back; B1 → `None`), `state_keeps_channel_tab`; autosave `replaced_content_survives_restart` (open a library patch, `replace_from_amp(other)`, auto-save, `KatanaApp::start` → editor patch == other, origin kept), `start_restores_channel_tab`.
- [ ] **Step 2:** `cargo test -p katana-gui` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p katana-gui` → PASS.
- [ ] **Step 5:** Commit `feat(gui): editor content from the amp, overwrite note, per-channel copies`.

### Task 6: App ↔ amp — connection, Live, tabs, Load, Open

**Files:** Modify `katana-gui/src/app.rs`, `katana-gui/src/views/top_bar.rs`, `katana-gui/src/views/action_bar.rs`, `katana-gui/src/views/status.rs`, `katana-gui/src/views/mod.rs`, `katana-gui/src/widgets/control.rs`, `katana-gui/src/views/chain.rs`; Create `katana-gui/tests/amp.rs`; Modify `katana-gui/tests/shell.rs`

**Interfaces:**
- Consumes: Task 3 `AmpLink`, `Job`, `Event`; Task 5 editor/library additions.
- Produces, `KatanaApp`:
  - `pub fn with_amp(self, link: AmpLink) -> KatanaApp`; fields `amp: Option<AmpLink>`, `pub connected: bool`, `pub amp_on: Option<u8>`, `pub live: bool`, `pub busy: Option<String>`, `pub last_panel_change: Option<(String, chrono::NaiveDateTime)>`.
  - `fn poll_amp(&mut self, now: chrono::NaiveDateTime)` — called first in `show`; drains every event.
  - `pub fn edit(&mut self, def: &ParamDef, value: u32) -> Result<(), ProtoError>` — `editor.set`, and with Live on sends `SetParam` with `encode_param` bytes. Every UI edit path (`widgets/control.rs`, `views/chain.rs`) goes through it.
  - `pub fn set_live(&mut self, on: bool)`, `pub fn click_channel(&mut self, selector: u8)`, `pub fn load_to_amp(&mut self)`.
- Behaviour (spec, "Live edit off", "Live edit on", "Both modes"):
  - `Connected` → `connected = true`; if `live` (kept across a disconnect) → quick-save if dirty, send `LiveOn`. `Disconnected` → `connected = false`, `busy = None`, `amp_on = None`.
  - `set_live(true)`: quick-save if dirty, send `LiveOn`; `live = true`. `set_live(false)`: send `LiveOff`.
  - `Current(sel, p)` (Live on): quick-save if dirty, `editor.replace_from_amp(p)`, `channel_tab = Some(sel)`, `amp_on = Some(sel)`, `library.save_channel_copy`.
  - `click_channel(sel)`: quick-save if dirty; connected + Live on → `Select(sel)`; connected + Live off → `ReadSlot(sel)`; not connected → load `channel_copy(sel)` with `replace_from_amp` (the tab is only enabled offline when a copy exists); `channel_tab = Some(sel)`. `Slot(sel, p)` → `replace_from_amp(p)`, `channel_copy` saved. PANEL with Live on only when `device::PANEL_SELECTABLE`, otherwise its tab is disabled with hover "Select PANEL on the amp".
  - `PanelChange` (Live on) → `editor.apply_amp_change`; `last_panel_change = (<param name from PARAMS at addr, else the hex address>, now)`.
  - `AmpOn(sel)` → `amp_on`; with Live on also `channel_tab`.
  - Open/Pick from the library: as today (discard prompt when dirty); then, with Live on, send `WritePatch(editor patch)`. Open, Save as, Quick save set `channel_tab = None` when Live is off.
  - `load_to_amp` → `WritePatch(editor patch)`; never changes the amp's channel (the user's choice).
  - Top bar: dot "connected"/"not connected"; tabs enabled when connected or when a channel copy exists for them (Live not needed); selected = `channel_tab`; Live edit checkbox enabled when connected. Action bar: Load to amp enabled when connected and not busy. Status line: `Amp: connected, on A1 · Live edit on · last auto-save 21:05 · panel change gain 21:07` (`—` when none), plus `busy` text; `editor.replaced_note()` shown next to the name in the top bar.
- Tests (`tests/amp.rs`, kittest with `AmpLink::pair()`; the test pushes events on `LinkEnd.events`, runs a frame, and reads `LinkEnd.jobs`).

- [ ] **Step 1: Failing tests:** `connect_enables_amp_actions`; `live_on_sends_live_on_and_takes_amp_settings` (dirty editor → a quick save file appears, then `Current(1, p)` → editor == p, tab A1 selected); `live_edit_sends_set_param` (knob change with Live on → `SetParam` for gain; Live off → none); `tab_click_live_off_reads_slot_only` (`ReadSlot(5)`, no `Select`); `tab_click_live_on_selects`; `open_with_live_on_writes_patch_not_store` (jobs == `[WritePatch(..)]`, never `Store`); `load_to_amp_writes_patch`; `panel_change_updates_editor_and_status`; `amp_switch_follows_with_live_on` (`Current(6, p)` → tab B2); `disconnect_disables_and_keeps_editor`; `reconnect_with_live_on_syncs_again`; `offline_tab_shows_channel_copy`; `replaced_note_shown`. `shell.rs`: `amp_actions_are_disabled` stays true without a link.
- [ ] **Step 2:** `cargo test -p katana-gui` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --workspace` → PASS; clippy → known warnings only.
- [ ] **Step 5:** Commit `feat(gui): live edit, channel tabs, load to amp`.

### Task 7: Store to channel… dialog

**Files:** Modify `katana-gui/src/app.rs`, `katana-gui/src/views/action_bar.rs`; tests in `katana-gui/tests/amp.rs`

**Interfaces:**
- Consumes: `Job::Peek`, `Job::Store`, `Event::Peeked/Stored/StoreFailed`; `Library::root()`.
- Behaviour: "Store to channel…" (enabled when connected, not busy and the library has a root; hover otherwise says why) opens a modal: channel combo box `A1`…`B4` preselected to `channel_tab` when it is 1–8 (else none); choosing a channel sends `Peek(ch)` and shows "Reading <ch>…"; on `Peeked` the question `"<ch> currently '<old>' → replace with '<new>'?"` with **Store** and **Cancel**; Escape/click outside = Cancel. Store sends `Store { ch, patch: editor patch, backup_dir: <library root>/backups, stamp: now as %Y-%m-%d_%H-%M-%S }`. `Stored` → status `stored '<new>' in <ch>; the amp is now on <ch>` (+ ` · solo delay not stored` when false), `channel_tab = Some(ch)`, `amp_on = Some(ch)`. `StoreFailed(msg)` → `error = msg`. The editor's content and dirty state are untouched by a store; nothing is saved to a file.
- Note: `"→"` is not in egui's default fonts — use `" -> "` in the UI string (the CLI keeps its arrow).

- [ ] **Step 1: Failing tests:** `store_asks_with_old_and_new_names`; `store_cancel_sends_nothing` (only `Peek` sent); `store_sends_store_with_backup_dir`; `stored_moves_tab_and_reports_solo_delay`; `store_failure_shows_message`; `store_does_not_save_file` (library files unchanged); `store_disabled_without_library`.
- [ ] **Step 2:** `cargo test -p katana-gui` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --workspace` → PASS.
- [ ] **Step 5:** Commit `feat(gui): store to channel with backups`.

### Task 8: Real worker in the app, quit and signals

**Files:** Modify `katana-gui/src/main.rs`, `katana-gui/src/app.rs`, `katana-gui/Cargo.toml`; `README.md` (GUI section)

**Interfaces:**
- `main.rs`: in the eframe creation closure, `AmpLink::spawn(Worker::new(<opens MidirTransport::open(DEFAULT_PORT)>, <MidirTransport::list_ports() contains DEFAULT_PORT>), <cc.egui_ctx.request_repaint>)` → `app.with_amp(link)`. Install a `ctrlc` handler (feature `termination`: SIGINT/SIGTERM/SIGHUP) that sets `app.quit_signal: Arc<AtomicBool>` and requests a repaint.
- `KatanaApp`: when `quit_signal` is set, auto-save and close without the discard prompt. `eframe::App::on_exit` → `amp.shutdown()` (editor mode off).

- [ ] **Step 1: Failing tests:** app `quit_signal_closes_without_prompt` (dirty editor, flag set, one frame → close requested, no "Discard" modal, an auto-save written); worker shutdown already covered in Task 4.
- [ ] **Step 2:** `cargo test -p katana-gui` → FAIL.
- [ ] **Step 3:** Implement; README: amp actions, Live edit rules, where backups go.
- [ ] **Step 4:** `cargo test --workspace` → PASS; `cargo build --release -p katana-gui`; start the app with the amp connected (official app closed): "connected" appears within ~1 s, nothing changes on the amp; quit; `katana-cli monitor` shows no editor-mode traffic left.
- [ ] **Step 5:** Commit `feat(gui): connect to the amp; editor mode off on quit and signals`.

### Task 9: On the amp — the user's two evenings (amp needed)

Say first: "The amp will play library patches and follow app edits; A3 will be stored and then put back from its backup."

- [ ] **Step 1:** Evening 1 with the user: start the app (Live off: last preset shown, amp untouched); Live on → editor shows the amp's channel and settings; Library → open a Tone Exchange patch → heard at once, channel tab = amp's channel, nothing stored (`katana-cli dump <channel> x.syx` before/after: identical); tweak gain in the app → heard; turn a panel knob → app follows; Save as a new file.
- [ ] **Step 2:** Evening 2: tweak live, Save as, Store to channel… → A3 (asks, backup paths in the status/backup folder); `katana-cli dump A3` equals the editor's patch outside `NOT_STORED`; restore A3 from its backup with `katana-cli store <backup> A3 --yes`.
- [ ] **Step 3:** Unplug the USB cable with Live on → "not connected", editor keeps its content; plug back → syncs again (amp wins, unsaved edits quick-saved first).
- [ ] **Step 4:** Record evidence in `docs/protocol.md` (if anything new) and update `docs/STATUS.md` (stage 2 done pending the user's check; deferred items). Commit `docs: stage 2 verified on the amp`.
- [ ] **Step 5:** Ask the user for the hands-on check.
