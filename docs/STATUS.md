# Project status and handoff

Last updated: 2026-10-28 (end of the macOS session). Read this first when picking the
work up on another machine, in particular the Linux machine with the amp.

## Where things stand

All on `main` (pushed to `origin`):

| Phase / stage | State | Spec | Plan |
|---|---|---|---|
| Phase 1: protocol library + `katana-cli` | done, verified on the amp | `docs/superpowers/specs/2026-10-22-katana-protocol-design.md` | `docs/superpowers/plans/2026-10-22-katana-protocol.md` |
| Phase 2, stage 2 shell: offline egui editor | done; checked on Linux in a VM | `docs/superpowers/specs/2026-10-22-katana-gui-design.md` | `docs/superpowers/plans/2026-10-26-katana-gui-shell.md` |
| GUI polish (review minors, text size, 2-row top bar) | done | same | `docs/superpowers/plans/2026-10-28-katana-gui-polish.md` |
| Stage 3: preset library | done; hands-on check in the Linux VM passed 2026-10-28 | `docs/superpowers/specs/2026-10-28-katana-preset-library-design.md` | `docs/superpowers/plans/2026-10-28-katana-preset-library.md` |

- 189 tests pass on macOS and on Linux (Ubuntu 26.04 arm64 VM).
- `cargo clippy --workspace --all-targets` shows 3 known warnings in `katana-proto`
  (2× `is_multiple_of`, 1× `chunks_exact`); no new ones should be added.
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

1. **Stage 1, amp control groundwork** (library + CLI, same evidence rules as
   phase 1; see the GUI design spec, "Work order", item 1):
   - editor mode on/off; select channel; read a stored channel;
   - store to channel, with an automatic backup of the target channel to disk
     first and confirmation in the CLI;
   - decode the amp's unsolicited change notifications (which panel actions are
     reported and which are not; fall back to a targeted re-read for the rest);
   - test writing the SOLO_DELAY block directly into a stored channel
     (`PATCH(n)` area), **only on the scratch channels A3, A4, B3, B4, with
     backups** (spec, "Solo delay").
2. **Rest of stage 2, editor ↔ amp** (spec, "Editor ↔ amp model" and
   "Channel tabs"):
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
4. Look at the GUI on the real GPU at 1920×1080 (the VM check used software GL).

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
