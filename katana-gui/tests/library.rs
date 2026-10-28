mod common;

use std::time::{Duration, SystemTime};

use common::*;
use katana_gui::library::{free_path, matches, safe_stem, sort_entries, Entry, Kind, Library, Sort};

#[test]
fn open_creates_folders_and_lists_both_kinds() {
    let root = tmp("lib-open");
    for d in ["patches", "quick-saves", "sets"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("patches/A.syx"), patch("A", 1).to_syx()).unwrap();
    std::fs::write(root.join("quick-saves/quick-save_x.syx"), patch("Q", 2).to_syx()).unwrap();
    std::fs::write(root.join("sets/s.tsl"), tsl("Black Sabbath", &[patch("Iron Man", 3), patch("Paranoid", 4)]))
        .unwrap();
    let lib = Library::open(root.clone());
    for d in ["patches", "quick-saves", "sets", "autosave"] {
        assert!(root.join(d).is_dir(), "{d}");
    }
    let mut got: Vec<(String, Kind)> = lib.patches().iter().map(|e| (e.name.clone(), e.kind)).collect();
    got.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(got, [("A".to_string(), Kind::Patch), ("quick-save_x".to_string(), Kind::QuickSave)]);
    assert_eq!(lib.sets().len(), 1);
    assert_eq!(lib.sets()[0].name, "Black Sabbath");
    assert_eq!(lib.sets()[0].patches, ["Iron Man", "Paranoid"]);
    assert!(lib.problems.is_empty(), "{:?}", lib.problems);
}

#[test]
fn broken_files_are_skipped_and_named() {
    let root = tmp("lib-broken");
    std::fs::create_dir_all(root.join("patches")).unwrap();
    std::fs::create_dir_all(root.join("sets")).unwrap();
    std::fs::write(root.join("patches/bad.syx"), b"junk").unwrap();
    std::fs::write(root.join("patches/good.syx"), patch("good", 1).to_syx()).unwrap();
    std::fs::write(root.join("sets/bad.tsl"), b"{").unwrap();
    let lib = Library::open(root);
    assert_eq!(lib.patches().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["good"]);
    assert!(lib.sets().is_empty());
    let all = lib.problems.join("\n");
    assert!(all.contains("bad.syx"), "{all}");
    assert!(all.contains("bad.tsl"), "{all}");
}

#[test]
fn broken_index_resets_read_only_and_says_so() {
    let root = tmp("lib-index");
    std::fs::create_dir_all(root.join("patches")).unwrap();
    std::fs::write(root.join("patches/A.syx"), patch("A", 1).to_syx()).unwrap();
    std::fs::write(root.join("index.json"), "{").unwrap();
    let lib = Library::open(root.clone());
    assert!(!lib.is_read_only(&root.join("patches/A.syx")));
    assert!(lib.problems.iter().any(|p| p.contains("index.json")), "{:?}", lib.problems);
    assert!(root.join("index.json.bad").exists());
    assert_eq!(lib.patches().len(), 1);
}

#[test]
fn read_only_flag_survives_reopen() {
    let root = tmp("lib-ro");
    std::fs::create_dir_all(root.join("patches")).unwrap();
    let a = root.join("patches/A.syx");
    std::fs::write(&a, patch("A", 1).to_syx()).unwrap();
    let mut lib = Library::open(root.clone());
    lib.set_read_only(&a, true).unwrap();
    assert!(lib.patches()[0].read_only);
    let lib = Library::open(root);
    assert!(lib.is_read_only(&a));
    assert!(lib.patches()[0].read_only);
}

#[test]
fn detached_library_lists_nothing_and_refuses_writes() {
    let mut lib = Library::detached();
    assert!(lib.root().is_none());
    assert!(lib.patches().is_empty());
    assert!(lib.set_read_only(std::path::Path::new("x.syx"), true).is_err());
}

#[test]
fn free_path_numbers_case_insensitively() {
    let d = tmp("lib-free");
    assert_eq!(free_path(&d, "Iron Man", "syx"), d.join("Iron Man.syx"));
    std::fs::write(d.join("Iron Man.syx"), b"").unwrap();
    assert_eq!(free_path(&d, "Iron Man", "syx"), d.join("Iron Man (2).syx"));
    std::fs::write(d.join("iron man (2).SYX"), b"").unwrap();
    assert_eq!(free_path(&d, "Iron Man", "syx"), d.join("Iron Man (3).syx"));
    assert_eq!(free_path(&d, "IRON MAN", "syx"), d.join("IRON MAN (3).syx"));
}

#[test]
fn safe_stem_keeps_readable_names() {
    assert_eq!(safe_stem("Iron Man (live)"), "Iron Man (live)");
    assert_eq!(safe_stem("AC/DC: Back?"), "AC_DC_ Back_");
    assert_eq!(safe_stem("   "), "patch");
    assert_eq!(safe_stem("tab\there"), "tab_here");
}

#[test]
fn search_matches_any_word() {
    assert!(matches("zeppelin black", "Black Dog"));
    assert!(matches("", "anything"));
    assert!(matches("  ", "anything"));
    assert!(!matches("metallica", "Black Dog"));
}

fn entry(name: &str, secs: u64) -> Entry {
    Entry {
        kind: Kind::Patch,
        path: format!("{name}.syx").into(),
        name: name.into(),
        modified: SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
        read_only: false,
    }
}

#[test]
fn sort_orders() {
    let names = |v: &[Entry]| v.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
    let mut v = vec![entry("b", 1), entry("A", 3), entry("c", 2)];
    sort_entries(&mut v, Sort::NameAz);
    assert_eq!(names(&v), ["A", "b", "c"]);
    sort_entries(&mut v, Sort::NameZa);
    assert_eq!(names(&v), ["c", "b", "A"]);
    sort_entries(&mut v, Sort::NewestFirst);
    assert_eq!(names(&v), ["A", "c", "b"]);
    sort_entries(&mut v, Sort::OldestFirst);
    assert_eq!(names(&v), ["b", "c", "A"]);
}
