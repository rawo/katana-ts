//! Helpers shared by the library tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use katana_proto::device::{PATCH_BLOCKS, PATCH_BLOCK_NAMES};
use katana_proto::{find_param, Address, Patch};
use serde_json::{json, Map, Value};

/// A fresh, empty directory for one test.
pub fn tmp(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("katana-gui-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The default preset named `name`, with gain `gain`.
pub fn patch(name: &str, gain: u32) -> Patch {
    let mut p = Patch::default_preset();
    p.set_name(name);
    p.set(find_param("gain").unwrap(), gain).unwrap();
    p
}

pub fn gain_of(p: &Patch) -> Option<u32> {
    p.get(find_param("gain").unwrap())
}

pub fn param_set(p: &Patch) -> Map<String, Value> {
    let mut m = Map::new();
    for (&(a, s), n) in PATCH_BLOCKS.iter().zip(PATCH_BLOCK_NAMES) {
        let bytes = p.mem.read(Address(a), s).unwrap();
        m.insert(format!("PATCH%{n}"), bytes.iter().map(|b| format!("{b:02X}")).collect());
    }
    m
}

/// A Tone Exchange `.tsl` text from raw `paramSet`s.
pub fn tsl_doc(set: &str, device: &str, sets: Vec<Map<String, Value>>) -> String {
    let data: Vec<Value> = sets.into_iter().map(|ps| json!({"memo": "", "paramSet": ps})).collect();
    json!({"name": set, "formatRev": "0000", "device": device, "data": [data]}).to_string()
}

/// A KATANA Gen3 `.tsl` text holding `patches`.
pub fn tsl(set: &str, patches: &[Patch]) -> String {
    tsl_doc(set, "KATANA Gen3", patches.iter().map(param_set).collect())
}

/// File names in `dir`, sorted; empty if it doesn't exist.
pub fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}
