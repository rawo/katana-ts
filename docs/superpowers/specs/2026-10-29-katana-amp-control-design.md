# KATANA Tone Studio for Linux — Stage 1: Amp control (CLI) design

Date: 2026-10-29. Refines the GUI design spec (`2026-10-22-katana-gui-design.md`),
"Work order", item 1.

## Intent

Today `katana-cli` changes the amp one setting at a time (`set`), or copies a whole
patch to and from the amp's **current** patch (`load`, `dump`). The user wants to
move **whole patches between files and the amp's stored channels** (A1–A4, B1–B4)
from the terminal:

1. patch *N* of a Tone Exchange `.tsl` set → a channel;
2. a `.syx` patch file → a channel;
3. a channel → a `.syx` or `.tsl` file, the format picked by the file's extension.

This is also the groundwork the GUI's Load to amp / Store to channel… / channel tabs
will build on (stage 2), so the protocol work goes into `katana-proto`, and the CLI
is a thin layer over it.

## Goals

- `store`, `dump <channel>`, and `tsl-list` commands as below, working on the user's
  amp, verified with the same evidence rules as phase 1.
- Writing a one-patch `.tsl` file (reading exists already, `katana-proto/src/tsl.rs`).
- In `katana-proto`: editor mode on/off; select a channel; read a stored channel;
  store into a channel; decode the amp's unsolicited messages well enough to know a
  channel switch or store happened (the rest of the notification decoding feeds
  stage 2).
- The SOLO_DELAY test from the GUI spec ("Solo delay"): does writing that block
  straight into a stored channel's area work? Result recorded in `docs/protocol.md`
  and acted on (below).

## Non-goals

- Editing file contents. The CLI only reads a file, or writes a whole file. No
  "change one setting inside a .syx/.tsl".
- Appending to a `.tsl` set, or writing multi-patch sets. One patch → one file.
- The GUI side (stage 2), parameter coverage (separate track), SYSTEM settings,
  PEDAL FX, the PANEL channel as a store target.

## Anti-goals

- Overwriting a stored channel without a backup on disk first.
- Overwriting a stored channel or an existing file without asking (unless `--yes`).
- Changing the behaviour of any existing command. `ports`, `identity`, `monitor`,
  `extract`, `diff`, `dump <file>`, `load`, `verify`, `set`, `params`, `watch` stay
  exactly as they are — the user still uses single-parameter `set`.
- Reporting success for something that didn't land (e.g. solo-delay settings, if the
  amp can't take them).
- Leaving the amp in editor mode after the command ends, including on error or
  Ctrl-C (best effort for Ctrl-C).

## Commands (decided with the user; spelling of names is Claude's proposal, accepted)

```
katana-cli tsl-list  sabbath.tsl                 # numbered list of the patches in a set
katana-cli store     sabbath.tsl A3 --patch 4    # patch 4 of the set → channel A3
katana-cli store     mine.syx    B4              # .syx → channel B4
katana-cli dump      A3 crunch.tsl               # channel A3 → file; extension picks format
katana-cli dump      crunch.syx                  # no channel = current patch (unchanged)
```

- **Channel names:** `A1`–`A4`, `B1`–`B4`, case-insensitive.
- **Format by extension:** `.syx` or `.tsl` (case-insensitive); anything else is an
  error naming the two allowed extensions.
- **`tsl-list`:** one line per patch: number (from 1), patch name. Works without the
  amp.
