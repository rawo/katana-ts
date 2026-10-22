//! Parameter names from the official app's `address_map.js`, read from the
//! user's own installation at runtime. Used only to label addresses in
//! discovery output; nothing here is a confirmed protocol fact.

use std::collections::{BTreeMap, HashMap};

use katana_proto::device::addr;

/// Where the official app unpacks its editor under Wine.
pub const DEFAULT_PATH: &str =
    "drive_c/users/{user}/AppData/Local/Roland/BOSS TONE STUDIO for KATANA Gen 3/html/js/config/address_map.js";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub block: String,
    pub param: String,
    /// Index of this byte within the parameter.
    pub byte: u32,
    /// Parameter length in bytes.
    pub len: u32,
    /// Displayed value = raw - ofs.
    pub ofs: i32,
    /// Multi-byte value stored as 4-bit nibbles (INTEGER2x4 / INTEGER4x4).
    pub nibbles: bool,
}

impl Label {
    /// Raw value from the parameter's bytes.
    pub fn decode(&self, bytes: &[u8]) -> u32 {
        let bits = if self.nibbles { 4 } else { 7 };
        bytes.iter().fold(0, |v, &b| (v << bits) | b as u32)
    }
}

struct Leaf {
    addr: u32,
    len: u32,
    ofs: i32,
    nibbles: bool,
    name: String,
}

pub struct AddrMap {
    labels: BTreeMap<u32, Label>,
}

/// Value of `key` in a `{ key:value, ... }` table row, up to the next `,` or `}`.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(&format!("{key}:"))? + key.len() + 1;
    let rest = &line[start..];
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches('\''))
}

/// Table addresses are 4 base-128 digits packed into a hex literal.
fn table_addr(hex: &str) -> Option<u32> {
    let v = u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok()?;
    Some(addr(v.to_be_bytes()))
}

fn byte_len(size: &str) -> u32 {
    match size {
        "INTEGER2x4" | "INTEGER2x7" => 2,
        "INTEGER4x4" => 4,
        s => s.parse().ok().filter(|&n| n > 0).unwrap_or(1),
    }
}

impl AddrMap {
    /// Labels every parameter of the `PATCH` group, placed at `base`.
    pub fn parse(js: &str, base: u32) -> AddrMap {
        let mut tables: HashMap<String, Vec<Leaf>> = HashMap::new();
        let mut patch_blocks: Vec<(u32, String, String)> = Vec::new();
        let mut current: Option<String> = None;
        for line in js.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("var ") {
                current = rest.split_whitespace().next().map(str::to_string);
                continue;
            }
            if t.starts_with("];") {
                current = None;
                continue;
            }
            let Some(table) = current.as_deref() else { continue };
            let Some(a) = field(t, "addr").and_then(table_addr) else { continue };
            let Some(name) = field(t, "name") else { continue };
            match field(t, "child") {
                Some(child) if table == "PATCH" => patch_blocks.push((a, child.to_string(), name.to_string())),
                Some(_) => {}
                None => tables.entry(table.to_string()).or_default().push(Leaf {
                    addr: a,
                    len: field(t, "size").map_or(1, byte_len),
                    nibbles: matches!(field(t, "size"), Some("INTEGER2x4" | "INTEGER4x4")),
                    ofs: field(t, "ofs").and_then(|o| o.parse().ok()).unwrap_or(0),
                    name: name.to_string(),
                }),
            }
        }
        let mut labels = BTreeMap::new();
        for (block_addr, child, block) in &patch_blocks {
            for leaf in tables.get(child).map(Vec::as_slice).unwrap_or(&[]) {
                for byte in 0..leaf.len {
                    labels.insert(
                        base + block_addr + leaf.addr + byte,
                        Label {
                            block: block.clone(),
                            param: leaf.name.clone(),
                            byte,
                            len: leaf.len,
                            ofs: leaf.ofs,
                            nibbles: leaf.nibbles,
                        },
                    );
                }
            }
        }
        AddrMap { labels }
    }

    pub fn label(&self, linear: u32) -> Option<&Label> {
        self.labels.get(&linear)
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
\tvar PATCH_AMP = [\t// PATCH AMP
\t\t{ addr:0x00000000,\tsize:INTEGER1x7,\tofs:   0,\tinit:  50,\tmin:   0,\tmax: 100,\tname:'PRMID_PATCH_AMP_GAIN'\t},
\t\t{ addr:0x00000001,\tsize:INTEGER2x4,\tofs:   0,\tinit:   0,\tmin:   0,\tmax: 200,\tname:'PRMID_PATCH_AMP_WIDE'\t},
\t];
\tvar PATCH_FX = [\t// PATCH FX
\t\t{ addr:0x00000100,\tsize:INTEGER1x7,\tofs:  20,\tinit:   0,\tmin: -20,\tmax:  20,\tname:'PRMID_PATCH_FX_LEVEL'\t},
\t];
\tvar PATCH = [\t// PATCH
\t\t{ addr:0x00000600,\tsize:   0,\tchild:PATCH_AMP,\tname:'AMP'\t},\t// PATCH AMP
\t\t{ addr:0x00007E00,\tsize:   0,\tchild:PATCH_FX,\tname:'FX(1)'\t},\t// PATCH FX
\t\t{ addr:0x00010000,\tsize:   0,\tchild:PATCH_FX,\tname:'FX(2)'\t},\t// PATCH FX
\t];
";

    fn lin(b: [u8; 4]) -> u32 {
        katana_proto::device::addr(b)
    }

    #[test]
    fn labels_single_byte_param() {
        let m = AddrMap::parse(SAMPLE, lin([0x20, 0, 0, 0]));
        let l = m.label(lin([0x20, 0x00, 0x06, 0x00])).unwrap();
        assert_eq!(
            (l.block.as_str(), l.param.as_str(), l.byte, l.len, l.ofs),
            ("AMP", "PRMID_PATCH_AMP_GAIN", 0, 1, 0)
        );
    }

    #[test]
    fn labels_each_byte_of_multibyte_param() {
        let m = AddrMap::parse(SAMPLE, lin([0x20, 0, 0, 0]));
        let l = m.label(lin([0x20, 0x00, 0x06, 0x02])).unwrap();
        assert_eq!((l.param.as_str(), l.byte, l.len), ("PRMID_PATCH_AMP_WIDE", 1, 2));
    }

    #[test]
    fn block_offsets_and_param_offsets_carry_7bit() {
        let m = AddrMap::parse(SAMPLE, lin([0x20, 0, 0, 0]));
        // FX(1) at 00 00 7E 00 + param 00 00 01 00 = 00 00 7F 00
        assert_eq!(m.label(lin([0x20, 0x00, 0x7F, 0x00])).unwrap().block, "FX(1)");
        // FX(2) at 00 01 00 00 + 00 00 01 00
        let l = m.label(lin([0x20, 0x01, 0x01, 0x00])).unwrap();
        assert_eq!((l.block.as_str(), l.ofs), ("FX(2)", 20));
    }

    #[test]
    fn decodes_nibble_value() {
        let m = AddrMap::parse(SAMPLE, lin([0x20, 0, 0, 0]));
        let l = m.label(lin([0x20, 0x00, 0x06, 0x01])).unwrap();
        assert_eq!(l.decode(&[0x0C, 0x08]), 200);
    }

    #[test]
    fn unknown_address_has_no_label() {
        let m = AddrMap::parse(SAMPLE, lin([0x20, 0, 0, 0]));
        assert!(m.label(lin([0x20, 0x00, 0x06, 0x05])).is_none());
    }
}
