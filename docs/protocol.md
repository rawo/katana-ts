# KATANA Gen 3 — USB-MIDI SysEx Protocol

Living reverse-engineered spec. Every entry is either **VERIFIED** (backed by a
capture in `captures/`) or **UNVERIFIED** (with its source). Code in
`katana-proto` may only rely on VERIFIED entries.

## Sources

| # | Source | What it gives | Notes |
|---|--------|---------------|-------|
| S1 | Official app's bundled web UI, unpacked by the app on first run to `%LOCALAPPDATA%\Roland\BOSS TONE STUDIO for KATANA Gen 3\html\` (Wine: `~/.wine/drive_c/users/<user>/AppData/Local/Roland/…/html/`). App version 1.1.0 build 26. | Framing constants (`js/utilities/constant.js`), model/device ID and timing (`js/config/product_setting.js`), full parameter address map with 888 `PRMID_*` entries (`js/config/address_map.js`), command addresses (`js/businesslogic/bts/address_const.js`). | The `.exe` is a native WebView2 host; the editor itself is HTML/JS. Only constant/data tables are used from here — no logic is copied. |
| S2 | Official Roland/BOSS MIDI Implementation for Gen 3 | — | None found (searched 2026-10-22). |
| S3 | Community: [jbsohn/katana-midi-bridge](https://github.com/jbsohn/katana-midi-bridge), [snhirsch/katana-midi-bridge](https://github.com/snhirsch/katana-midi-bridge) (`katana_sysex.txt`), [SteveObert/KatanaUSB_Midi_controller](https://github.com/SteveObert/KatanaUSB_Midi_controller) | Roland RQ1/DT1 framing and address layout for **MkI/MkII**. | Different model ID and address map from Gen 3 per S1; useful only as confirmation of the general framing. |

## Envelope

**VERIFIED** 2026-10-22 against the amp (firmware reply version `06 00 00 00`):
the amp answers RQ1 sent with this framing — evidence `captures/02-patch-read-{rq1,dt1}.syx`,
golden tests in `katana-proto/tests/golden_captures.rs`.

```
F0 41 <dev> <model-id…> <cmd> <addr ×4> <data… | size ×4> <checksum> F7
```

- `41` = Roland manufacturer ID.
- `<dev>` = `10` (S1 `deviceId`).
- `<model-id>` = `01 05 07` — **3 bytes** on the wire (VERIFIED: an RQ1 with `00 01 05 07` got no reply; with `01 05 07` the amp answers). Older Katanas use 4 bytes (`00 00 00 33`).
- `<cmd>`: `11` = RQ1 (data request), `12` = DT1 (data set).
- Address and RQ1 size are 4 bytes each (S1 `lengthOfAddr: 4`, `lengthOfSize: 4`), every byte 7-bit.
- Max SysEx length 128 bytes in the app (S1 `SYSEX_MAXLEN`). The amp's own replies exceed it: each RQ1 for a patch block (up to 225 bytes) is answered with exactly one DT1 (VERIFIED, `captures/02-patch-read-dt1.syx`). Whether the amp accepts >128-byte DT1 *input* is untested; we send ≤115-byte payloads.
- App waits 20 ms between transmissions (S1 `interval`), RQ1 timeout 15 s (S1 `timeout`).

## Checksum

**VERIFIED**: standard Roland — `(128 − (Σ addr+data bytes mod 128)) mod 128`.
All 80 amp→host DT1 replies in `captures/02-patch-read-dt1.syx` carry checksums
matching this formula, and the amp accepts RQ1s checksummed with it.

## Identity

**VERIFIED** (`captures/01-identity.txt`): Universal Identity Request
`F0 7E 7F 06 01 F7` → `F0 7E 10 06 02 41 07 05 00 00 06 00 00 00 F7`
(device 10, manufacturer 41, family `07 05`, member `00 00`, version `06 00 00 00`).
Matches the app's check in S1 `address_const.js`.

## Address model

VERIFIED for the current-patch area (block reads at `20 00 7E 00` → `20 01 00 00` carry correctly). Addresses in the S1 tables are written as 4 base-128 "digits"
packed into a 32-bit hex literal (e.g. `0x00010000` follows `0x00007E00`),
i.e. each byte carries 7 bits and arithmetic must carry at `0x7F`. The S1 code
converts them to linear values the same way our `Address` type does.

### Root areas

| Address | Area | Notes |
|---------|------|-------|
| `00 00 00 00` | SETUP | `PATCH_NUMBER` (current patch select) |
| `10 00 00 00` | SYSTEM | global settings: USB levels, global EQ, line out, GA-FC, MIDI |
| `20 00 00 00` | PATCH (current / edit buffer) | layout below; `ADDRESS.CURRENT_PATCH_NAME` |
| `20 10 00 00` … `20 70 00 00`, `21 00 00 00` … `21 20 00 00` | PATCH(1)…(10) | stored user patches, same layout |
| `30 00 00 00` | TEMP | `TEMP_COM`, `TEMP_KNOB`, `TEMP_KNOB_SOLO_DELAY` |
| `7F 00 00 00` | COMMAND | editor/control commands, see below |

### Channels

**VERIFIED** 2026-10-22: `SETUP PATCH_NUMBER` (`00 00 00 00`, INTEGER2x4) holds the
selected channel; selecting a channel copies stored slot **PATCH(selector + 1)**
into the current patch at `20 00 00 00`.

| Selector | Channel (app label) | Stored slot | Evidence |
|---|---|---|---|
| 0 | PANEL | PATCH(1) `20 10 00 00` | selector read `00 00` in PANEL mode |
| 1 | A: CH1 | PATCH(2) `20 20 00 00` | selector `00 01`; current patch equal to PATCH(2) in 6 blocks, unequal to PATCH(1)/(3) |
| 2–4 | A: CH2–CH4 | PATCH(3)–(5) | CH4: selector `00 04`, current name "Bloom Crunch" = PATCH(5) |
| 5–7 | B: CH1–CH3 | PATCH(6)–(8) | follows the pattern (bank B selected with the panel's bank button) |
| 8 | B: CH4 | PATCH(9) `21 10 00 00` | selector `00 08`, current name "Bite Lead" = PATCH(9) |
| — | app preview buffer | PATCH(10) `21 20 00 00` | slot named `PREVIEW` |

### Patch layout (offset from patch base)

Blocks every `00 00 02 00`: `COM` 00 00, `OTHER` 02 00, `COLOR` 04 00, `AMP` 06 00,
`SW` 08 00, `BOOSTER(1–3)` 0A 00–0E 00, `FX(1–6)` 10 00–1A 00,
`FX_DETAIL(1–6)` 1C 00–26 00, `DELAY(1–6)` 28 00–32 00, `REVERB(1–3)` 34 00–38 00,
`SOLO_COM` 3A 00, `SOLO_EQ` 3C 00, `SOLO_DELAY` 3E 00, `CONTOUR_COM` 40 00,
`CONTOUR(1–3)` 42 00–46 00, `PEDALFX_COM` 48 00, `PEDALFX` 4A 00,
`EQ_EACH(1–2)` 4C 00–4E 00, `EQ_PEQ(1–2)` 50 00–52 00, `EQ_GE10(1–2)` 54 00–56 00,
`NS` 58 00, `SENDRETURN` 5A 00, `ASSIGN_*` 5C 00 … `01 1A 00`,
`PATCH_KNOB_READONLY` `01 1C 00`, `PATCH_KNOB_SOLO_DELAY_READONLY` `01 1E 00`.
Per-parameter rows come from S1 `address_map.js` and are added to the address map
below only as each is verified.

### Command addresses (S1 `address_const.js`)

| Address | Name |
|---------|------|
| `7F 00 00 00` | EDITOR_COMMUNICATION_LEVEL |
| `7F 00 00 01` | EDITOR_COMMUNICATION_MODE (00 off, 01 on) |
| `7F 00 00 02` | RUNNING_MODE (00 play) |
| `7F 00 00 03` | EDITOR_COMMUNICATION_REVISION |
| `7F 00 01 00` | PATCH_SELECT |
| `7F 00 01 04` | PATCH_WRITE |
| `7F 00 01 0E` | PATCH_CLEAR |
| `7F 01 01 09` | PREVIEW_MUTE |
| `7F 01 01 0A` | EXPORT |
| `7F 01 02 02` | GAFC_TYPE |

### Value encodings (S1 `constant.js`)

| Name | Bytes | Layout |
|------|-------|--------|
| `INTEGER1x1`…`INTEGER1x7` | 1 | low 1…7 bits |
| `INTEGER2x4` | 2 | `0000aaaa 0000bbbb` (nibbles, high first) |
| `INTEGER2x7` | 2 | `0aaaaaaa 0bbbbbbb` |
| `INTEGER4x4` | 4 | four nibbles |

Table entries also carry `ofs` (displayed value = raw − ofs), `min`, `max`, `init`.

## Address map

Only VERIFIED entries.

| Address | Size | Name | Encoding | Range | Evidence |
|---------|------|------|----------|-------|----------|
| `20 00 00 00` | 16 | PATCH COM: patch name | ASCII, space-padded | — | `golden_captures.rs::captured_patch_name_reply_decodes` |
| `20 00 00 00` … `20 01 1E 00` | 80 blocks, 2 649 bytes | Current patch blocks | see `device::PATCH_BLOCKS` | — | `captures/02-patch-read-{rq1,dt1}.syx` (each block answered in full) |
| `20 00 06 00` | 1 | AMP GAIN (`gain`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 33→100) |
| `20 00 06 01` | 1 | AMP VOLUME (`volume`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 46→100) |
| `20 00 06 02` | 1 | AMP BASS (`bass`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 0→100) |
| `20 00 06 03` | 1 | AMP MIDDLE (`middle`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 0→100) |
| `20 00 06 04` | 1 | AMP TREBLE (`treble`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 20→100) |
| `20 00 06 05` | 1 | AMP PRESENCE (`presence`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 15→100) |
| `20 00 06 07` | 1 | AMP TYPE (`amp-type`) | U7 | 0–5 | `captures/100-watch-knobs.log` (all 6 positions) |
| `20 00 08 00` | 1 | BOOSTER on/off (`booster-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 08 01` | 1 | MOD on/off (`mod-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 08 02` | 1 | FX on/off (`fx-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 08 03` | 1 | DELAY on/off (`delay-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 08 05` | 1 | REVERB on/off (`reverb-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 3A 00` | 1 | SOLO on/off (`solo-sw`) | U7 | 0–1 | `captures/100-watch-knobs.log` (toggled) |
| `20 00 04 00` | 1 | BOOSTER colour (`booster-color`); selects BOOSTER(1..3) | U7 | 0–2 | `captures/100-watch-knobs.log` (0,1,2; knob edits moved between blocks) |
| `20 00 02 01` | 1 | Cabinet resonance (`cab-resonance`) | U7 | 0–2 | `captures/100-watch-knobs.log` (0,1,2) |
| `20 00 40 00` | 1 | Contour button (`contour`) | U7 | 0–3 | `captures/101-watch-contour.log` (cycles 0→1→2→3→0) |
| `20 00 0A 01 / 0C 01 / 0E 01` | 1 | BOOSTER(1/2/3) DRIVE (`booster1..3-drive`) | U7 | 0–120 | `captures/100-watch-knobs.log` (changed in each colour) |
| `20 00 2A 01` | 4 | DELAY(2) TIME (`delay2-time`) | 4 nibbles, high first | 1–2000 ms | `captures/100-watch-knobs.log` (382→489) |
| `20 00 2A 05` | 1 | DELAY(2) FEEDBACK (`delay2-feedback`) | U7 | 0–100 | `captures/100-watch-knobs.log` (changed) |
| `20 00 2A 07` | 1 | DELAY(2) EFFECT LEVEL (`delay2-level`) | U7 | 0–120 | `captures/100-watch-knobs.log` (changed) |
| `20 00 34 0A` | 1 | REVERB(1) EFFECT LEVEL (`reverb1-level`) | U7 | 0–100 | `captures/100-watch-knobs.log` (swept 0→100) |
| `20 01 1C 00–04` | 1 each | Panel knob positions BOOSTER/MOD/FX/DELAY/REVERB (read-only per app; −1 = off) | U7, ofs 1 | −1–100 | `captures/100-watch-knobs.log` (observed; not settable) |

Panel knobs for effects drive several parameters at once: e.g. turning BOOSTER
changes `BOOSTER(n) DRIVE` and `EFFECT_LEVEL` together, where `n` is chosen by
the colour button. `set` targets the underlying parameters.

**Write-protected: SOLO_DELAY in the current patch** (VERIFIED 2026-10-22):
DT1s to `20 00 3E 00`–`3E 0D` are accepted silently but never change the
value — level 0/30/1, feedback +1 and on/off 1 all read back unchanged, with
solo off and on. Writing all of stored PATCH(1) into the current patch changed
79 of the 80 differing bytes; the only one kept was SOLO_DELAY level. The
`PATCH_KNOB_*_READONLY` blocks, despite their app names, *are* written.
`write_patch` skips SOLO_DELAY (`device::WRITE_PROTECTED`), so `load` cannot
change solo-delay settings; `dump` still saves them. Stored slots don't accept
SOLO_DELAY writes either (VERIFIED 2026-10-31, see "Amp control").

**Colour changes apply asynchronously** (VERIFIED 2026-10-22): writing a
*different* variation colour (`20 00 04 00`…) makes the amp re-derive that
effect's on/off from its knob byte (`20 01 1C 00`…, non-zero → on) some time
after the DT1. An on/off written 20 ms later is overwritten (colour 2→1, then
booster on/off 0: reads 1); 300 ms later it sticks (reads 0). Rewriting the
same colour, or writing the knob byte alone, changes nothing. `write_patch`
pauses 300 ms after the COLOR block (`device::SETTLE_AFTER`). With that,
`verify` passes in PANEL mode against every stored slot PATCH(2)–(9).

**Panel knob position overrides remote on/off on colour change** (observed
2026-10-22, by the user): with the physical BOOSTER knob at its off position
(`PATCH_KNOB_READONLY` booster byte `00`), `set booster-sw on` turns the booster
on, but a following `set booster-color …` switches it off again — the amp
re-applies the knob position when the variation changes. With the knob turned
on, colour changes keep the booster on. Editors must send colour before on/off
(and may need to account for the knob-position byte).

## Amp control (VERIFIED 2026-10-22 by hand, 2026-10-31 in katana-proto)

First probed by hand with `amidi` (2026-10-22), then coded in `katana-proto::amp`
and checked with the hardware tests in `katana-proto/tests/hardware.rs`
(2026-10-31, scratch channel B4, backups first).

- **Editor mode:** DT1 `7F 00 00 01` ← `01` on, ← `00` off (`amp::EditorSession`,
  turned off again on drop).
- **Channel select:**
  - MIDI **Program Change** works without editor mode: PC 2 → selector 3 (A:CH3),
    PC 3 → selector 4 (A:CH4), i.e. PC *n* → selector *n*+1 (PANEL via PC untested).
  - DT1 to the selector `00 00 00 00` and `PATCH_SELECT` `7F 00 01 00` ← `00 nn` are
    **ignored without editor mode**; `PATCH_SELECT` works **with** editor mode on
    (`amp::select_channel`: waits until the selector shows the channel, then
    `device::SELECT_SETTLE` = 300 ms; 300 ms is enough, shorter values untried).
- **Store to channel:** `PATCH_WRITE` `7F 00 01 04` ← `00 <selector>` stores the
  current patch into that channel **only in editor mode** (ignored otherwise).
  2026-10-22, by hand on A:CH4: live gain 60→77, store → stored slot `20 50 06 00` =
  77; gain 60, store → byte-identical to the backup. 2026-10-31, `amp::store_patch`
  (select, write the current patch, store, read the slot back): "KTS TEST" with GAIN
  changed into B4, then the original back → both `Ok`, echo seen, B4 byte-identical
  to the original; passed twice.
- **Solo delay in a stored channel:** a DT1 straight into the slot's SOLO_DELAY is
  ignored too (2026-10-31: B4 level `20 50 3E 07` was 50, wrote 30, read 50). So a
  stored channel keeps its own solo delay (`device::SLOT_SOLO_DELAY_WRITABLE` =
  false); `store_patch` selects the channel first, so the current patch carries the
  channel's own solo delay into the store. `device::NOT_STORED` = SOLO_DELAY only:
  every other byte of a stored patch read back as written.
- **Messages the amp sends by itself in editor mode** (2026-10-31,
  `store_patch_round_trip_on_b4 --nocapture`):
  - nothing within 1 s of turning editor mode on;
  - after `PATCH_SELECT` B4: an echo of it (`7F 00 01 00` ← `00 08`), the selector
    (`00 00 00 00` ← `00 08`, seen twice), and the
    `7F 01 01 01`–`05` status stream (seen: `02 03 03 00 00`, repeating);
  - after `PATCH_WRITE` B4: its echo (`7F 00 01 04` ← `00 08`, seen twice), then the
    same status stream.
  The `7F 01 01 xx` stream arrives as `amp::Notification::Other`; its meaning is in
  "Panel in editor mode" below.
- **Panel in editor mode** (2026-11-02, the user at the amp, `monitor --editor`,
  `captures/102-*`, `103-*`, `104-*`): the panel behaved normally (sound, LEDs,
  channel switching). **Every panel action tried is reported as a DT1**. The app still
  re-reads the AMP block once a second (`device::PANEL_BLOCKS`), see "Unexplained" below:

  | Panel action | DT1s from the amp |
  |---|---|
  | GAIN, VOLUME, BASS, MIDDLE, TREBLE, PRESENCE | `20 00 06 00`–`05` |
  | Amp type | `20 00 06 07` |
  | Booster knob | `20 00 0A/0C/0E 01` drive and `06` level (block follows the colour), `20 01 1C 00`, `30 00 02 00` |
  | Mod, FX knob | the selected effect's own parameter (seen: `20 00 21 48`, `20 00 23 19`), `20 01 1C 01/02`, `30 00 02 01/02` |
  | Delay knob | `20 00 2A/2C 05` feedback and `07` level, `20 01 1C 03`, `30 00 02 03` |
  | Reverb knob | `20 00 34/36/38 0A` level and `09`, `20 01 1C 04`, `30 00 02 04` |
  | Effect knob to/from its off position | also `20 00 08 00/01/02/03/05` on/off |
  | Effect colour button | `20 00 04 00`–`04` and `7F 01 01 01`–`05` |
  | SOLO, CONTOUR, cabinet resonance | `20 00 3A 00`, `20 00 40 00`, `20 00 02 01` |
  | Channel | selector `00 00 00 00` ← `00 nn`, then `7F 01 01 01`–`05` (twice) |

  - Check: patch snapshot, log, snapshot; the log replayed over the first snapshot
    equals the second byte for byte (`103`: colours with knobs, all 6 amp types,
    contour, cabinet, 14 changed bytes; `104`: 35 fast amp type steps, none skipped).
    Knob turns in other colours land in that colour's block.
  - A channel switch reports only the selector, not the new patch, so the current
    patch must be read after it. Selecting PANEL reports the bytes where the panel
    differs from the channel just left, after the selector (`102`, lines 988–1001).
  - `7F 01 01 01`–`05` is the LED of booster, mod, FX, delay, reverb: `0`–`2` colour,
    `3` off (observed: follows `20 00 04 0n` and on/off; not in the app's table as
    such). `30 00 02 00`–`04` mirrors `20 01 1C 00`–`04`. Neither is in the current
    patch.
  - **Selecting PANEL with `PATCH_SELECT` works** (`device::PANEL_SELECTABLE` = true;
    hardware test `select_panel_and_back`, 2026-10-31: selector 5 → PANEL → 5).
  - **Unexplained:** in `102` the last amp type report is `3` (lines 453–454), yet
    the amp read `4` after the log and the user's knob was on LEAD (`4`). B4, the
    channel before PANEL, stores `4`, which is why selecting PANEL reported no
    change. A 3→4 report seems to be missing; it didn't happen again in `103` and
    `104` (41 reports, both checks byte-exact). The cause is unknown. If it is a real
    loss, the app would show and could store an amp type that isn't the one heard, so
    the AMP block is re-read once a second as a safety net.
- **CLI end to end** (2026-10-31, `katana-cli`, A3 and A4):
  - `tsl-list samples/Black_Sabbath.tsl` → `1 iron man`, `2 Paranoid`, `3 Black Sabbath`.
  - `dump A3 a3.syx` / `dump A3 a3.tsl` → both written ("A3 'Riff Shred'"); again
    with Enter at `Overwrite a3.syx? [y/N]` → "nothing written", file unchanged;
    without a terminal → error asking for `--yes`.
  - `store samples/Black_Sabbath.tsl A3 --patch 1`, `y` → both backup paths, "stored
    'iron man' in A3; the amp is now on A3", "solo delay not stored". `dump A3` as
    `.tsl` equals set patch 1 in all 80 blocks.
  - `store <A3 backup>.syx A3 --yes` → `dump A3` byte-identical to the dump before.
  - `store a3.tsl A4 --patch 1 --yes` → `dump A4` as `.tsl` equals `a3.tsl` in all 80
    blocks; A4 restored from its backup, byte-identical to before.
  In both comparisons SOLO_DELAY matched too: the channels involved hold the same
  solo-delay settings, so this run doesn't show a solo delay being left behind.
- Backups of the scratch channels taken before the tests: `backups/slot{4,5,8,9}-*.syx`
  (2026-10-22) and `backups/2026-10-31-slot{4,5,8,9}.syx` (hardware test
  `backup_scratch_channels`); untracked, local.

## Open questions

1. ~~Model ID width~~ — 3 bytes (resolved).
2. ~~Editor communication mode needed for RQ1?~~ — no: the amp answers RQ1 for the patch area without it (resolved).
3. The official app's own RQ1/DT1 sequence (connect handshake, write chunking) — not yet captured with usbmon; our block list is derived from its address table and verified against the amp.
4. ~~Whether DT1 to the current patch is applied audibly~~ — yes (resolved 2026-10-22, by ear, amp in PANEL mode: `katana-cli dump`, knobs changed, `katana-cli load` restored the dumped sound). Still open: whether the amp accepts >128-byte DT1 (we never send one).
5. ~~Slot ↔ channel mapping~~ — resolved, see "Channels" below.

## Value names

From the official app's panel labels (`js/businesslogic/bts/effect_controller.js`
`panelAmpTypeInfo`, `panelContourTypeInfo`) and resource tables (`js/config/resource.js`).

| Parameter | Values | Hardware check |
|---|---|---|
| `amp-type` | 0 ACOUSTIC, 1 CLEAN, 2 PUSHED, 3 CRUNCH, 4 LEAD, 5 BROWN | 6 panel positions = 0..5 in `100-watch-knobs.log`; `set amp-type 2` took effect |
| `contour` | 0 OFF, 1, 2, 3 | button cycle 0→3→0 in `101-watch-contour.log`; `set contour 2` took effect |
| `cab-resonance` | 0 VINTAGE, 1 MODERN, 2 DEEP | 3 positions in log |
| `booster-color` | 0 GREEN, 1 RED, 2 ORANGE | button LED observed by the user after `set booster-color 0/1/2` |
| `*-sw` | 0 OFF, 1 ON | toggled in log; `set reverb-sw 0` took effect |
