//! Parses the real Tone Exchange downloads in `samples/` (git-ignored); passes
//! trivially when there are none.

#[test]
fn real_samples_parse_completely() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "tsl")) {
        let set = katana_proto::tsl::ToneSet::parse(&std::fs::read(e.path()).unwrap()).unwrap();
        assert!(!set.patches.is_empty());
        for p in &set.patches {
            assert!(p.patch.is_ok(), "{}: {}: {:?}", e.path().display(), p.name, p.patch.as_ref().err());
        }
    }
}

#[test]
fn real_samples_survive_write() {
    use katana_proto::tsl::{write_tsl, ToneSet};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../samples");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for e in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "tsl")) {
        let set = ToneSet::parse(&std::fs::read(e.path()).unwrap()).unwrap();
        for p in set.patches.iter().filter_map(|p| p.patch.as_ref().ok()) {
            let back = ToneSet::parse(&write_tsl(p)).unwrap();
            assert_eq!(back.patches.len(), 1);
            assert_eq!(back.patches[0].patch.as_ref().unwrap(), p, "{}: {}", e.path().display(), p.name());
        }
    }
}
