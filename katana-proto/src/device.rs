//! Confirmed protocol constants for the KATANA Gen 3. Evidence for each is in
//! `captures/` and `docs/protocol.md`.

use crate::Header;

/// evidence: captures/01-identity.txt (identity reply device ID 10)
pub const DEVICE_ID: u8 = 0x10;
/// evidence: amp answers RQ1 with this 3-byte model ID (a 4-byte `00 01 05 07`
/// got no reply); captures/02-patch-read-dt1.syx
pub const MODEL_ID: &[u8] = &[0x01, 0x05, 0x07];
/// evidence: captures/02-patch-read-*.syx
pub const ADDR_WIDTH: usize = 4;

pub fn header() -> Header {
    Header { device_id: DEVICE_ID, model_id: MODEL_ID.to_vec(), addr_width: ADDR_WIDTH }
}

/// Linear value of a 4-digit base-128 address.
pub const fn addr(b: [u8; 4]) -> u32 {
    (b[0] as u32) << 21 | (b[1] as u32) << 14 | (b[2] as u32) << 7 | b[3] as u32
}

/// Largest DT1 payload we send: keeps each message within the 128-byte SysEx
/// limit the official app uses (13 bytes of envelope). The amp's own replies
/// can be longer (225-byte FX_DETAIL blocks arrive in one DT1).
pub const MAX_DT1_DATA: usize = 128 - 13;

/// Pause between messages we send, as the official app does (S1 `interval`).
pub const SEND_INTERVAL: std::time::Duration = std::time::Duration::from_millis(20);

/// Blocks after which the amp needs time before the next write. A variation
/// colour change is applied asynchronously and re-derives each effect's on/off
/// from its knob byte, overwriting an on/off written 20 ms later.
/// COLOR (`20 00 04 00`) — evidence: 2026-10-22, colour 2->1 then booster
/// on/off 0: with a 20 ms gap reads back 1, with 300 ms reads back 0.
pub const SETTLE_AFTER: &[(u32, std::time::Duration)] =
    &[(addr([0x20, 0x00, 0x04, 0x00]), std::time::Duration::from_millis(300))];

/// The COLOR block: the colours of booster, mod, FX, delay and reverb.
const COLOR_BLOCK: (u32, u32) = (addr([0x20, 0x00, 0x04, 0x00]), 5);

/// Pause owed after a single write to `a`. `SETTLE_AFTER` goes by block for a
/// whole patch; one parameter can be any colour in the block.
pub fn settle_after_param(a: u32) -> std::time::Duration {
    if (COLOR_BLOCK.0..COLOR_BLOCK.0 + COLOR_BLOCK.1).contains(&a) {
        SETTLE_AFTER[0].1
    } else {
        std::time::Duration::ZERO
    }
}

/// Current-patch ranges the amp accepts DT1 for but never changes (no error,
/// value kept). Skipped by `write_patch`, excluded from write-back checks.
/// SOLO_DELAY (`20 00 3E 00`, 14 bytes) — evidence: 2026-10-22 writes of
/// level 0/30/1, feedback +1 and on/off 1 (solo off and on) all read back
/// unchanged; a full PATCH(1) write changed 79 of 80 differing bytes, all but
/// this block. See docs/protocol.md.
pub const WRITE_PROTECTED: &[(u32, u32)] = &[(addr([0x20, 0x00, 0x3E, 0x00]), 14)];

/// `SETUP PATCH_NUMBER`: the selected channel, 2 bytes INTEGER2x4 (value =
/// `b0 << 4 | b1`); 0 = PANEL, 1–8 = A1–B4. Readable with RQ1 at any time; a
/// DT1 to it is ignored. While editor mode is on, the amp sends a DT1 of it by
/// itself after a channel change. Evidence: docs/protocol.md, "Channels" and
/// "Amp control" (2026-10-22).
pub const SELECTOR: u32 = addr([0x00, 0x00, 0x00, 0x00]);
/// `EDITOR_COMMUNICATION_MODE`: DT1 `01` on, `00` off. Evidence: docs/protocol.md, "Amp control".
pub const EDITOR_MODE: u32 = addr([0x7F, 0x00, 0x00, 0x01]);
/// DT1 `00 <selector>` selects a channel, only while editor mode is on.
/// Evidence: docs/protocol.md, "Amp control".
pub const PATCH_SELECT: u32 = addr([0x7F, 0x00, 0x01, 0x00]);
/// DT1 `00 <selector>` stores the current patch into that channel, only while
/// editor mode is on; the amp echoes it. Evidence: docs/protocol.md, "Amp
/// control" (A:CH4 store and restore, 2026-10-22).
pub const PATCH_WRITE: u32 = addr([0x7F, 0x00, 0x01, 0x04]);
/// Pause after the selector shows a new channel, before writing the current
/// patch, so the amp's own load of the channel can't overwrite our writes.
/// Evidence: 300 ms works — hardware test `store_patch_round_trip_on_b4`
/// passed twice on 2026-10-31 (store "KTS TEST" with GAIN changed into B4,
/// then the original back; B4 byte-identical at the end). Shorter values untried.
pub const SELECT_SETTLE: std::time::Duration = std::time::Duration::from_millis(300);

