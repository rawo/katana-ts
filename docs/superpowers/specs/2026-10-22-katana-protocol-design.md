# KATANA Gen 3 Protocol Library — Design Spec

Date: 2026-10-22
Status: Approved, not yet implemented (phase 1 of a two-phase project)

## Background

The user owns a BOSS KATANA Gen 3 amplifier and wants a native Linux
application to edit patches/tones on it, replacing their current reliance
on the official "BOSS TONE STUDIO for KATANA Gen 3" Windows app run under
Wine. That official app is a native Win32 C++ binary (confirmed via
`objdump -p`: CLR Runtime Header RVA/size are both zero, imports are
MSVCP140/VCRUNTIME140/Media Foundation/WinInet — not .NET, no managed
code to decompile).

Decompiling and recompiling the official app itself was explicitly ruled
out: it would mean reproducing a commercial closed-source product
(copyright issue) and is not realistically achievable anyway for a
binary of this kind (no recoverable class structure, hard dependency on
Windows-only APIs like COM/Media Foundation with no Linux equivalent).

Instead, the approach is standard interoperability reverse engineering:
learn the device's own wire protocol (what the official app sends to the
amp over USB-MIDI) well enough to write fresh, original software that
speaks it. This is well-trodden territory for music gear (Boss/Roland
devices have had SysEx protocols partially documented by hobbyists
before).

## Project split

This is split into two sub-projects with a hard dependency:

1. **Phase 1 (this spec):** protocol reverse-engineering + a Rust library
   (`katana-proto`) + a minimal CLI (`katana-cli`) that can read a full
   patch from the amp and write it back byte-identical, plus write
   individual parameters and have them take effect on the amp.
2. **Phase 2 (future, separate spec):** a native GUI app (Rust +
   egui/iced, per user preference) that uses `katana-proto` to provide
   full patch/tone editing matching the official app's feature set. Not
   designed yet — its data model depends on what phase 1 discovers about
   the protocol's actual shape, so designing it now would mean guessing.

The user was explicit that phase 1 must "work perfectly" — defined below
as full-patch round-trip, not just a couple of parameters — before phase
2 starts.

## Environment findings (as of 2026-10-22)

- Device enumerates on the Linux host as a **class-compliant USB-MIDI
  device**: `lsusb` shows `ID 0582:02f0 Roland Corp. KATANA3`; `amidi -l`
  shows `KATANA3 MIDI 1` on `hw:3,0,0`; ALSA sequencer client 28
  `'KATANA3'` port 0.
- This means no raw-USB/vendor-driver work is needed — the kernel's
  standard USB-MIDI class driver handles framing. The protocol library's
  I/O layer can use a normal MIDI crate (`midir`) instead of raw USB
  (`rusb`/`nusb`).
- The official app runs under Wine at
  `~/.wine/drive_c/Program Files (x86)/BOSS/BOSS TONE STUDIO for KATANA
  Gen 3/BOSS TONE STUDIO for KATANA Gen 3.exe`. Wine's winmm→ALSA routing
  means the app's real MIDI traffic flows through the same ALSA port we
  can observe, so we can capture live ground-truth traffic by running the
  real app and watching the wire.
- User has physical access to the device and can connect/disconnect it
  for capture sessions.
- Target repo: `~/Projects/katana-ts`, git already initialized, currently
  empty.

## Approach

Combine three things, in order of cheapness:

1. **Check for prior art first.** Before capturing a single byte, check
   whether Roland/Boss published an official MIDI Implementation chart
   for this model, and whether the hobbyist community has already
   reverse-engineered a related Katana/Boss device sharing the same
   Roland "Exclusive" SysEx family. Bootstrapping from accurate prior art
   is strictly faster than discovering from zero — but any inherited
   assumption must still be verified against this specific Gen 3 unit
   before being trusted.
2. **Capture-driven, clean-room discovery as the primary method.**
   Capture real USB traffic with `usbmon` + Wireshark (captures raw URBs
   regardless of ALSA's abstraction layer, so it's reliable ground truth
   independent of which driver path Wine/ALSA take) while driving the
   official app under Wine. Diff before/after captures to isolate exactly
   which bytes each UI control touches. This is the authoritative source
   of truth for the address map — nothing goes into `katana-proto`
   without a captured-and-verified byte sequence behind it.
