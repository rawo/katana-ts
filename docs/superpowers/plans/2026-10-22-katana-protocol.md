# KATANA Gen 3 Protocol Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust library (`katana-proto`) and CLI (`katana-cli`) that read the full current patch from a BOSS KATANA Gen 3, write it back byte-identical, and write individual parameters that audibly take effect.

**Architecture:** A Cargo workspace with two crates. `katana-proto` holds pure, hardware-free protocol code (7-bit address arithmetic, Roland checksum, SysEx envelope, USB-MIDI packet reassembly, memory-map diffing, patch and parameter logic) behind a `Transport` trait, so everything except the `midir` adapter is unit-tested against golden bytes and a `FakeAmp`. Protocol facts (model ID, patch address ranges, parameter addresses) are only entered after a capture proves them, and are recorded in `docs/protocol.md` in the same commit.

**Tech Stack:** Rust (edition 2021), `midir` (ALSA sequencer MIDI I/O), `thiserror` (library errors), `clap` derive + `anyhow` (CLI), `usbmon` + `tshark`/Wireshark (capture), Ghidra (constant tables only).

**Spec:** `docs/superpowers/specs/2026-10-22-katana-protocol-design.md`

## Global Constraints

- Device: `lsusb` `0582:02f0 Roland Corp. KATANA3`, ALSA `KATANA3 MIDI 1` (`hw:3,0,0`), sequencer client 28 port 0. Class-compliant USB-MIDI — use `midir`, no raw USB crates.
- Nothing enters `katana-proto` protocol constants without a captured-and-verified byte sequence; a finding is "confirmed" only when code and `docs/protocol.md` are updated together, with an evidence note (capture file or Ghidra address).
- Checksum must be verified bit-exact against real captured checksums, not only Roland documentation.
- Ghidra is used only to read constant/string tables (ID → name). No decompiled logic is copied.
- Golden-byte tests run in CI with no hardware. Hardware tests live behind `--features hardware-tests` and are run manually by the user.
- Phase 2 (GUI) is out of scope.

## Review Focus

1. **Address arithmetic across 7-bit boundaries** — Roland addresses/sizes are base-128 digits; `00 00 7F 7F` + 1 must be `00 01 00 00`, and a 300-byte block must chunk correctly across that boundary. (Task 2 `address_carries_across_7bit_digit`, Task 7 `read_patch_spans_7bit_boundary`.)
2. **Amp unplugged, port missing, or no reply** — the CLI must print a clear error naming the expected port and exit non-zero; reads must time out, never hang. (Task 5 `open_missing_port_errors`, Task 7 `read_patch_times_out_when_amp_silent`.)
3. **Unrelated MIDI arriving during a read** — Program Change, active sensing, or another app's SysEx interleaved with or splitting the response must not corrupt the patch. (Task 4 `extract_ignores_non_sysex_packets`, Task 7 `read_patch_ignores_unrelated_messages`.)
4. **Out-of-range `set` values** — a value outside the parameter's range or above 0x7F is rejected before anything is sent. (Task 8 `set_rejects_out_of_range`.)
5. **Malformed or foreign `.syx` files on `load`** — truncated message, bad checksum, or another model's SysEx is rejected before anything is written to the amp. (Task 6 `syx_rejects_bad_checksum`, Task 7 `load_rejects_foreign_model`.)

---

### Task 1: Prior-art check and protocol doc skeleton

No code. Deliverable: `docs/protocol.md` with sourced findings and a go/no-go on the envelope assumption the later tasks build on.

**Files:**
- Create: `docs/protocol.md`

**Interfaces:**
- Produces: `docs/protocol.md` sections `## Sources`, `## Envelope`, `## Checksum`, `## Identity`, `## Address map` (table columns: `Address | Size | Name | Encoding | Range | Evidence`), `## Open questions`.

- [ ] **Step 1: Search for an official Roland/BOSS "MIDI Implementation" PDF for KATANA Gen 3** (boss.info support/downloads for KATANA-50/100/Artist Gen 3). Record URL and what it covers, or "none found".
- [ ] **Step 2: Search for community reverse-engineering of KATANA MkI/MkII/Gen 3 SysEx** (GitHub, VGuitarForums, FxFloorboard/KatanaFxFloorboard, katana-related Python/JS projects). Record each source: model it targets, claimed model ID, address layout, checksum.
- [ ] **Step 3: Write `docs/protocol.md`** with every inherited claim marked `UNVERIFIED (source: …)`. Expected envelope to verify: `F0 41 <dev> <model-id…> <cmd 11=RQ1|12=DT1> <addr×N> <data|size×N> <checksum> F7`, checksum = `(128 - (sum(addr+data) % 128)) % 128`.
- [ ] **Step 4: Go/no-go.** If sources indicate Gen 3 does *not* use Roland RQ1/DT1 framing, stop and report to the user — Tasks 3, 7, 8 would need redesign. Otherwise continue.
- [ ] **Step 5: Commit**

