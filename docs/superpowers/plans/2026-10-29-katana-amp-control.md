# Stage 1: Amp control (CLI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `katana-cli store`, `dump <channel> <file>` and `tsl-list`: whole patches between `.syx`/`.tsl` files and the amp's stored channels A1–B4, with backups, questions and read-back.

**Architecture:** Protocol work goes into `katana-proto`: a `Channel` type, an `amp` module (editor mode, channel select, store, notifications, high-level `store_patch`) and a `.tsl` writer. `FakeAmp` learns editor mode, channel select and store so it all runs in unit tests. `katana-cli` gets small, tested helpers (file format by extension, the y/N question, backups) and three thin commands. Hardware facts the code depends on (solo delay written straight into a stored channel, settle times, blocks the amp doesn't store) are found by a hardware task on scratch channels and recorded as `device` constants with evidence.

**Tech Stack:** Rust 2021; existing crates `katana-proto`, `katana-cli`; `clap` 4, `anyhow`, `serde_json`; new in the CLI: `directories = "6"`, `chrono` (as in `katana-gui`), `ctrlc = "3"`; `serde_json` gains the `preserve_order` feature.

**Spec:** `docs/superpowers/specs/2026-10-29-katana-amp-control-design.md` (approved 2026-10-29). Background: `docs/protocol.md` § "Channels", § "Command addresses", § "Amp control".

## Global Constraints

- Existing commands keep their behaviour exactly: `ports`, `identity`, `monitor`, `extract`, `diff`, `dump <file>`, `load`, `verify`, `set`, `params`, `watch`. Existing tests must still pass unchanged.
- Never overwrite a stored channel without its backup on disk first; never overwrite a channel or an existing file without the y/N question unless `--yes`.
- Questions default to No. Without a terminal on stdin and without `--yes`: refuse with an error that says to add `--yes`.
- Never report something stored that the amp didn't store (solo delay).
- The amp is taken out of editor mode when a command ends, on success and on error; Ctrl-C during the amp sequence is delayed until the sequence ends (best effort).
- Hardware writes to stored channels during development: **only A3, A4, B3, B4, after backing them up**; say what will be heard before writing; final and by-ear checks are run by the user.
- Must still build on macOS; no new heavy dependencies.
- `samples/*.tsl` are other people's presets: never commit them.
- `cargo clippy --workspace --all-targets`: no new warnings (3 known ones in `katana-proto`).
- Work on branch `feature/amp-control`; commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Store fails half-way** (timeout after `write_patch`, before or after `PATCH_WRITE`): the command must exit with an error that prints both backup paths, and editor mode must be off. Test in Task 3 (`store_patch_leaves_editor_mode_on_error`) and Task 6 (error message names backups).
2. **`.tsl` patch that can't be loaded** (`SetPatch.patch` is `Err`, e.g. a 6-byte COLOR with a non-zero byte) or `--patch 0` / `--patch` beyond the set: clear error before the amp is touched. Tests in Task 5 (`tsl_patch_number_out_of_range`, `tsl_broken_patch_is_refused`).
3. **Unsupported or upper-case extension** (`x.SYX`, `x.TSL`, `x.json`, no extension): upper-case works; others fail naming `.syx` and `.tsl`. Test in Task 5 (`format_by_extension`).
4. **Patch names unsafe in file names** (`/`, `..`, spaces, empty name) in backup file names: sanitised, never escapes the backup folder. Test in Task 5 (`backup_name_is_safe`).
5. **`dump A3` with no file** (one argument that is a channel name): error "dump A3 needs a file", not a file named `A3`. Test in Task 6 (`dump_channel_without_file_is_refused`).

---

### Task 1: `Channel` type

**Files:**
- Create: `katana-proto/src/channel.rs`
- Modify: `katana-proto/src/lib.rs` (add `pub mod channel; pub use channel::Channel;`)

**Interfaces:**
- Produces: `pub enum Channel { A1, A2, A3, A4, B1, B2, B3, B4 }` (`Copy, Clone, Debug, PartialEq, Eq`); `Channel::ALL: [Channel; 8]`; `fn selector(self) -> u8` (A1 = 1 … B4 = 8, see protocol.md "Channels"); `fn slot(self) -> u8` (= selector + 1, for `read_slot`); `impl FromStr` (case-insensitive `a3`/`A3`; error type `ChannelError(String)` with message `unknown channel "X"; use A1–A4 or B1–B4`); `impl Display` (`A3`).

- [ ] **Step 1: Write the failing tests** in `channel.rs`:
  - `parses_any_case`: `"a3".parse() == Ok(Channel::A3)`, `"B4".parse() == Ok(Channel::B4)`.
  - `rejects_others`: `"PANEL"`, `"A5"`, `"C1"`, `""` are `Err`.
  - `selector_and_slot`: `A1.selector() == 1`, `A4.selector() == 4`, `B1.selector() == 5`, `B4.selector() == 8`, `B4.slot() == 9`.
  - `display_round_trips`: for each of `Channel::ALL`, `c.to_string().parse() == Ok(c)`.
- [ ] **Step 2:** `cargo test -p katana-proto channel` → FAIL (does not compile).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p katana-proto channel` → PASS.
- [ ] **Step 5:** Commit `feat(proto): Channel A1–B4 with selector and stored slot`.

### Task 2: Amp control primitives and a fake amp that knows them

**Files:**
- Create: `katana-proto/src/amp.rs`
- Modify: `katana-proto/src/fake.rs`, `katana-proto/src/device.rs`, `katana-proto/src/lib.rs`, `katana-proto/src/patch.rs` (make `slot_offset` and `read_patch_blocks` usable from `amp`, already `pub(crate)`)

**Interfaces:**
- Consumes: `Channel` (Task 1); `Transport`, `encode`/`decode`, `read_patch_blocks`, `slot_offset`.
- Produces, in `device.rs` (each with an evidence comment pointing at protocol.md "Amp control"):
  `pub const SELECTOR: u32 = addr([0,0,0,0]);` (2 bytes, INTEGER2x4: value = `b0 << 4 | b1`)
  `pub const EDITOR_MODE: u32 = addr([0x7F,0,0,1]);`
  `pub const PATCH_SELECT: u32 = addr([0x7F,0,1,0]);`
  `pub const PATCH_WRITE: u32 = addr([0x7F,0,1,4]);`
  `pub const SELECT_SETTLE: Duration = Duration::from_millis(300);` (starting value; Task 4 tunes it)
- Produces, in `amp.rs`:
  - `pub struct EditorSession<'t>`; `EditorSession::enter(t: &'t mut dyn Transport) -> Result<Self, ProtoError>` sends `EDITOR_MODE ← 01`; `fn transport(&mut self) -> &mut dyn Transport`; `fn close(self) -> Result<(), ProtoError>` sends `← 00`; `Drop` sends `← 00` if not closed, ignoring errors.
  - `pub fn read_selector(t: &mut dyn Transport, timeout: Duration) -> Result<u8, ProtoError>` (RQ1 `SELECTOR` size 2).
  - `pub fn select_channel(s: &mut EditorSession, ch: Channel, timeout: Duration) -> Result<(), ProtoError>`: DT1 `PATCH_SELECT ← [0, selector]`, then poll `read_selector` until it equals `ch.selector()` (error `ProtoError::Timeout { addr: Address(SELECTOR) }` after `timeout`), then sleep `SELECT_SETTLE`.
  - `pub fn store_current(s: &mut EditorSession, ch: Channel, timeout: Duration) -> Result<bool, ProtoError>`: DT1 `PATCH_WRITE ← [0, selector]`; waits up to `timeout` for the `PatchWritten(selector)` notification; returns whether it came (absence is not an error — the read-back decides).
  - `pub fn write_slot_block(t: &mut dyn Transport, ch: Channel, block: (u32, u32), p: &Patch) -> Result<(), ProtoError>`: DT1s of `p`'s bytes for a current-patch `block`, relocated by `slot_offset(ch.slot())` (route b; used for SOLO_DELAY).
  - `pub enum Notification { Selector(u8), PatchWritten(u8), Other { addr: Address, data: Vec<u8> } }` and `pub fn notification(h: &Header, msg: &[u8]) -> Option<Notification>` (DT1 to `SELECTOR` → `Selector`, to `PATCH_WRITE` → `PatchWritten`, any other DT1 → `Other`; non-DT1 → `None`).
- Produces, in `FakeAmp`: `pub editor: bool` (false at start), `pub echo: bool` (true at start). Behaviour: DT1 `EDITOR_MODE` sets `editor`. With `editor` on: `PATCH_SELECT [0,n]` sets the selector bytes in `mem`, copies stored slot `n + 1` over the current patch (all `PATCH_BLOCKS`, ignoring `read_only`), and if `echo` queues DT1 `SELECTOR ← [0,n]`; `PATCH_WRITE [0,n]` copies the current patch into slot `n + 1` (all blocks, ignoring `read_only`) and if `echo` queues DT1 `PATCH_WRITE ← [0,n]`. With `editor` off both are ignored. DT1 to the `7F` command area and to `SELECTOR` is never stored in `mem` as data.

- [ ] **Step 1: Write the failing tests** in `amp.rs` (fake with every block of slots 1..=9 and the current patch filled with distinct bytes, e.g. slot n = `n`, current = `0x40`; selector bytes `[0,4]`):
  - `editor_session_turns_mode_off_on_drop`: enter, drop without `close` → `amp.editor == false`; last sent message is DT1 `EDITOR_MODE ← 00`.
  - `select_needs_editor_mode`: DT1 `PATCH_SELECT ← [0,3]` sent directly without a session leaves the selector at 4 and the current patch unchanged.
  - `select_channel_switches_and_loads_slot`: in a session, `select_channel(A2)` → `read_selector == 2`, current patch equals slot 3's bytes.
  - `select_channel_times_out_when_amp_ignores_it`: fake with `ignore_dt1 = true` → `Err(ProtoError::Timeout { .. })` (use a 50 ms timeout).
  - `store_current_copies_into_slot_and_sees_echo`: select B3, write a patch with name "Stored" via `write_patch`, `store_current(B3)` → `Ok(true)`; `read_slot(B3.slot())` has name "Stored". With `amp.echo = false` → `Ok(false)` and the slot still holds it.
  - `write_slot_block_writes_relocated_bytes`: `write_slot_block(B4, SOLO_DELAY block, p)` changes slot 9's SOLO_DELAY bytes to `p`'s and nothing else in `amp.mem`.
  - `notifications_decode`: DT1 `SELECTOR ← [0,5]` → `Selector(5)`; DT1 `PATCH_WRITE ← [0,8]` → `PatchWritten(8)`; DT1 `7F 01 01 05 ← [0x12]` → `Other`; an RQ1 → `None`.
- [ ] **Step 2:** `cargo test -p katana-proto amp` → FAIL.
- [ ] **Step 3:** Implement `device` constants, `FakeAmp` behaviour, then `amp.rs`.
- [ ] **Step 4:** `cargo test -p katana-proto` → all PASS (existing fake tests unchanged).
- [ ] **Step 5:** Commit `feat(proto): editor mode, channel select, store, notifications`.

### Task 3: `store_patch`

**Files:**
- Modify: `katana-proto/src/amp.rs`, `katana-proto/src/device.rs`, `katana-proto/src/patch.rs` (generalise `mismatch` to skip a list of ranges), `katana-proto/src/error.rs`

**Interfaces:**
- Consumes: Task 2.
- Produces:
  - `device.rs`: `pub const NOT_STORED: &[(u32, u32)]` — current-patch-address ranges the amp doesn't store from our writes, excluded from the post-store comparison. Starts as `WRITE_PROTECTED` (SOLO_DELAY). `pub const SLOT_SOLO_DELAY_WRITABLE: bool = false;` (Task 4 decides it).
  - `amp.rs`: `pub struct StoreReport { pub solo_delay_stored: bool, pub echo_seen: bool }` and
    `pub fn store_patch(t: &mut dyn Transport, ch: Channel, p: &Patch, timeout: Duration) -> Result<StoreReport, ProtoError>`.
    Sequence (route a, decided in this plan): enter editor session → `select_channel(ch)` (first, so the channel keeps its own solo delay) → `write_patch` → `store_current(ch)` → if `SLOT_SOLO_DELAY_WRITABLE`, `write_slot_block(ch, SOLO_DELAY, p)` → `read_slot(ch.slot())` compared with `p` skipping `NOT_STORED` (and not skipping SOLO_DELAY when it was written) → `close`. Mismatch → `ProtoError::StoreMismatch { channel: String, addr: Address, expected: u8, got: Option<u8> }` (new variant, message `channel {channel} differs after storing at {addr}: expected {expected:#04x}, read {got:?}`).
  - `patch.rs`: `pub(crate) fn mismatch_skipping(expected: &Patch, got: &Patch, skip: &[(u32, u32)]) -> Option<(Address, u8, Option<u8>)>`; the existing `mismatch` calls it with `WRITE_PROTECTED`.

- [ ] **Step 1: Write the failing tests** in `amp.rs`:
  - `store_patch_stores_and_ends_on_channel`: fake (current patch = slot of selector 4), `store_patch(B3, p)` → `Ok`; `read_selector == 7`; slot 8 equals `p` outside SOLO_DELAY; `amp.editor == false`.
  - `store_patch_keeps_channel_solo_delay`: fake with SOLO_DELAY of the current patch `read_only` (as the real amp); slot 8's SOLO_DELAY = `0x55`s, the playing channel's = `0x66`s, `p`'s = `0x77`s → after store slot 8's SOLO_DELAY is still `0x55`s and `report.solo_delay_stored == false`.
  - `store_patch_detects_write_that_did_not_land`: fake with `read_only` covering the AMP block of slot 8 → `Err(ProtoError::StoreMismatch { .. })`, editor off.
  - `store_patch_leaves_editor_mode_on_error`: fake that times out on select (`ignore_dt1`) → `Err`, last message sent is `EDITOR_MODE ← 00`.
  - `store_patch_without_echo_still_verifies`: `amp.echo = false` → `Ok`, `report.echo_seen == false`.
- [ ] **Step 2:** `cargo test -p katana-proto store_patch` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p katana-proto` → PASS.
- [ ] **Step 5:** Commit `feat(proto): store a patch into a channel with read-back`.

### Task 4: Hardware probe — solo delay, settle time, notifications (amp needed)

Ask the user to connect the amp and close the official app first. Tell them before running: "B4 will be overwritten for a moment and put back; you may hear the amp switch to B4."

**Files:**
- Modify: `katana-proto/tests/hardware.rs`, `katana-proto/src/device.rs`, `docs/protocol.md`

- [ ] **Step 1: Back up the scratch channels.** The stored-channel `dump` doesn't exist yet, so add hardware test `backup_scratch_channels` that writes `read_slot(n).to_syx()` for slots 4, 5, 8, 9 (A3, A4, B3, B4) to `backups/<YYYY-MM-DD>-slot<n>.syx` (keep the test). Run it; confirm four files exist and each parses with `Patch::from_syx`.
- [ ] **Step 2: Write hardware test `slot_solo_delay_direct_write`** on B4: read slot 9; set its SOLO_DELAY level byte (`20 00 3E 00` + level offset from protocol.md, relocated) to a different value via `write_slot_block`; read back; restore the original block; read back. Print both results; assert the restore is byte-identical. Run: `cargo test -p katana-proto --features hardware-tests slot_solo_delay -- --nocapture`.
- [ ] **Step 3: Record the result.** If the changed byte read back: set `SLOT_SOLO_DELAY_WRITABLE = true` and remove SOLO_DELAY from `NOT_STORED`; otherwise leave both. Evidence comment in `device.rs` with date and values.
- [ ] **Step 4: Hardware test `store_patch_round_trip_on_b4`**: take slot 9 as `orig`, make `p = orig` with name "KTS TEST" and GAIN changed, `store_patch(B4, p)`, then `store_patch(B4, orig)`; assert both `Ok` and slot 9 byte-identical to `orig` at the end. If it fails because a block (e.g. `PATCH_KNOB_READONLY`) is amp-owned, add that block to `NOT_STORED` with evidence and re-run; if select loses writes, raise `SELECT_SETTLE` (record the value that works, with evidence). Note `echo_seen` and any `Notification::Other` addresses seen (print them with `--nocapture`).
- [ ] **Step 5: Document** in `docs/protocol.md` § "Amp control": move the verified items out of "not yet in katana-proto", add the solo-delay result, `SELECT_SETTLE`, `NOT_STORED`, the notifications observed.
- [ ] **Step 6:** `cargo test --workspace` → PASS. Commit `test(proto): hardware probes for storing into a channel` (docs + device constants included).

### Task 5: `.tsl` writer and CLI file helpers

**Files:**
- Modify: `Cargo.toml` of `katana-proto` (`serde_json = { version = "1", features = ["preserve_order"] }`), `katana-proto/src/tsl.rs`, `katana-proto/src/device.rs`
- Create: `katana-cli/src/files.rs`, `katana-cli/src/ask.rs`, `katana-cli/src/backup.rs`
- Modify: `katana-cli/Cargo.toml` (`directories = "6"`, `chrono = { version = "0.4", default-features = false, features = ["clock"] }`), `katana-cli/src/main.rs` (`mod files; mod ask; mod backup;`)

**Interfaces:**
- Produces, `katana-proto`:
  - `device.rs`: `pub const TSL_BLOCK_ORDER: [usize; 80]` — indexes into `PATCH_BLOCK_NAMES` in the order real Tone Exchange files list them (all 21 sample patches share one order: `COM, OTHER, COLOR, PATCH_KNOB_READONLY, PATCH_KNOB_SOLO_DELAY_READONLY, AMP, SW, BOOSTER(1)…` — take the full order from `samples/Black_Sabbath.tsl`).
  - `tsl.rs`: `pub fn write_tsl(p: &Patch) -> Vec<u8>` — one-patch set: `{"name": <p.name()>, "formatRev": "0000", "device": "KATANA Gen3", "data": [[{"memo": "", "paramSet": {...}}]]}`, keys in that order, bytes as upper-case two-digit hex strings, blocks in `TSL_BLOCK_ORDER`.
- Produces, `katana-cli`:
  - `files.rs`: `pub enum Format { Syx, Tsl }`; `pub fn format_of(path: &Path) -> anyhow::Result<Format>` (case-insensitive; error `"{path}: use a .syx or .tsl file"`); `pub fn read_patch_file(path: &Path, patch: Option<usize>) -> anyhow::Result<Patch>` (`.tsl` needs `Some(n)`, 1-based: error `"{path} is a set: choose a patch with --patch N (1–{len}); see katana-cli tsl-list {path}"`; `.syx` with `Some` → error `"--patch only applies to .tsl files"`; out of range → error naming `1–{len}`; a `SetPatch` whose `patch` is `Err(e)` → error `"patch {n} '{name}' can't be loaded: {e}"`); `pub fn patch_bytes(f: Format, p: &Patch) -> Vec<u8>` (`to_syx` or `write_tsl`).
  - `ask.rs`: `pub fn confirm(question: &str, yes: bool, interactive: bool, input: &mut dyn BufRead, out: &mut dyn Write) -> anyhow::Result<bool>` — `yes` → `Ok(true)` without printing; not `interactive` → `Err("… add --yes to answer yes without a question")`; otherwise prints `"{question} [y/N] "` and returns true only for `y`/`yes` (any case, trimmed). `pub fn confirm_stdin(question: &str, yes: bool) -> anyhow::Result<bool>` uses `std::io::IsTerminal` on stdin.
  - `backup.rs`: `pub fn backup_dir() -> anyhow::Result<PathBuf>` (`ProjectDirs::from("", "", "katana-ts")` data dir + `backups`, created if missing); `pub fn backup_name(now: chrono::NaiveDateTime, what: &str, patch_name: &str) -> String` → `"{%Y-%m-%d_%H-%M-%S}_{what}_{safe name}.syx"` where `what` is `A3` or `current`; safe name keeps `[A-Za-z0-9 _-]`, other characters become `_`, trimmed, empty → `unnamed`; `pub fn write_backup(dir: &Path, name: &str, p: &Patch) -> anyhow::Result<PathBuf>` (never overwrites: on an existing name appends `_2`, `_3`, …).

- [ ] **Step 1: Write the failing tests.**
  - `tsl.rs` `write_tsl_round_trips`: `default_preset` named "Iron Man" with GAIN 70 → `ToneSet::parse(&write_tsl(&p))` gives one patch equal to `p`, set name "Iron Man".
  - `tsl.rs` `write_tsl_matches_real_key_order`: keys of the written `paramSet` (parsed back with `preserve_order`) equal the `paramSet` keys of the first patch in `samples/Black_Sabbath.tsl` when that file exists (pass trivially otherwise, like `tsl_samples.rs`); top-level keys are `["name","formatRev","device","data"]`.
  - `katana-proto/tests/tsl_samples.rs` `real_samples_survive_write`: every loadable sample patch → `write_tsl` → parse → equal.
  - `files.rs` `format_by_extension`: `a.syx`, `a.SYX` → Syx; `a.Tsl` → Tsl; `a.json`, `a` → Err containing `.syx or .tsl`.
  - `files.rs` `tsl_needs_patch_number`, `syx_refuses_patch_number`, `tsl_patch_number_out_of_range` (`--patch 0` and `len + 1`), `tsl_broken_patch_is_refused` (set with a COLOR byte list of 6 ending in `"01"` → error text contains `can't be loaded`): build sets in a temp dir with `write_tsl` / hand-made JSON.
  - `ask.rs` `yes_skips_question`, `default_is_no` (input `"\n"` → false), `y_and_yes_accept` (`"Y\n"`, `" yes \n"`), `not_interactive_refuses` (error mentions `--yes`), `question_text_printed` (out == `"Overwrite x.tsl? [y/N] "`).
  - `backup.rs` `backup_name_format` (2026-10-29 21:05:09, `A3`, `Riff Shred` → `2026-10-29_21-05-09_A3_Riff Shred.syx`), `backup_name_is_safe` (`"../a/b"` → no `/` and no `..` in the result; `""` → `unnamed`), `write_backup_never_overwrites` (two writes with the same name → two files, the second ends `_2.syx`).
- [ ] **Step 2:** `cargo test -p katana-proto tsl && cargo test -p katana-cli` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test --workspace` → PASS (GUI tests too — `preserve_order` must not change them).
- [ ] **Step 5:** Commit `feat: .tsl writer; CLI file format, questions and backups`.

### Task 6: CLI commands `tsl-list`, `dump <channel> <file>`, `store`

**Files:**
- Create: `katana-cli/src/cmd/store.rs`, `katana-cli/src/cmd/tsl_list.rs`
- Modify: `katana-cli/src/main.rs`, `katana-cli/src/cmd/mod.rs`, `katana-cli/src/cmd/dump.rs`, `katana-cli/Cargo.toml` (`ctrlc = "3"`); `README.md` (CLI section)

**Interfaces:**
- Consumes: `Channel`, `store_patch`, `StoreReport`, `read_slot`, `read_patch` (proto); `files`, `ask`, `backup` (Task 5).
- Clap, in `Command`:
  - `TslList { file: PathBuf }` → name `tsl-list`. Prints `"{n:>3}  {name}"` per patch, and ` (can't be loaded: {e})` after a broken one.
  - `Dump { first: String, second: Option<PathBuf>, #[arg(long)] yes: bool }`. `second == None` → the existing current-patch dump with `first` as the path, **unchanged** (no question, `.syx` whatever the extension) — except when `first` parses as a `Channel`: error `"dump {ch} needs a file: katana-cli dump {ch} <file.syx|file.tsl>"`. `second == Some(file)` → `first` must be a channel; `format_of(file)` checked before the amp is opened; if `file` exists, `confirm_stdin("Overwrite {file}?", yes)`; `read_slot(ch.slot())`; write `patch_bytes`; print `"wrote {file}: {ch} '{name}'"`.
  - `Store { file: PathBuf, channel: String, #[arg(long)] patch: Option<usize>, #[arg(long)] yes: bool }`.
- `store.rs` flow: parse channel → `read_patch_file` (all file errors before the amp) → open transport → `read_slot(ch.slot())` and `read_patch` → `write_backup` both (`what` = channel / `current`), print `"backup of {ch} '{name}': {path}"` and `"backup of the current sound '{name}': {path}"` → `confirm_stdin("{ch} currently '{old}' → replace with '{new}'?", yes)`; on no, print `"nothing changed"` and exit 0 → install a `ctrlc` handler that only sets an `AtomicBool` → `store_patch(&mut t, ch, &p, TIMEOUT)` → on error, return it with context `"storing into {ch} failed; backups: {path1}, {path2}"` → print `"stored '{new}' in {ch}; the amp is now on {ch}"`, and `"solo delay not stored"` when `!report.solo_delay_stored` → if the flag was set, exit with code 130 after printing.
- The only unit-testable part without an amp is argument handling; keep `store.rs`'s amp-free checks in a function `fn prepare(file: &Path, channel: &str, patch: Option<usize>) -> anyhow::Result<(Channel, Patch)>`.

- [ ] **Step 1: Write the failing tests.**
  - `cmd/store.rs` `prepare_checks_before_amp`: unknown channel `"C1"` → Err; `"a3"` with a `.syx` written from `default_preset` → `(Channel::A3, p)`; `.tsl` without `--patch` → Err mentioning `--patch`.
  - `cmd/dump.rs` `dump_channel_without_file_is_refused`: `fn plan(first: &str, second: Option<&Path>) -> anyhow::Result<DumpPlan>` with `enum DumpPlan { Current(PathBuf), Channel(Channel, PathBuf, Format) }`: `("a3", None)` → Err containing `needs a file`; `("now.syx", None)` → `Current`; `("A3", Some("x.TSL"))` → `Channel(A3, _, Tsl)`; `("now.syx", Some("x.syx"))` → Err (first is not a channel); `("A3", Some("x.txt"))` → Err.
  - `main.rs` `cli_parses`: `Cli::try_parse_from(["katana-cli","store","x.tsl","A3","--patch","4","--yes"])` is Ok; `["katana-cli","dump","now.syx"]` is Ok; `["katana-cli","tsl-list","x.tsl"]` is Ok; `["katana-cli","set","gain","40"]` still Ok.
- [ ] **Step 2:** `cargo test -p katana-cli` → FAIL.
- [ ] **Step 3:** Implement; update the README's CLI section with the three commands and where backups go.
- [ ] **Step 4:** `cargo test --workspace` → PASS; `cargo clippy --workspace --all-targets` → only the 3 known warnings.
- [ ] **Step 5:** Commit `feat(cli): store, dump <channel> and tsl-list`.

### Task 7: On the amp — commands end to end (amp needed)

Say what will be heard first ("A3 will be replaced, the amp will switch to A3; then A3 is put back from its backup").

- [ ] **Step 1:** `katana-cli tsl-list "samples/Black_Sabbath.tsl"` → numbered names.
- [ ] **Step 2:** `katana-cli dump A3 backups/stage1/a3.syx` and `dump A3 backups/stage1/a3.tsl` (`backups/` is untracked) → both written; running again asks `Overwrite …? [y/N]`, Enter leaves the file unchanged (compare mtime/bytes).
- [ ] **Step 3:** `katana-cli store "samples/Black_Sabbath.tsl" A3 --patch 1` → two backup paths printed, question, `y` → "stored 'iron man' in A3; the amp is now on A3" (plus "solo delay not stored" if Task 4 found so). `dump A3 x.syx` and compare with the patch from the set outside `NOT_STORED` (`katana-cli diff`).
- [ ] **Step 4:** Restore: `katana-cli store <A3 backup>.syx A3 --yes` → `dump A3 after.syx`; `katana-cli diff` against the Step 2 dump → no differences.
- [ ] **Step 5:** Round trips: `store a3.tsl A4 --patch 1 --yes`, then `dump A4 a4.tsl` → same patch as `a3.tsl` outside `NOT_STORED`; restore A4 from its backup.
- [ ] **Step 6:** Record the evidence (commands, outputs, dates) in `docs/protocol.md` and update `docs/STATUS.md` (stage 1 done pending the user's check; new commands; `--no-cs` deferred). Commit `docs: stage 1 verified on the amp`.
- [ ] **Step 7:** Ask the user for the hands-on check: store a set patch into a scratch channel, play it, restore it from the backup, and try a written `.tsl` in the official Tone Studio (Wine) if they want to.