/// Current-patch ranges the amp doesn't store from our writes; left out when a
/// stored channel is compared with what was stored into it. SOLO_DELAY: see
/// `WRITE_PROTECTED` (stored channels keep their own unless
/// `SLOT_SOLO_DELAY_WRITABLE`).
pub const NOT_STORED: &[(u32, u32)] = WRITE_PROTECTED;

/// Whether SOLO_DELAY can be written straight into a stored channel. No —
/// evidence: hardware test `slot_solo_delay_direct_write`, 2026-10-31: B4's
/// SOLO_DELAY level (`20 50 3E 07` in slot 9) was 50, a DT1 of 30 read back 50.
pub const SLOT_SOLO_DELAY_WRITABLE: bool = false;

/// Current-patch blocks to re-read while Live edit is on. The amp reports every
/// panel action as a DT1 in editor mode, so this is a safety net: the AMP block.
/// Evidence (stage 2 hardware check, 2026-11-02): `captures/103-*` and
/// `captures/104-*` replay the log over a snapshot taken before it and equal the
/// snapshot taken after it. But in `captures/102-*` a report of the amp type
/// went missing for no known reason (docs/protocol.md, "Panel in editor mode"),
/// and a wrong amp type could be saved or stored without anyone hearing why.
pub const PANEL_BLOCKS: &[(u32, u32)] = &[(addr([0x20, 0x00, 0x06, 0x00]), 10)];

/// Whether `amp::select` can select PANEL (selector 0) in editor mode. Yes —
/// evidence: hardware test `select_panel_and_back`, 2026-10-31 (selector 5 →
/// PANEL → 5, both `Ok`).
pub const PANEL_SELECTABLE: bool = true;

pub fn is_write_protected(linear: u32) -> bool {
    WRITE_PROTECTED.iter().any(|&(a, n)| (a..a + n).contains(&linear))
}