```bash
git add docs/protocol.md
git commit -m "docs: record KATANA prior art and protocol doc skeleton"
```

---

### Task 2: Workspace, 7-bit `Address`, Roland checksum

**Files:**
- Create: `Cargo.toml` (workspace, members `katana-proto`, `katana-cli`), `.gitignore` (`/target`)
- Create: `katana-proto/Cargo.toml` (deps: `thiserror`; optional `midir` behind default feature `midi`), `katana-proto/src/lib.rs`, `katana-proto/src/error.rs`, `katana-proto/src/address.rs`, `katana-proto/src/checksum.rs`
- Create: `katana-cli/Cargo.toml` (deps: `katana-proto`, `clap` with `derive`, `anyhow`), `katana-cli/src/main.rs` (clap skeleton, no subcommands yet)

**Interfaces:**
- Produces:
  - `pub enum ProtoError` (thiserror) in `error.rs`; variants added by later tasks. Start with `NotSevenBit { index: usize, byte: u8 }`, `AddressOverflow`.
  - `pub struct Address(pub u32)` — linear value of base-128 digits. `Address::from_bytes(&[u8]) -> Result<Address, ProtoError>` (rejects bytes > 0x7F), `Address::to_bytes(self, width: usize) -> Result<Vec<u8>, ProtoError>` (errors if value doesn't fit `width` digits), `impl Add<u32> for Address`, `Ord`, `Copy`, `Debug` printing as spaced hex digits (`00 01 00 00`).
  - Sizes use the same type alias: `pub type Size = Address;`
  - `pub fn roland_checksum(bytes: &[u8]) -> u8` in `checksum.rs`.

- [ ] **Step 1: Write failing tests** in `address.rs` / `checksum.rs` `#[cfg(test)]` modules:

```rust
#[test] fn address_round_trips() {
    let a = Address::from_bytes(&[0x60, 0x00, 0x12, 0x34]).unwrap();
    assert_eq!(a.to_bytes(4).unwrap(), vec![0x60, 0x00, 0x12, 0x34]);
}
#[test] fn address_carries_across_7bit_digit() {
    let a = Address::from_bytes(&[0x00, 0x00, 0x7F, 0x7F]).unwrap() + 1;
    assert_eq!(a.to_bytes(4).unwrap(), vec![0x00, 0x01, 0x00, 0x00]);
}
#[test] fn address_rejects_8bit_byte() {
    assert!(matches!(Address::from_bytes(&[0x00, 0x80]), Err(ProtoError::NotSevenBit { index: 1, byte: 0x80 })));
}
#[test] fn address_rejects_too_wide() {
    assert!(Address(128 * 128).to_bytes(2).is_err());
}
// Roland GS Reset, published by Roland: F0 41 10 42 12 40 00 7F 00 41 F7
#[test] fn checksum_matches_gs_reset() {
    assert_eq!(roland_checksum(&[0x40, 0x00, 0x7F, 0x00]), 0x41);
}
#[test] fn checksum_of_zero_sum_is_zero() {
    assert_eq!(roland_checksum(&[0x00, 0x00]), 0x00);
}
```

- [ ] **Step 2: Run** `cargo test -p katana-proto` — Expected: compile errors (types not defined).
- [ ] **Step 3: Implement** `Address`, `ProtoError`, `roland_checksum` as specified. Make `midir` an optional dependency (`default = ["midi"]`) so CI can build `--no-default-features` without ALSA headers.
- [ ] **Step 4: Run** `cargo test -p katana-proto` — Expected: 6 passed. Also `cargo build --workspace` succeeds.
- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore katana-proto katana-cli
git commit -m "feat(proto): workspace, 7-bit address arithmetic, Roland checksum"
```

---

### Task 3: SysEx envelope (RQ1/DT1) and Universal Identity

**Files:**
- Create: `katana-proto/src/sysex.rs`, `katana-proto/src/identity.rs`
- Modify: `katana-proto/src/error.rs` (new variants), `katana-proto/src/lib.rs`

**Interfaces:**
- Consumes: `Address`, `Size`, `roland_checksum`, `ProtoError` (Task 2).
- Produces:
  - `pub struct Header { pub device_id: u8, pub model_id: Vec<u8>, pub addr_width: usize }`
  - `pub enum Command { Rq1 { addr: Address, size: Size }, Dt1 { addr: Address, data: Vec<u8> } }`
  - `pub fn encode(h: &Header, c: &Command) -> Result<Vec<u8>, ProtoError>`
  - `pub fn decode(h: &Header, msg: &[u8]) -> Result<Command, ProtoError>`
  - `ProtoError` variants: `NotSysEx`, `Truncated`, `WrongManufacturer(u8)`, `WrongModel { expected: Vec<u8>, got: Vec<u8> }`, `UnknownCommand(u8)`, `BadChecksum { expected: u8, got: u8 }`
  - `identity.rs`: `pub const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];` and `pub struct IdentityReply { pub device_id: u8, pub manufacturer: u8, pub family: [u8; 2], pub member: [u8; 2], pub version: [u8; 4] }` with `pub fn parse_identity_reply(msg: &[u8]) -> Result<IdentityReply, ProtoError>` (format `F0 7E <dev> 06 02 <mfr> <fam×2> <mem×2> <ver×4> F7`).

- [ ] **Step 1: Write failing tests** in `sysex.rs`:

```rust
fn gs() -> Header { Header { device_id: 0x10, model_id: vec![0x42], addr_width: 3 } }
const GS_RESET: [u8; 11] = [0xF0,0x41,0x10,0x42,0x12,0x40,0x00,0x7F,0x00,0x41,0xF7];

