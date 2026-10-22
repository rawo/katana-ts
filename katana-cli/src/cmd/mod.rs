pub mod diff;
pub mod extract;
pub mod identity;
pub mod monitor;
pub mod ports;

use std::path::Path;

use anyhow::{bail, Context};
use katana_proto::{decode, load_tshark_export, CapturedMessage, Command, Header};

pub fn header(model_id_hex: &str) -> anyhow::Result<Header> {
    Ok(Header { device_id: 0x10, model_id: parse_hex(model_id_hex)?, addr_width: 4 })
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

/// Loads the SysEx messages of a capture export.
pub fn load_messages(path: &Path) -> anyhow::Result<Vec<CapturedMessage>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(load_tshark_export(&text)?)
}