/// Current-patch blocks (base `20 00 00 00`), in read order. Block offsets and
/// sizes come from the official app's address table; each was read back in
/// full from the amp — evidence: captures/02-patch-read-{rq1,dt1}.syx
pub const PATCH_BLOCKS: &[(u32 /* addr */, u32 /* size */)] = &[
    (addr([0x20, 0x00, 0x00, 0x00]), 16),  // COM
    (addr([0x20, 0x00, 0x02, 0x00]), 3),   // OTHER
    (addr([0x20, 0x00, 0x04, 0x00]), 5),   // COLOR
    (addr([0x20, 0x00, 0x06, 0x00]), 10),  // AMP
    (addr([0x20, 0x00, 0x08, 0x00]), 6),   // SW
    (addr([0x20, 0x00, 0x0A, 0x00]), 8),   // BOOSTER(1)
    (addr([0x20, 0x00, 0x0C, 0x00]), 8),   // BOOSTER(2)
    (addr([0x20, 0x00, 0x0E, 0x00]), 8),   // BOOSTER(3)
    (addr([0x20, 0x00, 0x10, 0x00]), 1),   // FX(1)
    (addr([0x20, 0x00, 0x12, 0x00]), 1),   // FX(2)
    (addr([0x20, 0x00, 0x14, 0x00]), 1),   // FX(3)
    (addr([0x20, 0x00, 0x16, 0x00]), 1),   // FX(4)
    (addr([0x20, 0x00, 0x18, 0x00]), 1),   // FX(5)
    (addr([0x20, 0x00, 0x1A, 0x00]), 1),   // FX(6)
    (addr([0x20, 0x00, 0x1C, 0x00]), 225), // FX_DETAIL(1)
    (addr([0x20, 0x00, 0x1E, 0x00]), 225), // FX_DETAIL(2)
    (addr([0x20, 0x00, 0x20, 0x00]), 225), // FX_DETAIL(3)
    (addr([0x20, 0x00, 0x22, 0x00]), 225), // FX_DETAIL(4)
    (addr([0x20, 0x00, 0x24, 0x00]), 225), // FX_DETAIL(5)
    (addr([0x20, 0x00, 0x26, 0x00]), 225), // FX_DETAIL(6)
    (addr([0x20, 0x00, 0x28, 0x00]), 17),  // DELAY(1)
    (addr([0x20, 0x00, 0x2A, 0x00]), 17),  // DELAY(2)
    (addr([0x20, 0x00, 0x2C, 0x00]), 17),  // DELAY(3)
    (addr([0x20, 0x00, 0x2E, 0x00]), 17),  // DELAY(4)
    (addr([0x20, 0x00, 0x30, 0x00]), 17),  // DELAY(5)
    (addr([0x20, 0x00, 0x32, 0x00]), 17),  // DELAY(6)
    (addr([0x20, 0x00, 0x34, 0x00]), 13),  // REVERB(1)
    (addr([0x20, 0x00, 0x36, 0x00]), 13),  // REVERB(2)
    (addr([0x20, 0x00, 0x38, 0x00]), 13),  // REVERB(3)
    (addr([0x20, 0x00, 0x3A, 0x00]), 2),   // SOLO_COM
    (addr([0x20, 0x00, 0x3C, 0x00]), 10),  // SOLO_EQ
    (addr([0x20, 0x00, 0x3E, 0x00]), 14),  // SOLO_DELAY
    (addr([0x20, 0x00, 0x40, 0x00]), 1),   // CONTOUR_COM
    (addr([0x20, 0x00, 0x42, 0x00]), 2),   // CONTOUR(1)
    (addr([0x20, 0x00, 0x44, 0x00]), 2),   // CONTOUR(2)
    (addr([0x20, 0x00, 0x46, 0x00]), 2),   // CONTOUR(3)
    (addr([0x20, 0x00, 0x48, 0x00]), 3),   // PEDALFX_COM
    (addr([0x20, 0x00, 0x4A, 0x00]), 15),  // PEDALFX
    (addr([0x20, 0x00, 0x4C, 0x00]), 3),   // EQ_EACH(1)
    (addr([0x20, 0x00, 0x4E, 0x00]), 3),   // EQ_EACH(2)
    (addr([0x20, 0x00, 0x50, 0x00]), 11),  // EQ_PEQ(1)
    (addr([0x20, 0x00, 0x52, 0x00]), 11),  // EQ_PEQ(2)
    (addr([0x20, 0x00, 0x54, 0x00]), 11),  // EQ_GE10(1)
    (addr([0x20, 0x00, 0x56, 0x00]), 11),  // EQ_GE10(2)
    (addr([0x20, 0x00, 0x58, 0x00]), 3),   // NS
    (addr([0x20, 0x00, 0x5A, 0x00]), 5),   // SENDRETURN
    (addr([0x20, 0x00, 0x5C, 0x00]), 34),  // ASSIGN_KNOBS
    (addr([0x20, 0x00, 0x5E, 0x00]), 1),   // ASSIGN_EXPPDL_FUNC
    (addr([0x20, 0x00, 0x60, 0x00]), 34),  // ASSIGN_EXPPDL_DETAIL
    (addr([0x20, 0x00, 0x62, 0x00]), 49),  // ASSIGN_EXPPDL_MIN
    (addr([0x20, 0x00, 0x64, 0x00]), 49),  // ASSIGN_EXPPDL_MAX
    (addr([0x20, 0x00, 0x66, 0x00]), 2),   // ASSIGN_FS
    (addr([0x20, 0x00, 0x68, 0x00]), 3),   // ASSIGN_GAFCFS(1)
    (addr([0x20, 0x00, 0x6A, 0x00]), 3),   // ASSIGN_GAFCFS(2)
    (addr([0x20, 0x00, 0x6C, 0x00]), 1),   // ASSIGN_GAFCEXPPDL1_FUNC(1)
    (addr([0x20, 0x00, 0x6E, 0x00]), 1),   // ASSIGN_GAFCEXPPDL1_FUNC(2)
    (addr([0x20, 0x00, 0x70, 0x00]), 34),  // ASSIGN_GAFCEXPPDL1_DETAIL(1)
    (addr([0x20, 0x00, 0x72, 0x00]), 34),  // ASSIGN_GAFCEXPPDL1_DETAIL(2)
    (addr([0x20, 0x00, 0x74, 0x00]), 49),  // ASSIGN_GAFCEXPPDL1_MIN(1)
    (addr([0x20, 0x00, 0x76, 0x00]), 49),  // ASSIGN_GAFCEXPPDL1_MIN(2)
    (addr([0x20, 0x00, 0x78, 0x00]), 49),  // ASSIGN_GAFCEXPPDL1_MAX(1)
    (addr([0x20, 0x00, 0x7A, 0x00]), 49),  // ASSIGN_GAFCEXPPDL1_MAX(2)
    (addr([0x20, 0x00, 0x7C, 0x00]), 1),   // ASSIGN_GAFCEXPPDL2_FUNC(1)
    (addr([0x20, 0x00, 0x7E, 0x00]), 1),   // ASSIGN_GAFCEXPPDL2_FUNC(2)
    (addr([0x20, 0x01, 0x00, 0x00]), 34),  // ASSIGN_GAFCEXPPDL2_DETAIL(1)
    (addr([0x20, 0x01, 0x02, 0x00]), 34),  // ASSIGN_GAFCEXPPDL2_DETAIL(2)
    (addr([0x20, 0x01, 0x04, 0x00]), 49),  // ASSIGN_GAFCEXPPDL2_MIN(1)
    (addr([0x20, 0x01, 0x06, 0x00]), 49),  // ASSIGN_GAFCEXPPDL2_MIN(2)
    (addr([0x20, 0x01, 0x08, 0x00]), 49),  // ASSIGN_GAFCEXPPDL2_MAX(1)
    (addr([0x20, 0x01, 0x0A, 0x00]), 49),  // ASSIGN_GAFCEXPPDL2_MAX(2)
    (addr([0x20, 0x01, 0x0C, 0x00]), 1),   // ASSIGN_GAFCEXPPDL3_FUNC(1)
    (addr([0x20, 0x01, 0x0E, 0x00]), 1),   // ASSIGN_GAFCEXPPDL3_FUNC(2)
    (addr([0x20, 0x01, 0x10, 0x00]), 34),  // ASSIGN_GAFCEXPPDL3_DETAIL(1)
    (addr([0x20, 0x01, 0x12, 0x00]), 34),  // ASSIGN_GAFCEXPPDL3_DETAIL(2)
    (addr([0x20, 0x01, 0x14, 0x00]), 49),  // ASSIGN_GAFCEXPPDL3_MIN(1)
    (addr([0x20, 0x01, 0x16, 0x00]), 49),  // ASSIGN_GAFCEXPPDL3_MIN(2)
    (addr([0x20, 0x01, 0x18, 0x00]), 49),  // ASSIGN_GAFCEXPPDL3_MAX(1)
    (addr([0x20, 0x01, 0x1A, 0x00]), 49),  // ASSIGN_GAFCEXPPDL3_MAX(2)
    (addr([0x20, 0x01, 0x1C, 0x00]), 5),   // PATCH_KNOB_READONLY
    (addr([0x20, 0x01, 0x1E, 0x00]), 1),   // PATCH_KNOB_SOLO_DELAY_READONLY
];

