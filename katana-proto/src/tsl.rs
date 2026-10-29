//! BOSS Tone Exchange `.tsl` sets for the KATANA Gen 3: JSON holding a list of
//! patches, each a `paramSet` mapping `PATCH%<block>` to its bytes as hex
//! strings. See docs/superpowers/specs/2026-10-28-katana-preset-library-design.md.

use serde_json::{Map, Value};

use crate::device::{PATCH_BLOCKS, PATCH_BLOCK_NAMES, TSL_BLOCK_ORDER};
use crate::{Address, Patch};

/// The only `device` value accepted.
pub const DEVICE: &str = "KATANA Gen3";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TslError {
    #[error("not a Tone Exchange file: {0}")]
    NotJson(String),
    #[error("made for {0}, not {DEVICE}")]
    WrongDevice(String),
    #[error("damaged set: {0}")]
    Malformed(String),
    #[error("unknown block {0}")]
    UnknownBlock(String),
    #[error("block {0} is missing")]
    MissingBlock(&'static str),
    #[error("block {block} has {got} bytes, expected {expected}")]
    BlockSize { block: String, expected: u32, got: usize },
    #[error("block {block} has a bad byte {value:?}")]
    BadByte { block: String, value: String },
}

/// A downloaded set, in file order.
#[derive(Debug, Clone)]
pub struct ToneSet {
    pub name: String,
    pub patches: Vec<SetPatch>,
}

/// One patch of a set. `patch` is an error when the patch can't be mapped
/// completely onto a KATANA Gen 3 patch; the rest of the set is still usable.
#[derive(Debug, Clone)]
pub struct SetPatch {
    pub name: String,
    pub patch: Result<Patch, TslError>,
}

impl ToneSet {
    /// Refuses the whole file only when it isn't a KATANA Gen 3 set at all.
    pub fn parse(bytes: &[u8]) -> Result<ToneSet, TslError> {
        let doc: Value = serde_json::from_slice(bytes).map_err(|e| TslError::NotJson(e.to_string()))?;
        let device = doc.get("device").and_then(Value::as_str).unwrap_or_default();
        if device != DEVICE {
            return Err(TslError::WrongDevice(device.to_string()));
        }
        let name = doc.get("name").and_then(Value::as_str).unwrap_or_default().trim().to_string();
        let list = doc
            .get("data")
            .and_then(|d| d.get(0))
            .and_then(Value::as_array)
            .filter(|l| !l.is_empty())
            .ok_or_else(|| TslError::Malformed("no patches".into()))?;
        let patches = list
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let patch = entry
                    .get("paramSet")
                    .and_then(Value::as_object)
                    .ok_or_else(|| TslError::Malformed(format!("patch {} has no paramSet", i + 1)))
                    .and_then(to_patch);
                let name = match &patch {
                    Ok(p) => p.name(),
                    Err(_) => entry.get("paramSet").and_then(Value::as_object).and_then(com_name).unwrap_or_default(),
                };
                let name = if name.is_empty() { format!("Patch {}", i + 1) } else { name };
                SetPatch { name, patch }
            })
            .collect();
        Ok(ToneSet { name, patches })
    }
}

/// A one-patch set holding `p`, named after it, shaped like Tone Exchange
/// downloads (key order, upper-case hex bytes).
pub fn write_tsl(p: &Patch) -> Vec<u8> {
    let mut params = Map::new();
    for &i in &TSL_BLOCK_ORDER {
        let (a, s) = PATCH_BLOCKS[i];
        let bytes = p.mem.read(Address(a), s).expect("a Patch holds every block");
        params.insert(format!("PATCH%{}", PATCH_BLOCK_NAMES[i]), bytes.iter().map(|b| format!("{b:02X}")).collect());
    }
    let mut doc = Map::new();
    doc.insert("name".into(), p.name().into());
    doc.insert("formatRev".into(), "0000".into());
    doc.insert("device".into(), DEVICE.into());
    let mut entry = Map::new();
    entry.insert("memo".into(), "".into());
    entry.insert("paramSet".into(), Value::Object(params));
    doc.insert("data".into(), Value::Array(vec![Value::Array(vec![Value::Object(entry)])]));
    serde_json::to_vec(&Value::Object(doc)).expect("JSON of strings")
}

