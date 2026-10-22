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

## Open questions

1. ~~Model ID width~~ — 3 bytes (resolved).
2. ~~Editor communication mode needed for RQ1?~~ — no: the amp answers RQ1 for the patch area without it (resolved).
3. The official app's own RQ1/DT1 sequence (connect handshake, write chunking) — not yet captured with usbmon; our block list is derived from its address table and verified against the amp.
4. Whether DT1 to `20 00 00 00` (current patch) is applied immediately / audibly, and whether the amp accepts >128-byte DT1.