#[test] fn encodes_gs_reset() {
    let c = Command::Dt1 { addr: Address::from_bytes(&[0x40,0x00,0x7F]).unwrap(), data: vec![0x00] };
    assert_eq!(encode(&gs(), &c).unwrap(), GS_RESET.to_vec());
}
#[test] fn decodes_gs_reset() {
    assert_eq!(decode(&gs(), &GS_RESET).unwrap(),
        Command::Dt1 { addr: Address::from_bytes(&[0x40,0x00,0x7F]).unwrap(), data: vec![0x00] });
}
#[test] fn rq1_round_trips() {
    let h = Header { device_id: 0x10, model_id: vec![0x00,0x00,0x00,0x33], addr_width: 4 };
    let c = Command::Rq1 { addr: Address::from_bytes(&[0x60,0,0,0]).unwrap(), size: Address(0x200) };
    assert_eq!(decode(&h, &encode(&h, &c).unwrap()).unwrap(), c);
}
#[test] fn rejects_bad_checksum() {
    let mut m = GS_RESET; m[9] = 0x42;
    assert!(matches!(decode(&gs(), &m), Err(ProtoError::BadChecksum { expected: 0x41, got: 0x42 })));
}
#[test] fn rejects_truncated() { assert!(matches!(decode(&gs(), &GS_RESET[..6]), Err(ProtoError::Truncated))); }
#[test] fn rejects_wrong_model() {
    let h = Header { model_id: vec![0x43], ..gs() };
    assert!(matches!(decode(&h, &GS_RESET), Err(ProtoError::WrongModel { .. })));
}
#[test] fn encode_rejects_8bit_data() {
    let c = Command::Dt1 { addr: Address(0), data: vec![0x80] };
    assert!(encode(&gs(), &c).is_err());
}
```

In `identity.rs`:

```rust
#[test] fn parses_identity_reply() {
    let m = [0xF0,0x7E,0x10,0x06,0x02,0x41,0x33,0x03,0x00,0x00,0x00,0x01,0x00,0x00,0xF7];
    let r = parse_identity_reply(&m).unwrap();
    assert_eq!((r.manufacturer, r.family, r.member), (0x41, [0x33,0x03], [0x00,0x00]));
}
```

- [ ] **Step 2: Run** `cargo test -p katana-proto` — Expected: FAIL (undefined items).
- [ ] **Step 3: Implement** `encode`/`decode`/`parse_identity_reply`. Checksum covers address + data (or address + size) bytes only. Derive `PartialEq, Debug, Clone` on `Command`.
- [ ] **Step 4: Run** `cargo test -p katana-proto` — Expected: all pass.
- [ ] **Step 5: Commit**

```bash
git add katana-proto
git commit -m "feat(proto): Roland RQ1/DT1 envelope and MIDI identity reply parsing"
```

---

### Task 4: Capture tooling — USB-MIDI reassembly and tshark export loading

**Files:**
- Create: `katana-proto/src/capture.rs`
- Create: `captures/tools/capture.sh`, `captures/tools/export.sh`, `captures/README.md`

**Interfaces:**
- Produces:
  - `pub enum Direction { HostToAmp, AmpToHost }`
  - `pub fn extract_sysex(packets: &[u8]) -> Vec<Vec<u8>>` — input is a concatenation of 4-byte USB-MIDI event packets (USB-MIDI 1.0 §4); CIN `0x4` = SysEx start/continue (3 bytes), `0x5` = end with 1 byte, `0x6` = end with 2, `0x7` = end with 3; all other CINs ignored; an incomplete trailing SysEx is dropped.
  - `pub struct CapturedMessage { pub dir: Direction, pub bytes: Vec<u8> }`
  - `pub fn load_tshark_export(text: &str) -> Result<Vec<CapturedMessage>, ProtoError>` — one line per URB: `<direction>\t<hex>` where direction `0`=out(HostToAmp), `1`=in(AmpToHost), hex with or without `:` separators; blank lines and lines with empty hex skipped. SysEx reassembly is per-direction across lines. New `ProtoError::BadExport { line: usize }`.

- [ ] **Step 1: Write failing tests** in `capture.rs`:

```rust
const GS_RESET: [u8; 11] = [0xF0,0x41,0x10,0x42,0x12,0x40,0x00,0x7F,0x00,0x41,0xF7];
const GS_PACKETS: [u8; 16] = [0x04,0xF0,0x41,0x10, 0x04,0x42,0x12,0x40, 0x04,0x00,0x7F,0x00, 0x06,0x41,0xF7,0x00];

