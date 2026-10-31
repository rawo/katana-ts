# KATANA Tone Studio — Stage 2: editor ↔ amp

Date: 2026-10-31. Status: approved by the user 2026-10-31.

Refines the GUI design spec (`2026-10-22-katana-gui-design.md`, "Editor ↔ amp
model" and "Channel tabs"). Where the two differ, **this document wins**; the
differences are listed at the end. Builds on stage 1 (`katana-proto::amp`,
`docs/protocol.md` "Amp control") and stage 3 (preset library).

## Intent

Make the app usable with the amp: see what the amp is playing, try library
patches on it, tweak with the sound following the app, and keep results —
in files and in the amp's channels — only when the user says so.

The user's two evenings (their words, condensed):

1. **Try patches.** Open the app with the amp connected; see the settings the amp
   is playing on its current channel. Go through the library (own patches and
   Tone Exchange sets) and load one. Play, adjust. Save — mostly as a new file.
2. **Work on the live sound.** Open the app; see the current settings. Modify them
   live. Save them as a new patch file *and* keep them on a chosen channel in the
   amp.

At first the user will mostly put existing Tone Exchange patches into channels;
later they will tweak and keep their own.

## Goals

- G1 — With Live edit on, the app and the amp show and play the same settings, as
  closely as the amp allows, in both directions.
- G2 — Library patches can be heard on the amp straight away.
- G3 — Storing into a channel is always an explicit, confirmed action with a backup.
- G4 — No editor work is lost when the amp's settings replace it.
- G5 — The app never freezes while talking to the amp.

## Non-goals

- Parameter coverage beyond today's parameters (~350 settings, write/read-back
  verification tool) — the next project, after this stage.
- Tone Central (stage 4). SYSTEM settings, PEDAL FX (deferred in the GUI spec).
- Writing the solo delay (the amp refuses it over MIDI, current patch and stored
  channels alike — `docs/protocol.md`).
- A combined "save and store" action.

## Anti-goals (a failure even if it works)

- **Anything stored into a channel without the user clicking Store.** No automatic
  store, ever — not on Open, not on tweaks, not with Live on. Reason (the user's):
  every store writes the amp's internal memory, which wears; the user doesn't want
  to wear it unintentionally.
- Save (to a file) and Store (to a channel) triggering each other. They are two
  different actions with two buttons.
- Editor changes lost because the amp's settings replaced them.
- The UI freezing during a channel read (~1.5 s) or a patch write.
- Polling storms: reading all channels continuously, or re-reading the whole patch
  many times a second.

## Decided by the user

### Live edit off (the default)

- **Live edit is off at every app start.** The app opens the last edited preset
  (and its channel tab), exactly as today. The amp is not touched.
- Editing, Open, Save, Save as, Quick save act on the app only.
- **Load to amp:** the amp plays the editor's preset now, on whatever channel the
  amp is on — it never switches the amp's channel, even when the app shows another
  channel's tab (the user's choice, 2026-10-31); nothing is stored.
- **Store to channel…:** asks first, backs up the target channel to disk first,
  then stores the editor's preset into that channel.
- **Clicking a channel tab** loads that channel's *stored* settings from the amp
  into the editor, view only: the amp keeps playing what it was playing.
- The app and the amp are not kept in sync; the user syncs them by hand with Load
  and Store.

### Live edit on

- **Switching Live on:** the app reads the amp's **current settings** on its active
  channel (what is being heard, including panel tweaks), shows them in the editor,
  and selects that channel's tab. The amp wins.
- **While on:**
  - every app edit is sent to the amp's current settings at once (heard at once);
  - panel tweaks and channel switches made on the amp show up in the app;
  - **nothing is stored into a channel** — as on the amp itself, a tweak is kept
    only if stored.
- **Open from the library** (own patch, quick save, or a patch from a set): the
  preset goes into the amp's current settings on whichever channel the amp is on
  *at that moment* and is heard at once; not stored. If the amp's channel changed
  meanwhile (example: app on A1, amp switched to B2 on the panel), the app shows
  the new channel as active (B2) and the preset lands there.
- **Clicking a channel tab:** the amp switches to that channel too; the editor shows
  its current settings.
- **Store to channel…** works exactly as with Live off: explicit, asks, backs up.

### Both modes

- Save and Store are separate buttons; neither triggers the other. The user's
  second evening is: Save as (new file), then Store to channel (two clicks).
- Before the amp's settings replace the editor's content (Live switched on, channel
  tab click, a channel switch on the amp, reconnect with Live on), **unsaved editor
  changes are quick-saved first** (GUI spec, unchanged).
- The note *"Saving will overwrite library preset <name>"* when the amp's settings
  replaced the content of an editor that came from a library preset (GUI spec,
  unchanged; `editor::Origin` already records the file).
- Amp disconnected: Live switch, channel tabs' amp actions, Load and Store
  disabled (as today). Live remembers its position across a disconnect; on
  reconnect with Live on: sync again, amp wins (GUI spec, unchanged).

## Decided by Claude (the user may overrule)

- **MIDI off the UI thread.** One background worker owns the MIDI connection and
  runs one amp job at a time (read current settings, read a channel, write a
  preset, send one parameter, select a channel, store). The UI sends requests and
  receives results and amp messages through channels; it never waits on MIDI. While
  a long job runs (channel read, preset write, store), the status line says so and
  the actions that would conflict are disabled.
- **Live edits are sent per parameter** as DT1s, in the order the user makes them;
  a knob drag sends its latest value at a modest rate (coalesced, not every pixel).
  The ordering rules from `docs/protocol.md` apply (variation colour before on/off,
  `device::SETTLE_AFTER`). Nothing is stored.
