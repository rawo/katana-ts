# KATANA Tone Studio for Linux — Phase 2 (GUI) Design

Date: 2026-10-22
Status: Draft — awaiting user approval
Builds on: `docs/superpowers/specs/2026-10-22-katana-protocol-design.md` (phase 1, done),
`docs/protocol.md` (verified protocol facts and amp quirks).

## Intent

The user owns a BOSS **KATANA-100 Gen 3** and runs Linux. The official BOSS TONE
STUDIO does not run there (not even under Wine), so today they can only reach what
the amp's physical panel exposes. Most of the amp's sound — booster/mod/FX/delay/
reverb types and their detailed parameters, EQ, noise suppressor, chain order — is
unreachable. Phase 1 proved we can read and write all of it over USB.

Phase 2 is a native Linux desktop app that lets the user **shape sounds with every
setting the original Tone Studio offers, hear changes live, store results into the
amp's channels, and keep their work as presets on disk**. It is a personal tool for
the user (one person, one amp, 1920×1080 screen). Visually it is *similar in spirit*
to the original, not a copy.

### The user's evening, end to end

1. Open the app. The editor shows the last preset they worked on (or a default).
2. Explore booster and EQ settings the panel can't reach; or open a preset from the
   library into the editor.
3. Turn **Live edit** on to hear changes as they happen; the panel knobs keep working
   and the app follows them.
4. Quick-save every so often (and auto-save covers them anyway).
5. When happy, **Store to channel** (e.g. A2) so the sound lives on the amp.

## Goals

- **G1 — Full patch editor** for every *patch* setting the original Tone Studio
  offers on a KATANA-100 Gen 3, *excluding* footswitch / GA-FC / expression-pedal
  assignments and PEDAL FX (see Non-goals).
- **G2 — Live sync** with the amp under the rules in "Editor ↔ amp model" below.
- **G3 — Amp control:** switch the active channel; store the editor's sound into a
  channel; read every stored channel.
- **G4 — Never lose work:** quick save, save, save as, auto-save, open `.syx`.
- **G5 — Preset library** on disk with search, sort, read-only presets.
- **G6 (nice to have) — Tone Central** download of presets.

## Non-goals

- Footswitch, GA-FC, expression pedal, MIDI CC/PC assignment editing.
- **PEDAL FX** (pedal wah / bend; needs an expression pedal the user doesn't have)
  and the **amp-wide SYSTEM settings** (global EQ, line-out mic/ambience, USB levels,
  power-amp-in, stereo expand, half-power; ~78 settings): *to be implemented if
  needed* (user, 2026-10-22). The device description still records their addresses
  so they can be added later.
- Other Katana models or other BOSS devices *now* (but see "Device description").
- Audio playback/looper, firmware update, all-data backup of system settings.
- Multi-user polish, packaging for distribution, Windows/macOS.
- Pixel-matching the original app; reusing any of its graphics.

## Anti-goals (would be a failure even if it "works")

- Silently losing the user's edits (a sync, channel switch or connect wiping unsaved
  work with no copy kept).
- Writing to the amp's **stored channels** without an explicit user action and
  confirmation.
- The app fighting the physical panel (pushing its value back after a knob turn).
- A UI that says "saved/stored/synced" when the amp didn't actually take it.
- Shipping parameter definitions nobody checked against the amp.

## Constraints

- Rust; builds on the phase 1 crates `katana-proto` (protocol) and `katana-cli`.
- GUI toolkit: **egui** (eframe). Decided.
- Linux, amp on USB as class-compliant MIDI (`KATANA3`, via `midir`/ALSA).
- Development also happens on macOS; code stays portable (no OS-specific code),
  Linux remains the only target.
- Target window 1920×1080; main editor fits without scrolling.
- Clean-room rule from phase 1 stays: data/constant tables from the official app may
  be *read* as facts, no logic is copied, and nothing is trusted until checked on the
  amp. Known amp quirks (`docs/protocol.md`) must be respected:
  - current-patch **SOLO_DELAY** block is write-protected (skip on write);
  - **colour/variation change** is applied asynchronously and re-derives effect
    on/off from the knob byte — wait ≥300 ms after a colour write before writing
    on/off;
  - **channel select** works via MIDI Program Change (PC *n* → selector *n*+1) or
    `PATCH_SELECT` in editor mode; **store** (`PATCH_WRITE 7F 00 01 04`, data
    `00 <selector>`) works **only in editor mode** (`7F 00 00 01` = 1); in editor
    mode the amp **pushes change notifications** by itself.

