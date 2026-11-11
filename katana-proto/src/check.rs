//! Proves settings on the amp: writes each one and reads it back, then
//! restores the patch as it was. Nothing is stored into a channel.

use std::time::Duration;

use crate::device::{self, PATCH_BLOCKS, SEND_INTERVAL};
use crate::patch::mismatch_skipping;
use crate::{read_blocks, read_patch, set_param, write_patch, ParamDef, ProtoError, Transport};

/// One setting's check: the value written and what the amp read back.
#[derive(Debug)]
pub struct CheckResult {
    pub name: &'static str,
    pub wrote: u32,
    pub read: Option<u32>,
}

impl CheckResult {
    pub fn passed(&self) -> bool {
        self.read == Some(self.wrote)
    }
}

/// Writes a value different from the current one into each setting (its
/// minimum, or its maximum when it already is at the minimum), reads each
/// block back after its writes, then writes the original patch back and
/// checks it. Write-protected settings are skipped.
pub fn check_settings(
    t: &mut dyn Transport,
    defs: &[&'static ParamDef],
    timeout: Duration,
) -> Result<Vec<CheckResult>, ProtoError> {
    let original = read_patch(t, timeout)?;
    let mut results = Vec::new();
    for &block in PATCH_BLOCKS {
        let (start, end) = (block.0, block.0 + block.1);
        let mut wrote = Vec::new();
        for &d in defs.iter().filter(|d| (start..end).contains(&d.addr) && !device::is_write_protected(d.addr)) {
            if d.min == d.max {
                continue;
            }
            let v = if original.get(d) == Some(d.min) { d.max } else { d.min };
            set_param(t, d, v)?;
            std::thread::sleep(SEND_INTERVAL);
            wrote.push((d, v));
        }
        if wrote.is_empty() {
            continue;
        }
        let got = read_blocks(t, &[block], timeout)?;
        results.extend(wrote.into_iter().map(|(d, v)| CheckResult { name: d.name, wrote: v, read: got.get(d) }));
    }
    write_patch(t, &original)?;
    let back = read_patch(t, timeout)?;
    if let Some((addr, expected, got)) = mismatch_skipping(&original, &back, device::WRITE_PROTECTED) {
        return Err(ProtoError::WriteBackMismatch { addr, expected, got });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::device::{self, addr};
    use crate::{decode, settings, Command, FakeAmp, ParamDef, Patch};

    const T: Duration = Duration::from_millis(200);

    fn amp() -> FakeAmp {
        let mut amp = FakeAmp::new(device::header(), 255);
        amp.mem = Patch::default_preset().mem;
        amp
    }

    fn defs(prefix: &str) -> Vec<&'static ParamDef> {
        crate::all_params().iter().filter(|d| d.name.starts_with(prefix)).collect()
    }

    #[test]
    fn every_setting_reads_back_on_a_willing_amp() {
        let mut amp = amp();
        let before = amp.mem.clone();
        let r = check_settings(&mut amp, &defs("reverb1-"), T).unwrap();
        assert!(r.len() > 5, "{r:?}");
        assert!(r.iter().all(CheckResult::passed), "{r:?}");
        assert_eq!(amp.mem, before, "original restored");
    }

    #[test]
    fn a_setting_the_amp_ignores_fails() {
        let mut amp = amp();
        let density = settings::setting("REVERB", 0, "density").unwrap();
        amp.read_only = vec![(density.addr, 1)];
        let r = check_settings(&mut amp, &defs("reverb1-"), T).unwrap();
        let bad: Vec<_> = r.iter().filter(|r| !r.passed()).map(|r| r.name).collect();
        assert_eq!(bad, ["reverb1-density"]);
    }

    #[test]
    fn write_protected_settings_are_skipped() {
        let mut amp = amp();
        let r = check_settings(&mut amp, &defs("solodelay-"), T).unwrap();
        assert!(r.is_empty(), "{r:?}");
        let solo_delay = addr([0x20, 0x00, 0x3E, 0x00])..addr([0x20, 0x00, 0x3E, 0x0E]);
        for msg in &amp.sent {
            if let Ok(Command::Dt1 { addr: a, .. }) = decode(&device::header(), msg) {
                assert!(!solo_delay.contains(&a.0), "wrote {a:?}");
            }
        }
    }

    #[test]
    fn nothing_but_the_current_patch_is_written() {
        let mut amp = amp();
        check_settings(&mut amp, &defs("delay4-"), T).unwrap();
        for msg in &amp.sent {
            if let Ok(Command::Dt1 { addr: a, .. }) = decode(&device::header(), msg) {
                assert!(device::PATCH_BLOCKS.iter().any(|&(b, n)| (b..b + n).contains(&a.0)), "{a:?}");
            }
        }
    }
}
