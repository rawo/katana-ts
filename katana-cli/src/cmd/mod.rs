pub mod check_settings;
pub mod diff;
pub mod dump;
pub mod extract;
pub mod identity;
pub mod load;
pub mod monitor;
pub mod params;
pub mod ports;
pub mod set;
pub mod store;
pub mod tsl_list;
pub mod verify;
pub mod watch;

use std::path::Path;

use anyhow::{bail, Context};
use katana_proto::{decode, device, load_tshark_export, read_syx, Command, Direction, Header};

pub fn header(model_id_hex: Option<&str>) -> anyhow::Result<Header> {
    let mut h = device::header();
    if let Some(hex) = model_id_hex {
        h.model_id = parse_hex(hex)?;
    }
    Ok(h)
}

pub fn parse_hex(s: &str) -> anyhow::Result<Vec<u8>> {
    let s: String = s.chars().filter(|c| !c.is_whitespace() && *c != ':').collect();
    if s.len() % 2 != 0 {
        bail!("odd number of hex digits in {s:?}");
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).context("bad hex")).collect()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// One-line description of a decoded RQ1/DT1, if it decodes for `h`.
pub fn describe(h: &Header, msg: &[u8]) -> Option<String> {
    match decode(h, msg).ok()? {
        Command::Rq1 { addr, size } => Some(format!("RQ1 {addr} size {size}")),
        Command::Dt1 { addr, data } => Some(format!("DT1 {addr} [{}]", hex(&data))),
    }
}

/// Loads the SysEx messages of a `.syx` file (no direction) or a capture export.
pub fn load_messages(path: &Path) -> anyhow::Result<Vec<(Option<Direction>, Vec<u8>)>> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("syx")) {
        return Ok(read_syx(&bytes)?.into_iter().map(|m| (None, m)).collect());
    }
    let text = String::from_utf8(bytes).context("capture export is not UTF-8")?;
    Ok(load_tshark_export(&text)?.into_iter().map(|m| (Some(m.dir), m.bytes)).collect())
}