- **Editor mode** (`amp::EditorSession`) is on while Live is on, and for the
  duration of a select or store job otherwise; turned off when Live is switched off
  and on quit.
- **Store to channel… dialog:** a channel picker (A1–B4; PANEL is not a store
  target), preselected to the active channel tab when that is A1–B4; shows "<channel>
  currently '<old name>' → replace with '<new name>'?"; backs up the target channel
  and the current settings with the CLI's backup code (same folder,
  `~/.local/share/katana-ts/backups/`, same names), then `amp::store_patch`. Says
  "solo delay not stored" after a store, as the CLI does. On failure: names the
  backups and says the live sound was replaced (the current-settings backup brings it
  back) — this fixes a deferred minor from stage 1 for the GUI.
- **Store leaves the amp on the stored channel** (that is how `store_patch` works:
  it selects the channel first). With Live on, the app follows; with Live off, the
  app keeps showing the editor's preset and the tab of the stored channel.
- **Load to amp / Open with Live on** use `write_patch` (current settings, verified).
- **Panel changes with Live on — how they're noticed is settled by a hardware check
  early in the work, with the user at the amp:** with editor mode on, the user
  turns every panel knob, button and the channel switches while the app logs what
  the amp sends by itself. Then:
  - reported changes update the editor directly;
  - for what isn't reported, a **light re-read** of only the panel-reachable
    blocks (amp, effect on/off and colour, solo, contour — a few RQ1s) on a slow
    timer (≈ once a second) while Live is on; the whole patch is never polled.
  - A channel switch on the amp (the selector message, verified in stage 1)
    triggers a re-read of the current settings.
  The check's results go into `docs/protocol.md`.
- **Connection** is detected by watching the MIDI port list (cheap; no amp traffic),
  not by polling the amp.
- **Unsaved state when the amp's settings come in.** When the amp's settings replace
  the editor (Live switched on, tab click, channel switch on the amp, reconnect with
  Live on), they become the editor's new clean baseline: browsing channels never
  makes quick saves by itself. The editor's origin (library file) is kept, which is
  what shows the "Saving will overwrite library preset <name>" note; Save stays
  enabled then (blocked only when that file is read-only) and writes the editor's
  content into the file. With Live on, a **panel tweak** reported by the amp marks
  the editor unsaved, like an app edit — so it is quick-saved before the next
  replacement and covered by auto-save (G4).
- **Channel tabs with Live off** mean "the channel shown in the editor", not "the
  channel the amp plays". After Open, Save as or Quick save the tab is cleared (the
  editor no longer shows a channel's stored settings), as the GUI spec has it for
  presets that don't belong to a channel. The status line says which channel the amp
  is on (the selector, read with one RQ1 after each amp job and on connect; kept up
  to date by notifications while editor mode is on).
- **PANEL tab.** PANEL is never a store target. With Live on, the app follows the amp
  onto PANEL when the user selects it on the amp (editor shows the current settings),
  and clicking PANEL in the app selects it on the amp if the hardware check shows
  `PATCH_SELECT` ← `00 00` works; otherwise the PANEL tab is follow-only. With Live
  off, clicking PANEL shows the amp's stored PANEL slot, view only, like any channel.
  `katana-proto` gains PANEL only where needed (selector value 0), not as a store
  target.
- **Editor mode and the panel.** The hardware check also confirms that with editor
  mode on the panel knobs, buttons and channel switches still work normally. If they
  don't, Live is redesigned before the worker is built (channel select by Program
  Change, selector read instead of notifications) and the user is told. The app
  turns editor mode off on quit, on SIGTERM/SIGHUP, and sends "editor mode off" once
  when it connects (clears a mode left on by a crash).
- **Solo delay.** The editor has no solo-delay controls today (parameter coverage
  comes later), so nothing to disable. With Live on the editor holds the amp's solo
  delay; Save writes it into the file — the file matches what was heard.
- **Live is off at start even if it was on when the app was closed** (the user's
  rule 1); "Live remembers its position" applies to disconnects within a session.

## Constraints

- Linux is the target; keep building on macOS (portable crates only).
- No new dependencies beyond what's needed; `std::sync::mpsc` and a thread are
  enough for the worker.
- Tests never touch the amp or the user's library; GUI tests use `egui_kittest`
  and `katana_proto::FakeAmp` behind the worker. Hardware checks are run with the
  user, on scratch channels A3, A4, B3, B4 with backups.
- egui's default fonts: no "●", "→", "🔒" or emoji in the UI (use words or painted
  shapes). Every widget keeps an accessible name.
- `cargo clippy --workspace --all-targets`: no new warnings.

## Left to the builder

- Worker API shape, job queueing, and how amp messages reach the UI.
- Knob-drag coalescing rate; the re-read interval for unreported panel changes
  (within "≈ once a second", only panel-reachable blocks).
- Status-line wording and progress display during long jobs.
- How the per-channel "last edited copy" (GUI spec, "Channel tabs", amp not
  connected) is stored.

## Differences from the GUI design spec

- **Live edit on → Open from the library**: new (the GUI spec didn't say). The
  preset goes into the amp's current settings, not stored.
- **Live is off at app start**, always (the GUI spec said it remembers its position
  across disconnects; that stays, within a session).
- **No automatic store with Live on** — made explicit (the GUI spec implied it).
- Parameter coverage moved out of this stage (GUI spec, "Work order", item 3 said
  "alongside").