fn to_patch(set: &Map<String, Value>) -> Result<Patch, TslError> {
    if let Some(k) = set.keys().find(|k| key_index(k).is_none()) {
        return Err(TslError::UnknownBlock(k.clone()));
    }
    let mut p = Patch::default();
    for (&(a, s), &n) in PATCH_BLOCKS.iter().zip(PATCH_BLOCK_NAMES.iter()) {
        let v = set.get(&format!("PATCH%{n}")).ok_or(TslError::MissingBlock(n))?;
        p.mem.apply(Address(a), &block_bytes(n, s, v)?);
    }
    Ok(p)
}

fn key_index(key: &str) -> Option<usize> {
    let n = key.strip_prefix("PATCH%")?;
    PATCH_BLOCK_NAMES.iter().position(|&b| b == n)
}

fn block_bytes(name: &str, size: u32, v: &Value) -> Result<Vec<u8>, TslError> {
    let items = v.as_array().ok_or_else(|| TslError::Malformed(format!("block {name} is not a list")))?;
    let mut bytes = items
        .iter()
        .map(|x| {
            let bad =
                || TslError::BadByte { block: name.into(), value: x.as_str().map_or(x.to_string(), String::from) };
            let s = x.as_str().ok_or_else(bad)?;
            match u8::from_str_radix(s, 16) {
                Ok(b) if b <= 0x7F && !s.is_empty() && s.len() <= 2 => Ok(b),
                _ => Err(bad()),
            }
        })
        .collect::<Result<Vec<u8>, TslError>>()?;
    // Some Tone Exchange patches carry a longer block (a 6-byte COLOR, seen in
    // samples/). The amp has no room for the extra bytes, so zeros there are
    // dropped; anything else would be lost and is refused.
    if bytes.len() < size as usize || bytes[size as usize..].iter().any(|&b| b != 0) {
        return Err(TslError::BlockSize { block: name.into(), expected: size, got: bytes.len() });
    }
    bytes.truncate(size as usize);
    Ok(bytes)
}