- **`store <file> <channel>`:**
  - `.tsl` source needs `--patch N` (from 1, the order `tsl-list` shows), **even when
    the set holds one patch**. `--patch` with a `.syx` source is an error.
  - Reads the target channel first and **saves it as a backup** (`.syx`) before
    anything is written. Backup folder: `~/.local/share/katana-ts/backups/`, file
    name `<YYYY-MM-DD_HH-MM-SS>_<channel>_<patch name>.syx` *(location and naming:
    Claude's call)*. The command prints the backup's path.
  - Asks `A3 currently 'Riff Shred' → replace with 'Iron Man'? [y/N]`; default no.
    `--yes` skips the question. Without a terminal on stdin and without `--yes`, it
    refuses rather than guessing.
  - Writes the patch, then **reads the channel back and compares** (excluding
    whatever the amp is known not to store, e.g. solo delay if the test fails); a
    mismatch is an error that prints the backup path.
  - Before switching, it also **backs up the amp's current sound** (the current
    patch, including knob changes never stored to a channel) to the same backup
    folder, and prints that path *(Claude's call)*.
  - **Afterwards the amp switches to that channel** so the user hears it
    *(decided by the user)*.
  - **Deferred:** `--no-channel-switch` / `--no-cs` to stay on the channel the amp
    was on. The user wants it eventually, but skipped it for stage 1 (2026-10-29):
    "if switching to a given channel will be annoying then we will get back to this".
- **`dump <channel> <file>`:** reads the stored channel (no channel switch, no
  audible change) and writes the whole file. If the file exists: `Overwrite
  crunch.tsl? [y/N]`, `--yes` skips. The existing one-argument `dump <file>` (current
  patch, `.syx`) keeps working unchanged.

## What's known about the amp (from `docs/protocol.md`, "Amp control")

- Editor mode: DT1 `7F 00 00 01` ← `01`/`00`. While on, the amp sends unsolicited
  DT1s (selector after a channel change, `PATCH_WRITE` echo after a store, a stream
  of `7F 01 01 xx` status messages not yet decoded).
- Channel select: Program Change *n* → selector *n*+1, works without editor mode;
  `PATCH_SELECT` `7F 00 01 00` works only in editor mode.
- Store: `PATCH_WRITE` `7F 00 01 04` ← `00 <selector>` copies the **current patch**
  into a channel, only in editor mode. Verified on A4 on 2026-10-22.
- Stored channels can be read with RQ1 without editor mode (`PATCH(n)` area).
- The current patch's SOLO_DELAY block can't be written.

## Left to the builder

- **How `store` writes.** Two routes: (a) load the patch into the current patch,
  then `PATCH_WRITE` it into the channel; (b) DT1 straight into the channel's
  `PATCH(n)` area. Route (a) is proven; route (b) is untested except as the
  SOLO_DELAY test — prove it first on scratch channels. Since `store` always ends on
  the target channel, either route is acceptable.
- **Solo delay:** if a direct write of SOLO_DELAY into `PATCH(n)` works, `store`
  writes it there and the read-back covers it. If not, `store` prints "solo delay
  not stored" *(accepted by the user)*, and the read-back excludes it. In that
  case `store` switches to the target channel **before** loading the patch (it
  ends there anyway), so with route (a) the channel keeps its own previous solo
  delay rather than taking the one of whatever was playing *(Claude's call)*.
- `.tsl` writing: match the shape of real Tone Exchange files closely enough that
  this app's parser reads it back to an identical patch (test: write → parse →
  compare). Whether the official Tone Studio imports it is a by-hand check by the
  user. The five real sets are on this machine in `samples/*.tsl` (git-ignored,
  other people's presets: never commit them; tests pass trivially without them).
- Which unsolicited messages to decode beyond what `store` needs; record findings in
  `docs/protocol.md`.
- Settle delays, retries, timeouts, error wording.

## Constraints

- Rust, existing crates (`katana-proto`, `katana-cli`); portable (must still build
  on macOS); no new heavy dependencies.
- Hardware rules (agreed 2026-10-22): read-only probes are fine; live-patch writes
  that are restored straight away are fine after saying what will be heard; **stored
  channel writes during development only on A3, A4, B3, B4, after backing them up**.
  The finished `store` accepts any of A1–B4 — the safety is the backup + question.
- Unit tests use the fake transport (`katana-proto/src/fake.rs`); hardware
  acceptance checks are run by the user.
- Workflow as before: plan → native execution on a feature branch, TDD, one fresh
  reviewer → user's hands-on check → merge to `main` locally.

## Definition of done

On the user's amp: `tsl-list` on a real set; `store` from `.tsl` and from `.syx`
into a scratch channel, with backups, question, read-back, and channel switch; `dump A3 x.tsl` and `dump A3 x.syx`, each
loading back via `store` to a byte-identical channel; restoring a scratch channel
from its backup with `store`; by-ear check by the user; SOLO_DELAY result recorded.
All existing commands and tests unchanged.
