# KATANA Tone Studio — Stage 3: Preset Library Design

Date: 2026-10-28
Status: Draft — awaiting user approval
Builds on: `docs/superpowers/specs/2026-10-22-katana-gui-design.md` (the GUI design;
its "Presets on disk" section is refined and partly replaced here), and the GUI
shell and polish now on `main`.

## Intent

The user builds sounds in two ways: from scratch in the editor, and by starting
from presets other players share on **BOSS Tone Exchange**
(bosstoneexchange.com, KATANA Gen 3 section). Tone Exchange is the big one at the
start. Today the app can only open and save single `.syx` files; nothing reads
Tone Exchange downloads, and there is no place where the user's patches live.

Stage 3 gives the app a **library**: the user's own patches, the downloaded sets
they came from, a scratchpad of quick saves, and a hidden auto-save safety net.
Choosing a patch from the library loads it into the editor, so every knob,
switch and chain block shows its values.

**The unit of work is one patch** (user, 2026-10-28): one complete set of amp
settings, meant for one channel. Everything is built around picking, editing and
keeping single patches. Sending a patch to an amp channel comes later and is
built on the Linux machine with the amp (stage 1 and the rest of stage 2).

## Goals

- **L1 — Import Tone Exchange sets** (`.tsl`): keep the downloaded file in the
  library as it is, show the patches inside, and let the user pick any of them.
- **L2 — My patches:** the user's own `.syx` patches, with Save, Save as…,
  read-only lock, delete, search and sort.
- **L3 — Quick saves** as a scratchpad group inside My patches.
- **L4 — Auto-save** as a hidden safety net, and reopening the last state at start.
- **L5 — Library ↔ editor:** opening any patch (own, quick save, or picked from a
  set) loads it into the editor; Save / Quick save know which file the editor
  came from.

## Non-goals

- Anything that talks to the amp: Load to amp, Store to channel…, Live edit,
  channel tabs, sending a whole set to several channels. Those stay disabled
  here and are built on the Linux machine (user, 2026-10-28).
- Downloading from Tone Exchange inside the app (the user downloads in the
  browser and imports the file). Tone Central stays stage 4.
- Writing `.tsl` files, or editing a downloaded set.
- Showing auto-saves in the library.
- Folders, tags, ratings, memo editing.

## Anti-goals

- Losing the user's work: a delete without asking, a quick save overwriting a
  named patch, or a broken file making the library (or the app) fail to start.
- Changing a downloaded `.tsl` file.
- Importing a patch that is silently wrong: a `.tsl` patch that can't be mapped
  completely onto a KATANA Gen 3 patch must be refused with a message, not
  half-loaded.

## Constraints

- Everything in the GUI design's Constraints section still applies: Rust, egui,
  Linux target with development on macOS (portable code only), clean-room rule,
  1920×1080 target window (minimum now 1200×720).
- Patches on disk are plain `.syx` in the format `katana-cli dump` writes, so the
  CLI and the app share files.
- No amp I/O in this stage.

## What a Tone Exchange `.tsl` file is (observed in 5 samples, `samples/`)

- JSON: `{"name": <set name>, "formatRev": "0000", "device": "KATANA Gen3",
  "data": [[ <patch>, … ]]}`. `data` holds one list; it held 1, 2, 4, 4 and 10
  patches in the samples. Patches are an ordered list with **no channel labels**,
  and a set can hold more patches than the amp has channels.
- Each patch: `{"memo": <string, or {"memo": …, "isToneCentralPatch": …}>,
  "paramSet": { "<block>": [<hex byte strings>], … }}`.
- `paramSet` has 80 blocks named `PATCH%<BLOCK>`, e.g. `PATCH%COM` (16 bytes: the
  patch name in ASCII, space padded), `PATCH%AMP` (10), `PATCH%BOOSTER(1)` (8),
  `PATCH%FX_DETAIL(1)` (225), … `PATCH%PATCH_KNOB_SOLO_DELAY_READONLY` (1). The
  80 names match the blocks of `katana_proto::device::PATCH_BLOCKS` (named there
  only in comments today).
- Hex bytes appear in both upper and lower case (`"7a"` in one sample).
- The set name and the patch names can differ from the file name.

## Decided: how it works

### Library tab — layout C (chosen by the user)

Mockup: `.superpowers/brainstorm/…/library-layout.html`, option C.

- A bar on top: **Search**, **Sort** (date or name, ascending/descending),
  **Import…**.
- **Two lists side by side**, so it is clear what is the user's and what was
  downloaded:
  - **My patches** — the user's `.syx` patches. Inside it, **Quick saves** is its
    own group, separate from the named patches.
  - **Downloaded sets** — the imported `.tsl` files, by set name. A set opens in
    place to list its patches by name.
- Search applies to both lists: space-separated words; an item matches if its
  name contains **any** of them (from the GUI design). A set also matches if one
  of its patch names does.

### Picking a patch from a downloaded set (user)

- Creates a new `.syx` in My patches, named after the patch (trailing spaces
  trimmed). If that name is taken, a number is added: `Iron Man.syx`, then
  `Iron Man (2).syx`, `Iron Man (3).syx`.