#[test] fn extracts_sysex_from_packets() { assert_eq!(extract_sysex(&GS_PACKETS), vec![GS_RESET.to_vec()]); }
#[test] fn extract_ignores_non_sysex_packets() {
    let mut p = vec![0x0F,0xFE,0x00,0x00, 0x0C,0xC0,0x05,0x00]; // active sensing, program change
    p.extend_from_slice(&GS_PACKETS);
    assert_eq!(extract_sysex(&p), vec![GS_RESET.to_vec()]);
}
#[test] fn extract_drops_incomplete_trailing_sysex() { assert!(extract_sysex(&GS_PACKETS[..8]).is_empty()); }
#[test] fn loads_export_split_across_urbs() {
    let text = "0\t04:f0:41:10:04:42:12:40\n1\t\n0\t04007f000641f700";
    let msgs = load_tshark_export(text).unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].bytes, GS_RESET.to_vec());
    assert!(matches!(msgs[0].dir, Direction::HostToAmp));
}
#[test] fn export_rejects_garbage_line() {
    assert!(matches!(load_tshark_export("0\tzz"), Err(ProtoError::BadExport { line: 1 })));
}
```

- [ ] **Step 2: Run** `cargo test -p katana-proto capture` — Expected: FAIL.
- [ ] **Step 3: Implement** `capture.rs` as specified.
- [ ] **Step 4: Run** `cargo test -p katana-proto capture` — Expected: 5 passed.
- [ ] **Step 5: Write the scripts.**
  - `captures/tools/capture.sh <name>`: `sudo modprobe usbmon`; find the bus of `0582:02f0` via `lsusb`; run `tshark -i usbmon<bus> -w captures/<name>.pcapng` until Ctrl-C.
  - `captures/tools/export.sh <file.pcapng>`: `tshark -r <file> -Y 'usb.idVendor == 0x0582 || usb.capdata' -T fields -e usb.endpoint_address.direction -e usb.capdata > <file>.tsv` (filter by the device's bus/device address, printed by `capture.sh`).
  - `captures/README.md`: the session procedure (start capture → do exactly one action in the official app → stop → export → `katana-cli extract`/`diff`), naming convention `NN-short-description.pcapng`, and that `.pcapng` + `.tsv` are both committed as evidence.
- [ ] **Step 6: Verify scripts** by running `shellcheck captures/tools/*.sh` — Expected: no errors. (Live verification happens in Task 6.)
- [ ] **Step 7: Commit**

```bash
git add katana-proto/src/capture.rs katana-proto/src/lib.rs katana-proto/src/error.rs captures
git commit -m "feat(proto): USB-MIDI SysEx reassembly and usbmon capture tooling"
```

---

### Task 5: Transport, midir adapter, FakeAmp, and `ports` / `identity` / `monitor` / `extract` / `diff` commands

**Files:**
- Create: `katana-proto/src/transport.rs`, `katana-proto/src/midi.rs` (`#[cfg(feature = "midi")]`), `katana-proto/src/memmap.rs`, `katana-proto/src/fake.rs` (`#[cfg(any(test, feature = "testing"))]`)
- Create: `katana-cli/src/cmd/{mod.rs,ports.rs,identity.rs,monitor.rs,extract.rs,diff.rs}`
- Modify: `katana-cli/src/main.rs`, `katana-proto/src/lib.rs`, `katana-proto/Cargo.toml` (feature `testing`, feature `hardware-tests`)

**Interfaces:**
- Consumes: `Header`, `Command`, `encode`, `decode` (Task 3); `load_tshark_export`, `Direction` (Task 4).
- Produces:
  - `pub trait Transport { fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError>; fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError>; }` — `recv` returns only complete SysEx messages; `Ok(None)` on timeout.
  - `pub struct MidirTransport` with `pub fn open(port_substring: &str) -> Result<Self, ProtoError>` and `pub fn list_ports() -> Result<Vec<String>, ProtoError>`. Must call `MidiInput::ignore(Ignore::None)` (midir drops SysEx by default). Input callback forwards over `std::sync::mpsc`. New error `ProtoError::PortNotFound { wanted: String, available: Vec<String> }`, `ProtoError::Midi(String)`.
  - `pub const DEFAULT_PORT: &str = "KATANA3";`
  - `pub struct MemoryMap(pub BTreeMap<u32, u8>)` with `pub fn apply(&mut self, addr: Address, data: &[u8])`, `pub fn from_commands<'a>(cmds: impl IntoIterator<Item = &'a Command>) -> MemoryMap` (DT1 only), `pub fn diff(&self, other: &MemoryMap) -> Vec<(Address, Option<u8>, Option<u8>)>`, `pub fn read(&self, addr: Address, len: u32) -> Option<Vec<u8>>`.
  - `pub struct FakeAmp { pub header: Header, pub mem: MemoryMap, pub sent: Vec<Vec<u8>>, pub max_dt1: usize, pub silent: bool, pub noise: Vec<Vec<u8>> }` implementing `Transport`: on RQ1, queues DT1 replies split into `max_dt1`-byte chunks (preceded by any `noise` messages); on DT1, applies to `mem`; records every sent message in `sent`; `silent` makes it never reply.
  - CLI (`clap`): global `--port <substring>` (default `DEFAULT_PORT`); subcommands `ports`, `identity`, `monitor`, `extract <file>`, `diff <a> <b>`. `extract`/`diff` accept `.tsv` (Task 4 export) and, after Task 6, `.syx`. `diff` decodes with a `Header` whose `model_id` comes from `--model-id <hex>` until Task 6 fixes the constant.

- [ ] **Step 1: Write failing tests**:

```rust
// memmap.rs
#[test] fn diff_reports_changed_byte() {
    let mut a = MemoryMap::default(); a.apply(Address(0x100), &[1, 2, 3]);
    let mut b = a.clone(); b.apply(Address(0x101), &[9]);
    assert_eq!(a.diff(&b), vec![(Address(0x101), Some(2), Some(9))]);
}
#[test] fn read_requires_full_coverage() {
    let mut m = MemoryMap::default(); m.apply(Address(0), &[1, 2]);
    assert_eq!(m.read(Address(0), 2), Some(vec![1, 2]));
    assert_eq!(m.read(Address(0), 3), None);
}
// fake.rs
#[test] fn fake_amp_answers_rq1_in_chunks() {
    let h = Header { device_id: 0x10, model_id: vec![0x01], addr_width: 4 };
    let mut amp = FakeAmp::new(h.clone(), 4);
    amp.mem.apply(Address(0), &[1, 2, 3, 4, 5, 6]);
    amp.send(&encode(&h, &Command::Rq1 { addr: Address(0), size: Address(6) }).unwrap()).unwrap();
    let first = decode(&h, &amp.recv(Duration::ZERO).unwrap().unwrap()).unwrap();
    assert_eq!(first, Command::Dt1 { addr: Address(0), data: vec![1, 2, 3, 4] });
}
// midi.rs, behind feature "midi"
#[test] fn open_missing_port_errors() {
    assert!(matches!(MidirTransport::open("no-such-port-xyz"), Err(ProtoError::PortNotFound { .. })));
}
```

Hardware test in `katana-proto/tests/hardware.rs` with `#![cfg(feature = "hardware-tests")]`:

```rust
#[test] fn amp_answers_identity_request() {
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    t.send(&IDENTITY_REQUEST).unwrap();
    let reply = t.recv(Duration::from_secs(2)).unwrap().expect("no identity reply");
    assert_eq!(parse_identity_reply(&reply).unwrap().manufacturer, 0x41);
}
```

- [ ] **Step 2: Run** `cargo test -p katana-proto` — Expected: FAIL (undefined items).
- [ ] **Step 3: Implement** `Transport`, `MidirTransport`, `MemoryMap`, `FakeAmp`.
- [ ] **Step 4: Run** `cargo test -p katana-proto` — Expected: all pass (if the sandbox has no ALSA sequencer, `open_missing_port_errors` may return `Midi(_)`; it must still be an `Err`, never a panic — accept either variant in that case).
- [ ] **Step 5: Implement CLI subcommands.** `ports` prints one port per line. `identity` sends `IDENTITY_REQUEST`, prints the parsed reply or "no reply within 2s" (exit 1). `monitor` prints each received SysEx as spaced hex, plus `RQ1 <addr> <size>` / `DT1 <addr> <data>` when it decodes; note in `--help` that it only sees amp→host traffic (host→amp needs usbmon). `extract` prints `> ` / `< ` prefixed hex lines. `diff` prints `addr: old -> new` for each changed byte.
- [ ] **Step 6: Run** `cargo run -p katana-cli -- --port no-such diff /dev/null /dev/null` — Expected: exit 0, no output. `cargo run -p katana-cli -- --port no-such identity` — Expected: exit 1, error listing available ports.
- [ ] **Step 7: Hardware check (user).** With the amp connected and the official app closed: `cargo run -p katana-cli -- ports` lists `KATANA3 MIDI 1`; `cargo test -p katana-proto --features hardware-tests` passes.
- [ ] **Step 8: Commit**

```bash
git add katana-proto katana-cli
git commit -m "feat: MIDI transport, fake amp, memory-map diff, and discovery CLI commands"
```

---

### Task 6: Discovery session 1 — identity, envelope, full-patch read; `.syx` files

Human-in-the-loop with the amp and the official app under Wine. Produces the first confirmed protocol constants and golden tests from real captures.

**Files:**
- Create: `katana-proto/src/device.rs`, `katana-proto/src/syx.rs`, `katana-proto/tests/golden_captures.rs`, `katana-proto/tests/data/` (copies of the minimal captured `.tsv` fixtures the golden tests read)
- Create: `captures/01-identity.*`, `captures/02-app-connect.*`, `captures/03-patch-select.*`
- Modify: `docs/protocol.md`, `katana-cli/src/cmd/{extract,diff}.rs` (accept `.syx`, default to `device::header()`)

**Interfaces:**
- Consumes: everything from Tasks 2–5.
- Produces:
  - `device.rs`: `pub const DEVICE_ID: u8`, `pub const MODEL_ID: &[u8]`, `pub const ADDR_WIDTH: usize`, `pub fn header() -> Header`, `pub const PATCH_BLOCKS: &[(u32 /*addr*/, u32 /*size*/)]` (the RQ1 requests the official app issues to read the current patch, in its order), `pub const MAX_DT1_DATA: usize` (largest DT1 payload the app/amp use). Each constant carries a `// evidence: captures/NN-….tsv` comment.
  - `syx.rs`: `pub fn read_syx(bytes: &[u8]) -> Result<Vec<Vec<u8>>, ProtoError>` (splits concatenated `F0…F7` messages; error on data outside a message or unterminated message), `pub fn write_syx(msgs: &[Vec<u8>]) -> Vec<u8>`.