/// The name from `PATCH%COM` alone, for a patch that failed elsewhere.
fn com_name(set: &Map<String, Value>) -> Option<String> {
    let (a, s) = PATCH_BLOCKS[0];
    let bytes = block_bytes(PATCH_BLOCK_NAMES[0], s, set.get("PATCH%COM")?).ok()?;
    let mut p = Patch::default();
    p.mem.apply(Address(a), &bytes);
    Some(p.name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{PATCH_BLOCKS, PATCH_BLOCK_NAMES};
    use crate::{Address, Patch};
    use serde_json::{json, Map, Value};

    fn param_set(p: &Patch) -> Map<String, Value> {
        let mut m = Map::new();
        for (&(a, s), n) in PATCH_BLOCKS.iter().zip(PATCH_BLOCK_NAMES) {
            let bytes = p.mem.read(Address(a), s).unwrap();
            m.insert(format!("PATCH%{n}"), bytes.iter().map(|b| format!("{b:02X}")).collect());
        }
        m
    }

    fn doc(set: &str, device: &str, sets: Vec<Map<String, Value>>) -> String {
        let data: Vec<Value> = sets.into_iter().map(|ps| json!({"memo": "", "paramSet": ps})).collect();
        json!({"name": set, "formatRev": "0000", "device": device, "data": [data]}).to_string()
    }

    /// A .tsl text holding `patches`, built from their bytes and PATCH_BLOCK_NAMES.
    fn tsl(set: &str, device: &str, patches: &[Patch]) -> String {
        doc(set, device, patches.iter().map(param_set).collect())
    }

    #[test]
    fn parses_set_and_patches() {
        let mut a = Patch::default_preset();
        a.set_name("Iron Man");
        a.set(crate::find_param("gain").unwrap(), 70).unwrap();
        let mut b = Patch::default_preset();
        b.set_name("Paranoid");
        let set = ToneSet::parse(tsl("Black Sabbath", "KATANA Gen3", &[a.clone(), b]).as_bytes()).unwrap();
        assert_eq!(set.name, "Black Sabbath");
        assert_eq!(set.patches.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Iron Man", "Paranoid"]);
        assert_eq!(set.patches[0].patch.as_ref().unwrap(), &a);
    }

    #[test]
    fn lowercase_hex_and_memo_object_are_fine() {
        let mut a = Patch::default_preset();
        a.set_name("zz top"); // 0x7a: hex with letters
        let ps: Map<String, Value> = param_set(&a)
            .into_iter()
            .map(|(k, v)| {
                let low = v.as_array().unwrap().iter().map(|b| Value::from(b.as_str().unwrap().to_lowercase()));
                (k, Value::Array(low.collect()))
            })
            .collect();
        let text = json!({"name": "S", "formatRev": "0000", "device": "KATANA Gen3",
            "data": [[{"memo": {"memo": "", "isToneCentralPatch": false}, "paramSet": ps}]]})
        .to_string();
        let set = ToneSet::parse(text.as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch.as_ref().unwrap(), &a);
    }

    #[test]
    fn wrong_device_is_refused() {
        let e = ToneSet::parse(tsl("X", "KATANA MkII", &[Patch::default_preset()]).as_bytes()).unwrap_err();
        assert_eq!(e, TslError::WrongDevice("KATANA MkII".into()));
    }

    #[test]
    fn one_bad_patch_keeps_the_rest() {
        let mut a = Patch::default_preset();
        a.set_name("Good");
        let mut b = Patch::default_preset();
        b.set_name("Bad");
        let mut bad = param_set(&b);
        bad.remove("PATCH%AMP");
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![param_set(&a), bad]).as_bytes()).unwrap();
        assert_eq!(set.patches.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Good", "Bad"]);
        assert!(set.patches[0].patch.is_ok());
        assert_eq!(set.patches[1].patch, Err(TslError::MissingBlock("AMP")));
    }

    #[test]
    fn wrong_block_size_and_byte_are_refused() {
        let p = Patch::default_preset();
        let mut short = param_set(&p);
        short["PATCH%AMP"].as_array_mut().unwrap().pop();
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![short]).as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch, Err(TslError::BlockSize { block: "AMP".into(), expected: 10, got: 9 }));

        let mut high = param_set(&p);
        high["PATCH%AMP"][0] = Value::from("80");
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![high]).as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch, Err(TslError::BadByte { block: "AMP".into(), value: "80".into() }));
    }

    #[test]
    fn longer_block_with_zero_padding_is_trimmed() {
        let p = Patch::default_preset();
        let mut ps = param_set(&p);
        ps["PATCH%COLOR"].as_array_mut().unwrap().push(Value::from("00"));
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![ps.clone()]).as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch.as_ref().unwrap(), &p);

        *ps["PATCH%COLOR"].as_array_mut().unwrap().last_mut().unwrap() = Value::from("01");
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![ps]).as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch, Err(TslError::BlockSize { block: "COLOR".into(), expected: 5, got: 6 }));
    }

    #[test]
    fn unknown_block_is_refused() {
        let mut ps = param_set(&Patch::default_preset());
        ps.insert("PATCH%NEW_THING".into(), Value::Array(vec![]));
        let set = ToneSet::parse(doc("S", "KATANA Gen3", vec![ps]).as_bytes()).unwrap();
        assert_eq!(set.patches[0].patch, Err(TslError::UnknownBlock("PATCH%NEW_THING".into())));
    }

    #[test]
    fn write_tsl_round_trips() {
        let mut p = Patch::default_preset();
        p.set_name("Iron Man");
        p.set(crate::find_param("gain").unwrap(), 70).unwrap();
        let set = ToneSet::parse(&write_tsl(&p)).unwrap();
        assert_eq!(set.name, "Iron Man");
        assert_eq!(set.patches.len(), 1);
        assert_eq!(set.patches[0].patch.as_ref().unwrap(), &p);
    }

    #[test]
    fn write_tsl_matches_real_key_order() {
        let doc: Value = serde_json::from_slice(&write_tsl(&Patch::default_preset())).unwrap();
        let top: Vec<&String> = doc.as_object().unwrap().keys().collect();
        assert_eq!(top, ["name", "formatRev", "device", "data"]);
        let keys =
            |d: &Value| -> Vec<String> { d["data"][0][0]["paramSet"].as_object().unwrap().keys().cloned().collect() };
        let written = keys(&doc);
        assert_eq!(written.len(), 80);
        assert_eq!(written[3], "PATCH%PATCH_KNOB_READONLY");
        let real = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples/Black_Sabbath.tsl");
        if let Ok(bytes) = std::fs::read(real) {
            assert_eq!(written, keys(&serde_json::from_slice(&bytes).unwrap()));
        }
    }

    #[test]
    fn not_json_is_refused() {
        assert!(matches!(ToneSet::parse(b"junk"), Err(TslError::NotJson(_))));
    }

    #[test]
    fn empty_set_is_malformed() {
        let text = json!({"name": "S", "device": "KATANA Gen3", "data": [[]]}).to_string();
        assert!(matches!(ToneSet::parse(text.as_bytes()), Err(TslError::Malformed(_))));
    }
}