3. **Ghidra static analysis as an accelerant, not a source of logic.**
   Used only to read constant/string tables in the binary (e.g. amp-model
   ID → display-name tables, effect-type enumerations) to avoid having to
   manually click through every single UI option to discover every ID.
   Never used to copy implementation logic — the protocol library's
   actual encode/decode logic is written from scratch based on captured
   bytes, not from decompiled code.

This combination was chosen over two alternatives considered:
- Pure black-box (skip Ghidra entirely): cleanest possible legal posture
  but full parameter coverage by hand (every amp model × every effect
  block × every parameter) is very slow. Rejected as impractically slow
  for the "full patch round-trip" bar phase 1 must meet.
- Prior-art-first as the primary method (not just a first check): risks
  inheriting wrong assumptions from a different device/model if Gen 3
  diverges, and would still need full verification against this unit
  anyway, so it doesn't actually save the verification work. Used only
  as a cheap first step, not the main method.

## Components

- **`katana-proto/`** (library crate)
  - MIDI I/O via the `midir` crate (cross-platform, uses ALSA sequencer
    on Linux — matches the confirmed class-compliant device).
  - SysEx envelope assembly/parsing: header bytes, address field,
    payload, checksum — exact framing (Roland's historical "Exclusive"
    format uses a header/model-ID/command/address/data/checksum
    structure, but exact bytes for this model are unconfirmed until
    captured).
  - Checksum algorithm must be verified bit-exact against real captured
    checksums, not assumed from older Roland product documentation.
  - A `Patch` struct covering the full current-patch address range.
  - Typed read/write functions per parameter, added only as each is
    confirmed via capture.
- **`katana-cli/`** (binary crate)
  - `dump` — read current patch, print/save it.
  - `load` — write a saved patch back to the amp.
  - `set <param> <value>` — write a single parameter.
  - `monitor` — live-tail MIDI traffic on the device's port, used during
    discovery sessions.
- **`docs/protocol.md`** — the living reverse-engineered spec: envelope
  format, and an address-map table (address, length, name, encoding,
  value range, and an evidence note per entry — which capture or static
  analysis finding proved it). This document and the code are updated
  together; a finding is not "confirmed" until both are updated.
- **`captures/`** — raw Wireshark/usbmon capture files kept as evidence,
  plus a small script to diff two captures and highlight changed bytes
  (the core tool for the one-change-at-a-time discovery method below).

## Discovery methodology

1. Check for existing documentation (official chart, community docs for
   related models) before capturing anything.
2. Baseline capture: connect/handshake sequence between the official app
   and the amp.
3. Full-patch read capture: trigger whatever action causes the app to
   read the complete current patch (e.g. switching patches). Likely the
   single highest-value capture, probably one bulk request/response
   covering the whole patch address range.
4. Systematic one-change-at-a-time captures: for each control (amp
   model, gain, each effect block's parameters, EQ, etc.), capture before
   and after changing exactly one value in the official app, and diff the
   two captures to isolate the address↔parameter mapping.
5. Ghidra pass over the binary for ID→name enumeration tables, to fill in
   value mappings (e.g. "amp model ID 7 = Brown High") without manually
   clicking through every option.
6. Encode each confirmed finding into `katana-proto` and `docs/protocol.md`
   together.

## Testing strategy

- **Golden-byte unit tests** (run in CI, no hardware required): given a
  real captured byte sequence, the encoder/decoder must reproduce it
  exactly.
- **Hardware-in-the-loop tests** (marked `#[ignore]` or feature-gated
  behind e.g. `--features hardware-tests`): read-modify-write-reread
  against the real amp. Run manually by the user, since CI has no device
  attached.

## Definition of done (phase 1)

- Full current-patch read from the amp.
- Byte-identical write-back of that same patch.
- Individual parameter writes confirmed to take effect on the amp
  (observed on the amp itself, not just "no error returned").
- All of the above backed by golden-byte tests derived from real
  captures.

Only once this bar is met does phase 2 (GUI) get its own design pass.