- [ ] **Step 1: Identity.** Run `katana-cli identity`; save output to `captures/01-identity.txt`. Record family/member/version in `docs/protocol.md` `## Identity`.
- [ ] **Step 2: Connect capture.** `captures/tools/capture.sh 02-app-connect`, launch the official app under Wine, wait for it to finish loading, close it, stop capture, export. Run `katana-cli extract captures/02-app-connect.pcapng.tsv`.
- [ ] **Step 3: Confirm the envelope** from step 2: manufacturer `0x41`, device ID, model-ID bytes, address width, RQ1/DT1 command bytes. Recompute `roland_checksum` for at least 5 captured messages in each direction; all must match. If any doesn't, stop and report — the checksum assumption is wrong.
- [ ] **Step 4: Patch-read capture.** `capture.sh 03-patch-select`, with the app open select a different patch, stop, export. Identify the RQ1 block list and DT1 chunk size → `PATCH_BLOCKS`, `MAX_DT1_DATA`.
- [ ] **Step 5: Write `device.rs`** with the confirmed values and evidence comments; update `docs/protocol.md` (`## Envelope`, `## Checksum` marked VERIFIED with capture references; patch block ranges into `## Address map`).
- [ ] **Step 6: Write golden tests** in `tests/golden_captures.rs`, reading fixtures from `tests/data/`:

