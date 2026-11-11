# KATANA Tone Studio — Full editor: chain and hidden settings

Date: 2026-11-10, updated 2026-11-11 for the merged effect colours round. Status:
draft, waiting for the user's approval (builder check done).

Refines the GUI design spec (`2026-10-22-katana-gui-design.md`, Layout B) and builds
on stage 2 (`2026-10-31-katana-editor-amp-design.md`) and the effect colours round
(merged 2026-11-10, `docs/STATUS.md`). Where they differ, **this document wins**. The
settings themselves are listed in `docs/settings.md`, generated from BOSS Tone
Studio's own data tables by `scripts/bts_settings.py`.

## What exists (main at `30a916f`)

- Colour lights both ways for BOOSTER, MOD, FX, DELAY, REVERB (`*-color` in
  `params.rs`).
- The settings each panel knob moves, per colour: booster DRIVE; delay TIME,
  FEEDBACK, EFFECT LEVEL and the TAP light; reverb EFFECT LEVEL; MOD/FX per type
  for all 31 types (`katana-proto/src/modfx.rs`: `KNOB`, parameters generated for
  all six slots as `<slot>-<type>-<setting>`, `all_params()`), with signed values
  and value-list drop-downs. All verified on the amp (`captures/105`–`113`).
- `BlockDef` in `layout.rs` lists per-colour controls and MOD/FX slots; the chain
  order is fixed (`CHAIN`).
- MOD/FX type is shown but read-only.

This round adds the rest of each block, not a second copy: settings that exist keep
their parameters and names, and the panel knob's settings appear among the type's
full settings.

## Intent

The KATANA's panel has a handful of knobs; most of what the amp can do is hidden
and only reachable through BOSS Tone Studio. The user wants that in their own app,
on Linux, for their own amp. In the user's words: recreate from Tone Studio the
"chain of effects, all the hidden (not available at amp) knobs for booster, mod, fx,
delay, delay2, reverb, solo, equaliser, contour. All katana 100 specific. No other
device, since I don't have any other."

## Goals

- G1 — Every setting Tone Studio offers for these blocks can be seen and edited:
  booster, MOD, FX, DELAY, DELAY 2, REVERB, SOLO, EQ 1, EQ 2, CONTOUR, the chain
  order, and the AMP's hidden settings (RESONANCE, BLOOM, VARIATION).
- G2 — All three colours (GREEN, RED, ORANGE) of each colour block can be edited,
  each with its own type, not only the colour the amp is playing.
- G3 — The chain order can be changed by dragging blocks, as in Tone Studio, and
  only into orders the amp allows.
- G4 — Everything works with stage 2's model unchanged: Live edit sends edits at
  once; with Live off edits stay in the app; Save/Store stay separate and explicit.

## Non-goals