- Loads it into the editor (the editor now "comes from" that new file).
- Picking the same patch again makes another copy; handling duplicates is up to
  the user (user). The downloaded set is never changed.

### Opening and importing

- Opening a patch from My patches (or a quick save) loads it into the editor; the
  editor comes from that file.
- **Import…** takes `.syx` and `.tsl` from anywhere. A `.syx` is copied into My
  patches (numbered name if taken); a `.tsl` is copied into Downloaded sets
  unchanged.
- Opening anything into the editor while it has unsaved changes asks first
  (the existing discard prompt).

### Saving (user, and the GUI design)

- **Quick save** (user):
  - If the editor came from a quick save, it overwrites that file.
  - Otherwise (default preset, a named patch, a patch picked from a set) it
    creates a new file `quick-save_<timestamp>.syx` in the Quick saves group, and
    the editor then comes from it. Quick save never overwrites a named patch.
  - This replaces the GUI design's `quicksave001.syx` numbering.
- **Save:** overwrites the file the editor came from. Disabled when that patch is
  read-only (Save as… offered instead). When the editor comes from no file (the
  default preset), Save acts as Save as….
- **Save as…:** asks for a name, creates a new named patch in My patches
  (numbered if taken), and the editor comes from it. A quick save stays where it
  is until the user deletes it.
- **Export .syx…** (replaces today's Save .syx…): writes a copy anywhere, e.g. to
  share. Today's **Open .syx…** becomes Import….

### Names (user, 2026-10-28)

- A patch has two names: its **file name** and the **name stored inside the patch**
  (`PATCH%COM`, at most 16 ASCII characters).
- The library lists, the search, the sort, the delete question and the editor's
  top bar all use the **file name** (without `.syx`), so the user always sees one
  name.
- **Save as…** names the file and also writes the typed name into the patch,
  cut to 16 characters (characters outside printable ASCII replaced).
- Numbered copies (`Iron Man (2).syx`) keep the inner name they came with.
- File names stay readable: spaces and parentheses are kept; only characters
  that can't be in a file name are replaced. "Name taken" ignores letter case
  (macOS file systems do).
- A file from the default preset (no file yet) shows the inner name, as today.

### Read-only and delete (user)

- A patch in My patches can be locked read-only and unlocked.
- **Delete** works on a patch, a quick save, or a whole downloaded set, and always
  asks first ("Delete Iron Man (2)?"). A read-only patch can't be deleted until
  it is unlocked. Deleting a set leaves the patches already taken from it.

### Auto-save (user: hidden safety net)

- About every 5 minutes, only if the editor changed since the last auto-save,
  the editor's state is written to a hidden auto-save folder; the last 10 are
  kept, oldest overwritten.
- Not shown in the library. Reachable with Import… / the file dialog if ever
  needed.
- At start the app shows **exactly the state at the last quit** (Claude's call,
  from the anti-goal "never lose work"): an auto-save is also written when the
  app closes and after every Save / Save as… / Quick save, so the newest
  auto-save is never older than the user's last save. After "Discard" at close,
  the state at quit is the file the editor came from, not the discarded edits.
  The auto-save records which file the editor came from (so Save and Quick save
  still point at the right file), and the unsaved marker reflects whether it
  differs from that file. With no auto-save, the default preset as today.

## Calls made by Claude (not decided by the user)

- Quick-save timestamp format: `quick-save_2026-10-28_15-04-12.syx` (local time).
- Numbered names: ` (2)`, ` (3)`, … before `.syx`.
- Library location: the XDG data folder, `~/.local/share/katana-ts/`, with
  subfolders `patches/`, `quick-saves/`, `sets/`, `autosave/`. Found through a
  portable crate (e.g. `directories`), so development on macOS works too.
- Metadata (read-only flag, where a patch came from — set name and patch name —,
  dates) in one small index file in that folder. A patch without an index entry
  is still listed (date from the file).
- Robustness: an unreadable library folder, a broken `.syx` or `.tsl`, or a broken
  index never stops the app; bad items are skipped and named in the status line.
- A `.tsl` patch whose blocks don't all match `PATCH_BLOCKS` by name and size is
  refused with a message naming the patch (see Anti-goals). Blocks the app
  doesn't recognise make the patch refused, not ignored.
- Opening the auto-save at start never asks anything.
- Read-only is a flag in the index only; files keep normal permissions, so
  `katana-cli` is unaffected. Quick saves can be locked too. If the editor came
  from a locked quick save, Quick save creates a new quick save instead of
  overwriting it.
- Import edge cases: a `.tsl` whose `device` isn't `"KATANA Gen3"` is refused at
  import. A set with one patch that can't be mapped is still imported; that
  patch is refused (with a message) when the user picks it.

## Left to the builder

- The exact `.tsl` → `Patch` mapping code, and giving `PATCH_BLOCKS` real names.
- Index file format (e.g. JSON or TOML) and how it is kept in step with the files.
- How the editor records "came from" (path plus kind: patch / quick save / none).
- Widget details of the two lists, the set fold-out, the delete confirmation and
  the Save as… name prompt; how Quick saves are shown as a group.
- Auto-save timer mechanics, and tests that don't wait 5 minutes.
- Moving today's Open .syx… / Save .syx… code into Import… / Export .syx….

## Open questions

None blocking.