```rust
#[test] fn every_captured_message_decodes_and_reencodes_identically() {
    for m in load_tshark_export(include_str!("data/03-patch-select.tsv")).unwrap() {
        let cmd = decode(&device::header(), &m.bytes).unwrap();
        assert_eq!(encode(&device::header(), &cmd).unwrap(), m.bytes);
    }
}
#[test] fn captured_patch_read_matches_patch_blocks() {
    let rq1: Vec<_> = /* HostToAmp RQ1 commands from 03-patch-select */;
    assert_eq!(rq1, device::PATCH_BLOCKS.iter().map(|&(a, s)| Command::Rq1 { addr: Address(a), size: Address(s) }).collect::<Vec<_>>());
}
```

Plus in `syx.rs`:

```rust
#[test] fn syx_round_trips() { let m = vec![GS_RESET.to_vec(), GS_RESET.to_vec()]; assert_eq!(read_syx(&write_syx(&m)).unwrap(), m); }
#[test] fn syx_rejects_unterminated() { assert!(read_syx(&GS_RESET[..5]).is_err()); }
#[test] fn syx_rejects_bad_checksum() { /* read_syx is framing-only; assert decode(&header, &msg) of a corrupted-checksum message from the file errors with BadChecksum */ }
```

- [ ] **Step 7: Run** `cargo test -p katana-proto` — Expected: all pass, including the golden tests.
- [ ] **Step 8: Commit**

```bash
git add katana-proto katana-cli captures docs/protocol.md
git commit -m "feat(proto): confirm KATANA Gen 3 envelope and patch layout from captures"
```

---

### Task 7: Full-patch read and byte-identical write-back; `dump` / `load` / `verify`