/// The official app's name for each block of `PATCH_BLOCKS`, same order. Tone
/// Exchange `.tsl` files key blocks as `PATCH%<name>`.
pub const PATCH_BLOCK_NAMES: [&str; 80] = [
    "COM",
    "OTHER",
    "COLOR",
    "AMP",
    "SW",
    "BOOSTER(1)",
    "BOOSTER(2)",
    "BOOSTER(3)",
    "FX(1)",
    "FX(2)",
    "FX(3)",
    "FX(4)",
    "FX(5)",
    "FX(6)",
    "FX_DETAIL(1)",
    "FX_DETAIL(2)",
    "FX_DETAIL(3)",
    "FX_DETAIL(4)",
    "FX_DETAIL(5)",
    "FX_DETAIL(6)",
    "DELAY(1)",
    "DELAY(2)",
    "DELAY(3)",
    "DELAY(4)",
    "DELAY(5)",
    "DELAY(6)",
    "REVERB(1)",
    "REVERB(2)",
    "REVERB(3)",
    "SOLO_COM",
    "SOLO_EQ",
    "SOLO_DELAY",
    "CONTOUR_COM",
    "CONTOUR(1)",
    "CONTOUR(2)",
    "CONTOUR(3)",
    "PEDALFX_COM",
    "PEDALFX",
    "EQ_EACH(1)",
    "EQ_EACH(2)",
    "EQ_PEQ(1)",
    "EQ_PEQ(2)",
    "EQ_GE10(1)",
    "EQ_GE10(2)",
    "NS",
    "SENDRETURN",
    "ASSIGN_KNOBS",
    "ASSIGN_EXPPDL_FUNC",
    "ASSIGN_EXPPDL_DETAIL",
    "ASSIGN_EXPPDL_MIN",
    "ASSIGN_EXPPDL_MAX",
    "ASSIGN_FS",
    "ASSIGN_GAFCFS(1)",
    "ASSIGN_GAFCFS(2)",
    "ASSIGN_GAFCEXPPDL1_FUNC(1)",
    "ASSIGN_GAFCEXPPDL1_FUNC(2)",
    "ASSIGN_GAFCEXPPDL1_DETAIL(1)",
    "ASSIGN_GAFCEXPPDL1_DETAIL(2)",
    "ASSIGN_GAFCEXPPDL1_MIN(1)",
    "ASSIGN_GAFCEXPPDL1_MIN(2)",
    "ASSIGN_GAFCEXPPDL1_MAX(1)",
    "ASSIGN_GAFCEXPPDL1_MAX(2)",
    "ASSIGN_GAFCEXPPDL2_FUNC(1)",
    "ASSIGN_GAFCEXPPDL2_FUNC(2)",
    "ASSIGN_GAFCEXPPDL2_DETAIL(1)",
    "ASSIGN_GAFCEXPPDL2_DETAIL(2)",
    "ASSIGN_GAFCEXPPDL2_MIN(1)",
    "ASSIGN_GAFCEXPPDL2_MIN(2)",
    "ASSIGN_GAFCEXPPDL2_MAX(1)",
    "ASSIGN_GAFCEXPPDL2_MAX(2)",
    "ASSIGN_GAFCEXPPDL3_FUNC(1)",
    "ASSIGN_GAFCEXPPDL3_FUNC(2)",
    "ASSIGN_GAFCEXPPDL3_DETAIL(1)",
    "ASSIGN_GAFCEXPPDL3_DETAIL(2)",
    "ASSIGN_GAFCEXPPDL3_MIN(1)",
    "ASSIGN_GAFCEXPPDL3_MIN(2)",
    "ASSIGN_GAFCEXPPDL3_MAX(1)",
    "ASSIGN_GAFCEXPPDL3_MAX(2)",
    "PATCH_KNOB_READONLY",
    "PATCH_KNOB_SOLO_DELAY_READONLY",
];