## Decided: how it works

### Work order (stages)

Each stage ends working and verified on the user's amp before the next starts.

1. **Amp control groundwork** (library + CLI, same evidence rules as phase 1):
   editor mode on/off; select channel; store to channel (with automatic backup of the
   target channel to disk first, and confirmation in the CLI); read a stored channel;
   decode the amp's unsolicited change notifications (which panel actions are
   reported, which aren't — fall back to a targeted re-read for the rest).
   Also: **test writing the SOLO_DELAY block directly into a stored channel**
   (`PATCH(n)` area) — only on the user's scratch channels A3, A4, B3, B4, with
   backups (see "Solo delay").
2. **Editor GUI (layout B) + live sync**, with basic **Open .syx / Save .syx** so no
   work is lost before the library exists.
   *Stage 2 shell (offline, no amp I/O): verified on Linux 2026-10-28* — in a
   VirtualBox VM (Ubuntu 26.04 arm64, GNOME Wayland, 1920×1080, software GL): all
   tests pass; the Editor tab fits without scrolling; Open/Save .syx dialogs work
   through the portal; closing with unsaved changes asks first. Still to confirm on
   the Linux machine itself with its real GPU.
3. **Preset library**: quick save, auto-save, save, save as, read-only, search, sort.
4. **Tone Central** (nice to have).

### Layout B — "signal chain" with tabs (chosen by the user)

Mockup: `docs/superpowers/mockups/2026-10-22-main-layout.html` (option B; the channel row there is superseded by "Channel tabs").

- **Top bar:** connection status; preset name + "unsaved" marker; **Live edit**
  switch; Quick save, Save, Save as…, Load to amp, Store to channel…
- **Channel tabs** (always visible): PANEL, A1–A4, B1–B4 — see "Channel tabs".
- **Tabs:** Editor | Library | Tone Central (the last only in stage 4).
- **Editor tab:** the **signal chain** drawn in processing order (guitar → … → amp …
  → reverb → output, following the patch's chain setting), each block showing on/off,
  type, and green/red/orange variation lights; clicking a block opens its full
  parameters in a **detail panel**; an **amp strip** is always visible.
- **Status line:** amp channel, live on/off, last auto-save time, last panel change
  (e.g. "GAIN 95 → 40").

### Controls and look

- Dark theme by default, own visual style, no copied graphics. **Colour themes**
  selectable (egui supports custom palettes).
- Knobs: drag up/down; wheel = fine step; double-click = type a value; value always
  shown. Named values (amp type, effect type, colour, chain) are buttons/dropdowns.
- Something on screen always states: connected or not, live on/off, unsaved changes,
  last panel change.

### Editor ↔ amp model (decided by the user)

The **Live edit switch is the only thing that syncs** editor and amp.

| Situation | Editor | Amp |
|---|---|---|
| App start | last edited preset; if none, **default preset** (everything off / 0, amp type CLEAN) | — |
| Live **off** (connected or not) | the user's working copy; the amp never changes it — except that clicking a channel tab loads that channel from the amp (see "Channel tabs") | changed only by explicit actions: **Load to amp**, **Store to channel** |
| Live switched **on** | **takes the amp's current live settings (amp wins)**, then stays synced: editor changes go to the amp immediately; panel changes and channel switches on the amp come back into the editor | follows the editor |
| Amp disconnected | last edited; Live switch disabled | — |
| Amp reconnects | if Live was on: sync again, amp wins. If off: nothing happens | — |

- **Live is off by default.** It remembers its position across disconnects (user
  confirmed).
- **Panel always wins:** a panel change overwrites the app's value; the app never
  pushes it back.
- Before the amp's settings replace the editor (Live on, reconnect with Live on, channel-tab
  switch), unsaved editor changes are **quick-saved first**. (Same for channel-tab switches.)
- The editor remembers which library preset it came from. When its content no longer
  matches that preset because the amp's settings replaced it, show: *"Saving will
  overwrite library preset <name>"*. If that preset is read-only: Save disabled,
  Save as offered. If the preset is not in the library: no note.