**Files:**
- Create: `katana-proto/src/patch.rs`, `katana-cli/src/cmd/{dump,load,verify}.rs`
- Modify: `katana-proto/tests/hardware.rs`, `katana-cli/src/main.rs`, `docs/protocol.md`

**Interfaces:**
- Consumes: `Transport`, `FakeAmp`, `MemoryMap`, `device::{header, PATCH_BLOCKS, MAX_DT1_DATA}`, `read_syx`/`write_syx`.
- Produces:
  - `pub struct Patch { pub mem: MemoryMap }` — exactly the bytes covered by `PATCH_BLOCKS`.
  - `pub fn read_patch(t: &mut dyn Transport, timeout: Duration) -> Result<Patch, ProtoError>` — sends each RQ1 in `PATCH_BLOCKS` order; collects DT1 replies until the block is fully covered; ignores messages that don't decode for this model or fall outside the block; per-block deadline `timeout` → `ProtoError::Timeout { addr: Address }`.
  - `pub fn write_patch(t: &mut dyn Transport, p: &Patch) -> Result<(), ProtoError>` — DT1 per block, chunked at `MAX_DT1_DATA`, chunk addresses via `Address + offset`.
  - `impl Patch { pub fn to_syx(&self) -> Vec<u8>; pub fn from_syx(bytes: &[u8]) -> Result<Patch, ProtoError> }` — `from_syx` errors (`ProtoError::IncompletePatch`) unless every `PATCH_BLOCKS` byte is covered, and fails on any message that doesn't decode for `device::header()`.
  - CLI: `dump <out.syx>`, `load <in.syx>`, `verify` (dump → load → dump, compare, print first differing address or "identical").