/// `PATCH_BLOCK_NAMES` indexes in the order real Tone Exchange files list
/// their blocks: the two `*_READONLY` blocks right after COLOR, the rest as
/// read. Evidence: all 21 patches in samples/*.tsl share this order.
pub const TSL_BLOCK_ORDER: [usize; 80] = {
    let mut order = [0; 80];
    let mut i = 0;
    while i < 80 {
        order[i] = match i {
            0..=2 => i,
            3 => 78,
            4 => 79,
            _ => i - 2,
        };
        i += 1;
    }
    order
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_names_line_up_with_blocks() {
        assert_eq!(PATCH_BLOCK_NAMES.len(), PATCH_BLOCKS.len());
        assert_eq!(PATCH_BLOCK_NAMES[0], "COM");
        assert_eq!(PATCH_BLOCK_NAMES[3], "AMP");
        assert_eq!(PATCH_BLOCK_NAMES[79], "PATCH_KNOB_SOLO_DELAY_READONLY");
        let mut sorted = PATCH_BLOCK_NAMES.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 80, "names are unique");
    }

    #[test]
    fn panel_blocks_are_patch_blocks() {
        for b in PANEL_BLOCKS {
            assert!(PATCH_BLOCKS.contains(b), "{b:?}");
        }
    }
}