- Pedal FX, noise suppressor, send/return, expression-pedal / footswitch / GA-FC
  assignments, SYSTEM (global) settings. (Claude's guess, confirmed by the user.)
- Writing the solo delay: the amp ignores writes to `SOLO_DELAY` in the current patch
  and in stored channels (`docs/protocol.md`, verified twice). Tone Studio writes the
  same block the same way and has no workaround in its code.
- Other KATANA models or generations. Only the KATANA Gen 3 100 the user owns.
- Redoing what the effect colours round built (colour lights, knob settings).
- Knob assignments other than PRESET (`ASSIGN_KNOBS`): the "panel knob also moves
  this" mark assumes PRESET, as the colour round does.

## Anti-goals (a failure even if it works)

- A chain order the amp doesn't allow, or a drop that silently does something other
  than what the user saw.
- A knob that edits the wrong setting (wrong colour instance, wrong type's slot).
  A wrong address is worse than a missing knob.
- Anything stored into a channel without the user clicking Store (stage 2 rule,
  unchanged: storing wears the amp's memory).
- Copying Tone Studio's look (its block colours, graphics). The app keeps its own
  style; only the data (addresses, ranges, value names, chain tables) is taken.

## Decided by the user

Confirmed on the mockups in the brainstorm (2026-11-10):

### Editor layout (unchanged from Layout B, extended)

Top bar, channel tabs, the signal chain row, the detail panel of the selected block,
the amp strip, the status line. The chain row shows, in processing order: BOOSTER,
MOD, AMP, EQ 1, FX, DELAY, DELAY 2 / REVERB, EQ 2, SOLO (the order and EQ places
follow the patch). Each card: title, on/off, type line, colour lights where the
block has colours.

### Colour blocks: three tabs

BOOSTER, MOD, FX, DELAY and DELAY 2 / REVERB show three colour tabs in their detail
panel: GREEN, RED, ORANGE, each tab labelled with that colour's type
(e.g. "RED · CHORUS"). The tab of the colour the amp plays has an outline. The panel
shows the selected tab's type selector and every setting of that type. The TYPE
selector is a drop-down, for MOD and FX too (the user asked for it on 2026-11-10).

Instances: colour n (GREEN=1, RED=2, ORANGE=3) edits BOOSTER(n), MOD = FX(n) +
FX_DETAIL(n), FX = FX(n+3) + FX_DETAIL(n+3), DELAY = DELAY(n), REVERB = REVERB(n),
DELAY 2 = DELAY(n+3) (driven by the REVERB colour). See `docs/settings.md`, "Patch
blocks".

### Per-type settings

MOD and FX share one list of 31 types; FX_DETAIL holds the settings of every type at
once, so switching type keeps each type's settings. The panel shows only the
selected type's settings, with Tone Studio's labels, ranges and value names
(`docs/settings.md`, one table per type). Delay types and reverb types likewise
change which knobs show (e.g. SDE-3000 adds RANGE, FILTER, phases, MOD SW; SPRING
adds COLOR; PAN adds TAP TIME; MODULATE adds MOD RATE/DEPTH). Grouping inside a
panel follows the setting names (e.g. CHORUS: crossover, low band, high band).
HARMONIST also shows MASTER KEY, which lives in the OTHER block and is shared by
all six MOD/FX instances (as on Tone Studio's HARMONIST page; *Claude's call*).

### DELAY 2 / REVERB: one block

One chain card and one panel, because one amp button and the reverb's colour drive
both. Panel header: colour tabs, MODE switch DELAY 2 / DLY+REV / REVERB
(REVERB LAYER_MODE, per colour). Below, DELAY 2 and REVERB side by side, each with
its own on/off, type selector and knobs. The half that the current mode doesn't use
is dimmed but still editable.

On/off (from Tone Studio's `effect_controller.js`): the patch has two switches,
DELAY2_SW (`20 00 08 04`) and REVERB_SW (`20 00 08 05`), and Tone Studio shows each
on its own half. Its panel REVERB button lights from the switch(es) the mode uses:
DELAY2_SW in mode DELAY 2, REVERB_SW in REVERB, either one in DLY+REV. The chain
card's on/off shows the same thing. *Claude's call:* clicking the card's on/off sets
the switch(es) the mode uses (both in DLY+REV) to the new state. What the amp's own
REVERB button writes in each mode is checked on the amp early in the work, and the
card follows the amp if it differs.

### EQ 1 and EQ 2

Each: on/off, POSITION switch AMP IN / AMP OUT, TYPE switch PARAMETRIC / GE-10.
GE-10: ten vertical sliders 31 Hz…16 kHz plus LEVEL (±12 dB). PARAMETRIC: knobs
LOW CUT, LOW GAIN, LOW-MID FREQ/Q/GAIN, HIGH-MID FREQ/Q/GAIN, HIGH GAIN, HIGH CUT,
LEVEL. Each EQ keeps both types' settings. Changing POSITION moves the card in the
chain, the same as dragging it.

### SOLO

On/off and LEVEL; SOLO EQ (on/off, AMP IN/OUT, LOW CUT, LOW GAIN, MID FREQ, MID Q,
MID GAIN, HIGH GAIN, HIGH CUT, LEVEL) and SOLO DELAY side by side. SOLO DELAY is
shown read-only (see Non-goals) with a short note why.

### Amp strip: CONTOUR and the AMP's hidden settings

CONTOUR OFF / 1 / 2 / 3, and for the selected contour its TYPE (1–4) and FREQ SHIFT
(−50…+50). With CONTOUR OFF, TYPE and FREQ SHIFT are hidden or disabled.

Also on the amp strip (the user, 2026-11-10), next to the existing knobs:
RESONANCE (knob, `20 00 06 08`), BLOOM (on/off, POWERAMP_VARIATION `20 00 06 06`)
and VARIATION (on/off, PREAMP_VARIATION `20 00 06 09`; Tone Studio's label on its
panel page, next to AMP TYPE). Ranges and values from `docs/settings.md`, "AMP".

### Chain editing: drag, like Tone Studio

- BOOSTER, MOD, FX and DELAY can be dragged. While dragging, only the drop spots that
  lead to an allowed order light up; dropping elsewhere does nothing.
- The allowed moves are Tone Studio's table (`docs/settings.md`, "Chain orders":
  CHAIN values 0–8 and "Drag moves"). A drop sets CHAIN to the target order.
- EQ 1 and EQ 2 drag to either side of the AMP; a drop sets their POSITION
  (AMP IN / AMP OUT). Their place around the AMP follows the "EQ 1 / EQ 2 / Around
  the AMP" table.
- AMP, DELAY 2 / REVERB and SOLO don't move. Pedal FX, foot volume and send/return
  are not shown (non-goals) and don't affect the drawing.

## Decided by Claude (the user may overrule)

- **Clicking a colour tab only shows that colour**, it doesn't change what the amp
  plays. The colour lights on the chain card stay the way to switch the playing
  colour (as today). Reason: editing a hidden colour shouldn't change the sound.
- **Settings the panel knob drives:** the amp re-applies the knob position on a
  colour change and when the knob turns: delay HIGH CUT and EFFECT LEVEL; reverb
  LOW CUT, HIGH CUT, DENSITY, EFFECT LEVEL; the booster and MOD/FX settings in
  `KNOB` (`docs/protocol.md`). These settings are shown and editable like the rest,
  with a small mark saying the panel knob also moves them. With Live on the app
  follows what the amp reports, so a knob turn or colour change can overwrite an edit
  made in the app; that is how the amp behaves, not a bug. (The colour round hid
  delay HIGH CUT and reverb LOW CUT/DENSITY for this reason; they come back here.)
- **A chain drop is an edit like any knob:** with Live on it is sent at once; with
  Live off it changes the app's patch only. Undo isn't added.
- **Where the settings come from:** the parameter definitions (address, encoding,
  range, value names, display format) are generated from Tone Studio's tables, the
  same data as `docs/settings.md`, into Rust source in `katana-proto`, checked in. The
  generator stays a dev tool; the app never reads Tone Studio at runtime. Labels and
  units are the app's own text, written from Tone Studio's. Existing parameter names
  (`delay1-time`, `mod1-chorus-…`, etc.) stay; where a generated setting is one that
  already exists, the existing definition wins and the generator checks they agree.
- **Verification before it counts as done:** `params.rs` today accepts only settings
  confirmed on the amp. The new settings come from Tone Studio's tables, which matched
  every one of the 23 verified ones. They are checked on the amp by an automatic
  write / read-back run over every setting this design shows (with a backup of the
  current patch first and the original restored after), plus a hands-on check by the
  user that a sample of knobs per block changes the sound.
  - The run comes early, right after the generated table exists and before the new
    panels are built. The amp works from the Mac too (port `KATANA3`), so it doesn't
    wait for a Linux session.
  - Each generated setting carries a verified / excluded mark from the start. An
    excluded setting doesn't show in the GUI and is listed in `docs/protocol.md`.
  - A read-back that differs from what was written is a failure, unless the amp
    does it for a known reason (e.g. the asynchronous colour behaviour, or the
    knob re-applying its position, in `docs/protocol.md`); then it is written down
    there. The run doesn't change colours or touch the panel while it writes.
  - If a setting the layout depends on fails (a TYPE, LAYER_MODE, CHAIN, EQ
    POSITION or a colour selector), work stops and the user is asked before the
    design is changed.
- **Order of work:** on a feature branch from `main` at `30a916f` (the effect colours
  round merged).
- **Name clash:** the existing `delay2-*` parameters are DELAY(2), the RED delay, not
  DELAY 2. New DELAY 2 parameters are named so they can't be confused with them
  (e.g. by instance, `delay4..6-*`), and the GUI labels DELAY 2 as its own block.
- **Patch files:** nothing changes. A `Patch` already holds every byte of the
  patch, so Save, Load and Store already carry all hidden settings.

## Left to the builder

- How the generated definitions are shaped in Rust (extend `ParamDef`/`BlockDef` in
  `katana-proto/src/layout.rs` or a new table), as long as each setting carries
  address, encoding, raw range, display offset, value names or display format.
- Display formats (ms, s, Hz/kHz lists, dB, %, signed values), from Tone Studio's
  format expressions and value lists (`docs/settings.md`, "Value lists").
- egui drag-and-drop mechanics, drop-spot drawing, widget sizes, panel scrolling
  when a type has many settings (HARMONIST has the most).
- Which value-list settings are knobs, which are dropdowns or switches (follow Tone
  Studio's control kinds where sensible).
- Keyboard and accessibility details, matching the existing widgets.
- How the write/read-back run is built and run (CLI subcommand or test), as long
  as it never stores into a channel.