- [ ] **Step 1: Write failing tests** in `patch.rs` (use a test-local block list via `read_patch_blocks(t, blocks, timeout)` internal helper so tests don't depend on real addresses):

```rust
#[test] fn read_then_write_is_byte_identical() { /* FakeAmp filled with pseudo-random 7-bit bytes over all blocks; read_patch; fresh FakeAmp; write_patch; assert amp2.mem == amp1.mem */ }
#[test] fn read_patch_spans_7bit_boundary() { /* block (0x7F00 as linear of 00 00 7F 00, size 300), max_dt1 128 → patch covers all 300 bytes; DT1 addresses in amp.sent equal 00 00 7F 00, 00 01 00 00, … */ }
#[test] fn read_patch_ignores_unrelated_messages() { /* FakeAmp.noise = [identity reply, a DT1 for another model id, a DT1 outside the block]; read succeeds and equals the clean read */ }
#[test] fn read_patch_times_out_when_amp_silent() { /* FakeAmp.silent = true; read_patch(.., 50ms) returns Err(Timeout{..}) */ }
#[test] fn syx_round_trip_preserves_patch() { /* Patch::from_syx(&p.to_syx()) == p */ }
#[test] fn load_rejects_foreign_model() { /* .syx of a DT1 with a different model id → from_syx errors */ }
#[test] fn from_syx_rejects_incomplete_patch() { /* drop one chunk → IncompletePatch */ }
```

Hardware test (`tests/hardware.rs`):

```rust
#[test] fn hardware_patch_round_trip_is_byte_identical() {
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let a = read_patch(&mut t, Duration::from_secs(2)).unwrap();
    write_patch(&mut t, &a).unwrap();
    let b = read_patch(&mut t, Duration::from_secs(2)).unwrap();
    assert_eq!(a, b);
}
```

- [ ] **Step 2: Run** `cargo test -p katana-proto patch` — Expected: FAIL.
- [ ] **Step 3: Implement** `patch.rs` as specified.
- [ ] **Step 4: Run** `cargo test -p katana-proto` — Expected: all pass.
- [ ] **Step 5: Implement CLI** `dump`, `load`, `verify`.
- [ ] **Step 6: Golden test from capture.** Add to `golden_captures.rs`: build the amp→host DT1s of `03-patch-select.tsv` into a `Patch`; `write_patch` into a `FakeAmp`; assert the resulting DT1 messages in `sent` equal the captured amp→host DT1 bytes (same chunking), or, if the app's write chunking differs, capture `04-patch-write` (app "write patch") and compare against that instead.
- [ ] **Step 7: Hardware check (user).** Official app closed. `cargo test -p katana-proto --features hardware-tests` passes; `katana-cli verify` prints "identical"; `katana-cli dump a.syx`, change a knob on the amp, `katana-cli load a.syx` — the knob's sound returns to the dumped state.
- [ ] **Step 8: Commit**

```bash
git add katana-proto katana-cli captures docs/protocol.md
git commit -m "feat: full-patch dump/load with byte-identical round-trip"
```

---

### Task 8: Typed parameters and `set`; per-parameter discovery loop

**Files:**
- Create: `katana-proto/src/params.rs`, `katana-cli/src/cmd/{set,params}.rs`
- Create: `captures/1NN-<param>-before.*`, `captures/1NN-<param>-after.*` per parameter
- Modify: `docs/protocol.md`, `katana-proto/tests/golden_captures.rs`

**Interfaces:**
- Consumes: `Transport`, `encode`, `Command::Dt1`, `device::header`, `MemoryMap::diff`, `Patch`.
- Produces:
  - `pub enum Encoding { U7 }` — add variants (e.g. multi-byte nibbles) only when a capture shows one, with a golden test for it.
  - `pub struct ParamDef { pub name: &'static str, pub addr: u32, pub encoding: Encoding, pub min: u32, pub max: u32, pub values: Option<&'static [(u32, &'static str)]> }`
  - `pub const PARAMS: &[ParamDef]`; `pub fn find_param(name: &str) -> Option<&'static ParamDef>`
  - `pub fn encode_param(p: &ParamDef, value: u32) -> Result<Vec<u8>, ProtoError>` → new `ProtoError::OutOfRange { name: &'static str, value: u32, min: u32, max: u32 }`
  - `pub fn set_param(t: &mut dyn Transport, p: &ParamDef, value: u32) -> Result<(), ProtoError>` — one DT1, nothing sent on error.
  - `impl Patch { pub fn get(&self, p: &ParamDef) -> Option<u32> }`
  - CLI: `set <name> <value>` (value is a number or a name from `values`), `params` (list name, range, current value from a live read).

- [ ] **Step 1: Write failing tests** with a test-local `ParamDef` (`addr: 0x10, U7, 0..=100`):

```rust
#[test] fn set_sends_single_dt1() { /* FakeAmp; set_param(..,50); amp.sent == [encode(header, Dt1{addr:0x10, data:[50]})] */ }
#[test] fn set_rejects_out_of_range() { /* set_param(..,101) → Err(OutOfRange{..}); amp.sent.is_empty() */ }
#[test] fn patch_get_reads_param() { /* Patch with mem[0x10]=42 → get == Some(42) */ }
```

- [ ] **Step 2: Run** `cargo test -p katana-proto params` — Expected: FAIL.
- [ ] **Step 3: Implement** `params.rs` and the `set` / `params` commands. `PARAMS` starts empty.
- [ ] **Step 4: Run** `cargo test -p katana-proto` — Expected: all pass.
- [ ] **Step 5: Commit** — `git commit -m "feat: typed parameter definitions and set command"`.
- [ ] **Step 6: Discovery loop, repeated per parameter** (start with: amp type, gain, volume, bass, middle, treble, presence, then each effect block's on/off and type). For each:
  1. `katana-cli dump before.syx`; change exactly that one control on the amp or in the official app; `katana-cli dump after.syx`; `katana-cli diff before.syx after.syx` → the address(es) and values.
  2. Sweep min→max to fix the range and encoding; also capture the official app's DT1 for that control with usbmon (`1NN-<param>.pcapng`).
  3. Add the `ParamDef` row (`// evidence:` comment), the `docs/protocol.md` row, and a golden test that `encode(header, Dt1 from encode_param(p, v))` equals the app's captured bytes.
  4. `katana-cli set <name> <value>` and confirm by ear / amp display.
  5. Commit per parameter or small group: `feat(params): add <names>`.
- [ ] **Step 7: Run** `cargo test -p katana-proto` after each addition — Expected: all pass.

---

### Task 9: Enumerations via Ghidra and Definition-of-Done check

**Files:**
- Modify: `katana-proto/src/params.rs` (`values` tables), `docs/protocol.md`, `katana-proto/tests/golden_captures.rs`

- [ ] **Step 1: Ghidra pass.** Open the official `.exe` in Ghidra; search defined strings for known display names (e.g. amp-type names seen in the app UI); follow references to the adjacent ID→name tables. Record each table's binary address in `docs/protocol.md` as evidence. Copy only ID/name pairs.
- [ ] **Step 2: Fill `values`** for each enumerated `ParamDef`; for each table, verify at least two non-adjacent IDs on hardware with `katana-cli set` (the amp's sound/display matches the name). Add a test that every `values` entry lies within `min..=max`.
- [ ] **Step 3: Run** `cargo test -p katana-proto` — Expected: all pass.
- [ ] **Step 4: Definition-of-done check (user, hardware).** All must hold:
  - `katana-cli verify` → "identical" on at least three different patches.
  - `cargo test -p katana-proto --features hardware-tests` passes.
  - Every row in `docs/protocol.md` `## Address map` has an evidence entry and a matching `ParamDef` or `PATCH_BLOCKS` entry.
  - `cargo test -p katana-proto --no-default-features` passes with no amp connected and no ALSA headers (CI check).
- [ ] **Step 5: Commit**

```bash
git add katana-proto docs/protocol.md
git commit -m "feat(params): enumeration names from binary tables; phase 1 complete"
```
