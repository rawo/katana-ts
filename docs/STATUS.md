# Project status and handoff

Last updated: 2026-11-11 (full editor round built on `feature/full-editor`; effect colours round merged into `main` and pushed 2026-11-10). Read this first when picking the work up.

## Full editor round: built on `feature/full-editor`, waiting for review and the user's check

Spec `docs/superpowers/specs/2026-11-10-katana-full-editor-design.md`, plan
`docs/superpowers/plans/2026-11-11-katana-full-editor.md`, ledger (rulings)
`.superpowers/sdd/2026-11-11-katana-full-editor/progress.md` (git-ignored).

- Every BOSS Tone Studio setting of BOOSTER, MOD, FX, DELAY, DELAY 2, REVERB, SOLO,
  EQ 1/2, CONTOUR and the AMP (RESONANCE, BLOOM, VARIATION), in all three colours:
  colour tabs (view only), TYPE drop-downs for every colour block, a mark on what the
  panel knob also moves. DELAY 2 / REVERB is one block with its layer MODE.
- The chain follows the patch (CHAIN, EQ positions) and can be reordered by dragging,
  only into orders the amp allows (Tone Studio's tables).
- Settings come from Tone Studio's data: `scripts/bts_settings.py` writes
  `docs/settings.md` and, with `--rust`, `katana-proto/src/settings_gen.rs`.
- Checked on the amp on the Mac 2026-11-11: `katana-cli check-settings` wrote and read
  back all 1222 new settings, nothing excluded; the REVERB button's switch behaviour
  per mode recorded (`docs/protocol.md`, "Settings check", "DELAY 2 / REVERB switches").
- Still to do: whole-branch review, the user's hands-on check (Mac, then Linux), merge.

## Effect colours round: done, merged to `main` 2026-11-10

Scope agreed with the user (2026-11-07, approved in chat, no written spec): mirror in
the app what is visible on the amp's panel. For DELAY, REVERB, MOD and FX: colour
lights that work both ways (like the booster), and the settings the effect's knob moves,
for the current colour. The full Tone Studio settings per effect are the **next**
round. One effect at a time, each recorded with the user at the amp
(`katana-cli monitor --editor > captures/1NN-….log`, dump before/after, diff), built
test-first, then checked by the user.

| Part | State | Evidence |
|---|---|---|
| DELAY: colours, TIME/FEEDBACK/EFFECT LEVEL per colour, TAP light (red, blinks at the delay time, only with the delay on) | done, user's check passed | `captures/105-*`, `106-*` |
| REVERB: colours, EFFECT LEVEL per colour | done, user's check passed | `captures/107-*` |
| MOD: colours, type line, the knob's settings for the colour's type | done, user's check passed | `captures/108-*` |
| FX: same as MOD | done, user's check passed | `captures/109-*` |
| Knob settings per effect type (31 types, MOD and FX share them) | **all 31 mapped**, user's check passed 2026-11-10 (knob and colours in sync both ways) | `captures/108`–`113` |

How MOD/FX work: MOD green/red/orange are slots FX(1..3), FX green/red/orange FX(4..6).
Each slot has a type; the knob (default "PRESET" assignment, `ASSIGN_KNOBS`) moves
type-specific settings, not one to one. Per-type table: `KNOB` in
`katana-proto/src/modfx.rs`; parameters are generated for all six slots
(`<slot>-<type>-<setting>`, `all_params()`). Details in `docs/protocol.md`, "MOD colours and knob".

**Type batches: done 2026-11-10.** Every type was recorded at the amp (MOD green,
`katana-cli set mod1-type N`, the user sweeping the knob): batch 1 (1–6, `110`),
batch 2 (7–12, `111`), batch 3 (13–15, 17–19, `112`), batch 4 (20–22, 24, 27–30,
`113`). Offsets per type in `docs/protocol.md`, "MOD colours and knob". GUITAR SIM
TYPE and HARMONIST HARMONY1 are value lists (names from the app's
`export/layout.div`, a drop-down in the GUI; "->" instead of an arrow, which egui's
default font lacks; the drop-down shows about 200 px, longer lists scroll). The "not
mapped yet" message is gone (no type left to show it).

The user's check in the app passed 2026-11-10: knob settings and colours stay in sync
both ways. Reviewed, fixed, merged (fast-forward) into `main` and pushed 2026-11-10;
branch deleted. **Next round:** the full Tone Studio settings per effect, with a TYPE
drop-down for MOD and FX (see the notes below).

Branch review 2026-11-10 (fresh reviewer, whole branch): ready with fixes, no critical
issues. Fixed: the TAP light repainted ~200 times a second at short delay times (now
steady below 150 ms, no repaints; dark at 0); name and byte-overlap tests now cover
the generated parameters; `KNOB` sorted by type with a test; MOD/FX Live tests.

Noted for the full-settings round: a TYPE drop-down for MOD and FX (the user,
2026-11-10; today the type is shown but read-only, set with `katana-cli set
mod1-type N`). With Live on the app does not see writes from other programs (e.g.
`katana-cli set`): the amp does not report them and only the AMP block is re-read;
switching Live off and on, or a `katana-cli watch`, brings them in. The knob settings
shown assume the knob's PRESET assignment: a patch whose `ASSIGN_KNOBS` (`20 00 5C 00`)
points a knob elsewhere shows settings the knob doesn't move (also the booster) — check
the byte and say so. A slot type 31..127 (damaged patch) shows "No parameters verified…"
instead of "type ? (n)". `knob_targets`/`slot_type` look names up by string every frame:
index `GENERATED` before the TYPE drop-down adds more. LIMITER RATIO shows raw steps 0–17 (value names not
found in the app's files); HIGH CUT (delay) and LOW CUT/DENSITY (reverb) change by
themselves on a colour change and are not shown; the CLI `set`/`params` show raw values
(no `ofs`).

The amp's current sound (A3 active) was changed by the recordings and not stored; the
user said it is fine as it is.


## Stage 2, editor <-> amp: done, merged to `main` 2026-11-06

Verified on the amp with the user on 2026-11-06 (evidence: `docs/protocol.md`, "GUI end
to end"); the user's hands-on check passed (open, load to amp, play, tweak, store to a
channel, save in the library: "All works"). Merged into `main` and pushed to `origin`. The plan workspace (ledger, review diff, logs) is kept on purpose.
**Next:** colour indicators for MOD, FX, DELAY and REVERB: in progress, see above.

- Spec: `docs/superpowers/specs/2026-10-31-katana-editor-amp-design.md`
- Plan: `docs/superpowers/plans/2026-10-31-katana-editor-amp.md`
- Ledger: `.superpowers/sdd/2026-10-31-katana-editor-amp/progress.md` (git-ignored,
  local; `git clean -fdx` would destroy it).
- Snapshot of all 8 channels and the current sound before Task 9:
  `backups/2026-11-06-before-task9/` (untracked). A3 was stored and restored; it is
  byte-identical to the snapshot.

### Done

- Tasks 1-8, the branch review (C1, I1-I13 fixed; I14 partly), and Task 9 on the amp.
- Seen on the amp: Live on/off, Open with Live on (nothing stored), edits both ways,
  fast drags, colours vs LEDs, amp-type steps, fast channel presses, Save as, Store to
  A3 with backups, unplug/replug, fast replug, power cycle, cable pulled during a
  store, SIGTERM with a minimised window (editor mode off afterwards).
- Seen on the amp, matching deferred minors: a cable pulled early in a store shows only
  "Amp not connected". The status line showed no hex address in these tests.
- Not looked at: whether the amp echoes app writes.

### Follow-ups from Task 9

- MOD, FX, DELAY and REVERB have no colour indicator in the GUI (only the booster
  has one); to be implemented (the user, 2026-11-06).
- A quit signal with a minimised window takes 3-8 s (by design: 3 s wait for a frame,
  then the worker stops).

### Decisions the user can still overrule

- With Live on the app re-reads the AMP block once a second (reverses the Task 2 ruling,
  because a missed amp-type report could get a wrong sound saved or stored).
- The whole central panel greys out during jobs, about 1.5 s per channel switch.
- After a failed store only the channel is reported; the editor keeps what the user
  wanted to store, and Load to amp puts it back.
- With a hidden window a quit signal exits without a final auto-save, so up to 5 minutes
  of edits could be lost.
- A panel read finishing in the same instant as a click can clear that job's lock.

The user's own decisions: Live off at every start; nothing is stored without an explicit
Store click (amp memory wear); Open with Live on plays without storing; Save and Store
stay separate; Load to amp never switches channel.

## Where things stand

All on `main` (pushed to `origin`), so the rest of this file describes `main` unless
it says otherwise:

| Phase / stage | State | Spec | Plan |
|---|---|---|---|
| Phase 1: protocol library + `katana-cli` | done, verified on the amp | `docs/superpowers/specs/2026-10-22-katana-protocol-design.md` | `docs/superpowers/plans/2026-10-22-katana-protocol.md` |
| Phase 2, stage 2 shell: offline egui editor | done; checked on Linux in a VM | `docs/superpowers/specs/2026-10-22-katana-gui-design.md` | `docs/superpowers/plans/2026-10-26-katana-gui-shell.md` |
| GUI polish (review minors, text size, 2-row top bar) | done | same | `docs/superpowers/plans/2026-10-28-katana-gui-polish.md` |
| Stage 3: preset library | done; hands-on check in the Linux VM passed 2026-10-28 | `docs/superpowers/specs/2026-10-28-katana-preset-library-design.md` | `docs/superpowers/plans/2026-10-28-katana-preset-library.md` |
| Stage 2: editor <-> amp | done, verified on the amp; user's hands-on check passed 2026-11-06 | `docs/superpowers/specs/2026-10-31-katana-editor-amp-design.md` | `docs/superpowers/plans/2026-10-31-katana-editor-amp.md` |
| Stage 1: amp control (library + CLI) | done, verified on the amp; final review clean; user's hands-on check passed 2026-10-31 (incl. B1 and the Led Zeppelin Collection set) | `docs/superpowers/specs/2026-10-29-katana-amp-control-design.md` | `docs/superpowers/plans/2026-10-29-katana-amp-control.md` |

- 225 tests pass on the Linux machine (`cargo test --workspace`, 2026-10-31); hardware
  tests: `cargo test -p katana-proto --features hardware-tests` (amp needed).
- `cargo clippy --workspace --all-targets` shows 3 known warnings (`katana-proto` capture.rs:
  `chunks_exact`, `is_multiple_of`; `katana-cli` cmd/mod.rs: `is_multiple_of`); no new ones
  should be added.
- The GUI has not yet run on the Linux machine itself (real GPU, real amp). Only the
  VM, with software OpenGL.

What the app does today, offline: edit a patch (knobs, switches, chain); Library tab
with My patches, Quick saves and imported Tone Exchange `.tsl` sets; Import…,
Export .syx…, Quick save, Save, Save as…, read-only, delete, search, sort; auto-save
and restore of the last state at start. All amp actions (Live edit, channel tabs,
Load to amp, Store to channel…) are visible but disabled. See the README's GUI
section.

## Next: work on the Linux machine with the amp

The user's decision (2026-10-28): stage 1 and the rest of stage 2 are developed
directly on the Linux machine with the amp connected. Every real amp action is
built and verified there.

1. **Stage 1, amp control groundwork** — done (merged to `main`), verified on
   the amp 2026-10-31 (docs/protocol.md, "Amp control"):
   - `katana-proto::amp`: editor mode, channel select, `store_patch` (select, write,
     store, read back), notifications; `tsl::write_tsl`.
   - CLI: `store <file> <channel> [--patch N] [--yes]` (backs up the channel and the
     current sound to `~/.local/share/katana-ts/backups/` first, then asks),
     `dump <channel> <file.syx|file.tsl>`, `tsl-list <file>`.
   - Solo delay can't be written into a stored channel either: a channel keeps its
     own (the CLI says "solo delay not stored").
   - Still open: which panel actions the amp reports in editor mode (needs the user
     at the amp); the `7F 01 01 xx` status messages are not decoded; `--no-cs`
     deferred.
   - Deferred minors from the final review: no test for a store failing after the
     patch write or after `PATCH_WRITE`, nor for the backup paths in the error text;
     the failure message lists the backups unlabelled and doesn't say the live sound
     was replaced (`katana-cli load <current-sound backup>` gets it back); `ctrlc`
     without its `termination` feature, so SIGTERM/SIGHUP mid-store can leave editor
     mode on until the amp is power-cycled.
2. **Rest of stage 2, editor ↔ amp**, built and verified on the amp on
   `feature/editor-amp` (see the top) (spec, "Editor ↔ amp model" and "Channel tabs"):
   - MIDI I/O off the UI thread (no blocking, no polling storms);
   - Load to amp, Store to channel… (asks first, backs up first), channel tabs,
     Live edit (amp wins when switched on; panel always wins);
   - quick-save unsaved editor changes before the amp's settings replace them;
   - the note "Saving will overwrite library preset <name>" when amp settings
     replaced the editor's content. The editor's origin (`editor::Origin`) already
     records which library file it came from.
3. **Alongside:** parameter coverage (~350 settings) with an automatic
   write/read-back verification tool (spec, "Parameter coverage and
   verification").
4. ~~Look at the GUI on the real GPU~~ — done 2026-10-29 on the Linux machine
   (COSMIC Wayland, GTX 1080 Ti, 2560×1440, no amp): runs without software GL,
   layout fits, auto-save works; the user's check: "works perfect".

How we have worked so far: brainstorm → written spec in `docs/superpowers/specs/`
→ plan in `docs/superpowers/plans/` → execution (the user prefers **native**
execution: one implementer, TDD per step, a ledger in `.superpowers/sdd/`, then one
fresh reviewer on the most capable model for the whole branch) → a short hands-on
check by the user → merge to `main` locally. Work happens on a feature branch.

## Things to bring to the Linux machine by hand

These are git-ignored, so `git pull` does not bring them:

- `samples/*.tsl`: five real Tone Exchange sets (Black Sabbath, Black Sabbath
  Paranoid, Black Label Society, Black album Metallica, Led Zeppelin Collection).
  They are other people's presets: keep them out of git and treat them as
  untrusted data. `katana-proto/tests/tsl_samples.rs` parses them when present and
  passes trivially when absent.
- `backups/`: the user's own, untracked; not touched by the work so far.
- `docs/*.pdf` (manuals) are git-ignored too.

## Known minor issues (deferred, not fixed)

From the stage 2 review (`feature/editor-amp`):
- The status channel goes stale with Live off (ReadSlot, Peek and LiveOff don't read the
  selector).
- A failed tab click stays highlighted, and Store preselects it.
- The store failure message always says the amp plays part of the new preset, even if
  the select failed; after a read-back mismatch it doesn't say the channel may be
  overwritten.
- An unplug early in a store shows only "not connected".
- A red Failed stays until a file action.
- Backup paths aren't shown after a successful store (the CLI prints them).
- Discard on close with replaced content shows the library file at the next start,
  depending on the last auto-save.
- Unplug and replug inside one port check: jobs time out and nothing reopens (suspected).
- The status line shows a hex address for knob mirrors.
- A DT1 in 20 00-20 01 outside `PATCH_BLOCKS` would enter the patch (suspected, none in
  the logs).
- Ledger "tests:" entries record a doc-test line (0 passed).

From the preset library review:
- A failing auto-save (e.g. disk full) retries on every frame and replaces other
  messages in the status line.
- Every save, import and delete re-reads the whole library on the UI thread; may
  stall with hundreds of patches. Files added outside the app appear only after the
  next write.
- Deleting the patch the editor came from leaves the editor clean with no file
  behind it (no "unsaved" marker).
- A missing or unreadable `autosave/state.json` while auto-saves exist falls back to
  the default preset without a message; a damaged origin file is reported as "no
  longer exists".
- An `index.json` that exists but can't be read is treated as empty without a
  message, and a later lock or pick overwrites it; a second damaged index
  overwrites `index.json.bad`.
- Read-only flags of files deleted outside the app stay in the index, so a new file
  with the same name starts locked.
- A very long Save as… name fails instead of being shortened; a set patch that
  can't be loaded shows a normal Open button until clicked; `"+7"` parses as a hex
  byte.
- Not handled by design: two app instances at once; quitting without a close
  request (SIGTERM, logout) skips the final auto-save.

From the GUI polish review:
- Trackpad leftover scroll can turn a knob one step early; the wheel latch starts
  on any wheel input.
- Export needs write permission on the folder and would overwrite a user's own
  `<name>.syx.tmp`; no test for the rename-failure path; no fsync before the rename.
- The knob's typing field keeps focus behind the discard prompt.

## Notes for development

- Linux is the only target, but the code must keep building and running on macOS
  (portable crates only: `directories`, `midir`, egui).
- The library lives in `~/.local/share/katana-ts/` (`patches/`, `quick-saves/`,
  `sets/`, `autosave/`, `index.json`). Tests never touch it; they use temp folders.
- egui's default fonts have no "●", "→", "🔒" or emoji: use words or painted shapes.
  Every widget needs an accessible name (the kittest tests find widgets by it).
- Real Tone Exchange finding: 3 of the 21 sample patches carry a 6-byte COLOR block
  (the amp's is 5); trailing zero bytes are dropped, anything else is refused
  (`katana-proto/src/tsl.rs`).
- Testing from a Mac uses a VirtualBox Ubuntu VM driven by `scripts/vm.sh` (README,
  "Testing on Linux from a Mac"). In that VM the GUI needs
  `LIBGL_ALWAYS_SOFTWARE=1` (`scripts/vm.sh run` sets it), or the window is black.
  This should not be needed on the real Linux machine.