- **Store to channel** writes the editor's preset into the selected channel tab's
  channel; always asks for confirmation and first backs up that channel to disk.

### Channel tabs (decided by the user)

The editor always edits **one preset**. It may belong to a channel or not (e.g.
opened from the library, or the default preset — then no tab is selected).

- Clicking a channel tab, **amp connected**: the editor loads that channel's stored
  settings from the amp (one stored-slot read, ~80 block requests).
  - **Live off:** view only — the amp keeps playing its current channel. The user
    may be playing the amp while browsing channels in the app.
  - **Live on:** the amp also switches to that channel; sync continues on it.
- Clicking a channel tab, **amp not connected**: the editor shows what was last
  edited for that channel. Each channel keeps its own last-edited copy on disk.
- **App start:** reopen the last edited preset *and* the channel tab it was on.
- Switching tabs replaces the editor's content, so unsaved changes are quick-saved
  first.
- Stored channels are read on demand (tab click), after our own stores, and when
  notifications say they changed — never by continuous polling (reading all nine
  takes ~15 s).

### Presets on disk

- Each preset is a plain **`.syx`** (same format `katana-cli dump` writes), so CLI and
  app share files. Metadata (name, read-only flag, source, timestamps) lives next to
  it or in one index file for the library.
- **Quick save:** one click → `quicksave001.syx`, `002`, … in the library folder.
- **Save as:** asks for a name. **Save:** overwrites the current preset (blocked if
  read-only).
- **Auto-save:** about every 5 minutes, **only if something changed**, rolling history
  of the last ~10 (oldest overwritten). Saves the editor's current state, including
  values that came from the panel.
- **Library tab:** search by name where the query is space-separated words and a
  preset matches if its name contains **any** of them; sort by date or name,
  ascending/descending; lock/unlock read-only; open into editor; import a `.syx` from
  anywhere.

### Device description (extensibility)

Only the KATANA-100 Gen 3 is supported now, but nothing model-specific is hard-coded
in the GUI. Everything model-specific — model ID, address map, parameter list with
encodings/ranges/value names, block structure, write-protected ranges, settle
delays, channel layout, command addresses — lives in one **device description** that
the protocol library and GUI consume. Adding a model later means adding a
description. (Phase 1's `device.rs`/`params.rs` are the seed of this.)

### Parameter coverage and verification

- Coverage: every patch parameter of the official app's address map for this model,
  minus `ASSIGN_*` (pedal/footswitch) and PEDALFX blocks — about 350 settings,
  including each effect type's own parameter set (booster, mod, FX, delay, reverb
  types; EQ parametric/graphic; NS; send/return; solo; contour; chain). The
  official app's data tables also describe which parameters each effect type shows
  and how (`export/item.json`, `export/layout.div` in its unpacked UI); these may be
  read as data like the address map.
- **Automatic verification on the amp** (decided): a tool writes each parameter at
  min / middle / max, reads it back, restores the original; failures are flagged in
  the device description like SOLO_DELAY. Ear checks for a sample per block.
  Results are recorded as evidence, as in phase 1.

### Solo delay (builder's call recorded by Claude, not decided by the user)

The current patch's SOLO_DELAY block can't be written, so Load to amp / Store to
channel can't carry solo-delay settings as designed. Stage 1 tests writing that
block straight into a stored channel's area. If it works, Store also writes it
there. If not, the editor shows solo-delay settings as *"can't be sent to the amp"*,
and post-store checks exclude them so the UI never claims they were stored.

## Left to the builder

- Exact widget styling, knob rendering, the built-in theme set.
- How the GUI threads MIDI I/O vs UI (must not block the UI; no polling storms).
- Exact auto-save interval/history size within "≈5 min / ≈10".
- Library folder location (XDG data dir is the natural choice) and metadata format.
- How change notifications are mapped to GUI updates; fallback re-read cadence.
- Default-preset exact values beyond "everything off / 0, amp type CLEAN".
- Tone Central integration details (stage 4; investigate the service when reached).

## Open questions

None blocking. Resolved in review (2026-10-22): SYSTEM settings and PEDAL FX are
deferred; stored channels are shown through channel tabs, not a separate view;
clicking a tab switches the amp only when Live is on.
