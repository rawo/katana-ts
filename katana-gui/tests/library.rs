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

// Task 4: actions.

fn lib_with_set(
    test: &str,
    set: &str,
    patches: &[katana_proto::Patch],
) -> (std::path::PathBuf, Library, std::path::PathBuf) {
    let root = tmp(test);
    let mut lib = Library::open(root.join("lib"));
    let src = root.join(format!("{set}.tsl"));
    std::fs::write(&src, tsl(set, patches)).unwrap();
    let set_path = lib.import(&src).unwrap();
    (root, lib, set_path)
}

#[test]
fn import_syx_and_tsl_land_in_their_lists() {
    let root = tmp("lib-import");
    let mut lib = Library::open(root.join("lib"));
    let a = root.join("a.syx");
    std::fs::write(&a, patch("A", 5).to_syx()).unwrap();
    let got = lib.import(&a).unwrap();
    assert_eq!(got, root.join("lib/patches/a.syx"));
    assert_eq!(lib.kind_of(&got), Some(Kind::Patch));
    let s = root.join("s.tsl");
    let text = tsl("Black Sabbath", &[patch("Iron Man", 3)]);
    std::fs::write(&s, &text).unwrap();
    let set = lib.import(&s).unwrap();
    assert_eq!(std::fs::read(&set).unwrap(), text.as_bytes());
    assert_eq!(lib.sets()[0].name, "Black Sabbath");
    assert_eq!(lib.import(&a).unwrap(), root.join("lib/patches/a (2).syx"));
    let mut names: Vec<_> = lib.patches().iter().map(|e| e.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["a", "a (2)"]);
}

#[test]
fn import_refuses_wrong_device_and_other_files() {
    let root = tmp("lib-import-bad");
    let mut lib = Library::open(root.join("lib"));
    let mk2 = root.join("mk2.tsl");
    std::fs::write(&mk2, tsl_doc("X", "KATANA MkII", vec![param_set(&patch("A", 1))])).unwrap();
    let e = lib.import(&mk2).unwrap_err();
    assert!(e.contains("KATANA Gen3"), "{e}");
    let txt = root.join("x.txt");
    std::fs::write(&txt, b"hello").unwrap();
    assert!(lib.import(&txt).unwrap_err().contains("not a .syx or .tsl"));
    let junk = root.join("junk.syx");
    std::fs::write(&junk, b"junk").unwrap();
    assert!(lib.import(&junk).is_err());
    assert!(names_in(&root.join("lib/sets")).is_empty());
    assert!(names_in(&root.join("lib/patches")).is_empty());
}

#[test]
fn pick_creates_numbered_patch_and_records_source() {
    let iron = patch("Iron Man", 33);
    let (root, mut lib, set) = lib_with_set("lib-pick", "Black Sabbath", std::slice::from_ref(&iron));
    let before = std::fs::read(&set).unwrap();
    let (p1, got) = lib.pick(&set, 0).unwrap();
    assert_eq!(got, iron);
    assert_eq!(p1, root.join("lib/patches/Iron Man.syx"));
    let (p2, _) = lib.pick(&set, 0).unwrap();
    assert_eq!(p2, root.join("lib/patches/Iron Man (2).syx"));
    assert_eq!(lib.load(&p2).unwrap(), iron);
    assert_eq!(std::fs::read(&set).unwrap(), before);
    let index = std::fs::read_to_string(root.join("lib/index.json")).unwrap();
    assert!(index.contains("Black Sabbath") && index.contains("Iron Man"), "{index}");
}

#[test]
fn pick_bad_patch_is_refused_without_a_file() {
    let root = tmp("lib-pick-bad");
    let mut lib = Library::open(root.join("lib"));
    let mut bad = param_set(&patch("Broken", 1));
    bad.remove("PATCH%AMP");
    let src = root.join("s.tsl");
    std::fs::write(&src, tsl_doc("Set", "KATANA Gen3", vec![param_set(&patch("Fine", 1)), bad])).unwrap();
    let set = lib.import(&src).unwrap();
    let e = lib.pick(&set, 1).unwrap_err();
    assert!(e.contains("Broken") && e.contains("Set"), "{e}");
    assert!(names_in(&root.join("lib/patches")).is_empty());
    assert!(lib.pick(&set, 5).is_err());
}

#[test]
fn save_as_names_file_and_patch() {
    let root = tmp("lib-save-as");
    let mut lib = Library::open(root.join("lib"));
    let p = patch("DEFAULT", 12);
    let a = lib.save_as("Blues Lead", &p).unwrap();
    assert_eq!(a, root.join("lib/patches/Blues Lead.syx"));
    let saved = lib.load(&a).unwrap();
    assert_eq!(saved.name(), "Blues Lead");
    assert_eq!(gain_of(&saved), Some(12));
    let b = lib.save_as("blues lead", &p).unwrap();
    assert_eq!(b, root.join("lib/patches/blues lead (2).syx"));
    let long = lib.save_as("A very long patch name", &p).unwrap();
    assert_eq!(lib.load(&long).unwrap().name(), "A very long patc");
    assert!(long.ends_with("A very long patch name.syx"));
}

#[test]
fn quick_saves_in_the_same_second_are_distinct() {
    let root = tmp("lib-quick");
    let mut lib = Library::open(root.join("lib"));
    let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 28).unwrap().and_hms_opt(15, 4, 12).unwrap();
    let a = lib.new_quick_save(&patch("X", 1), now).unwrap();
    let b = lib.new_quick_save(&patch("X", 2), now).unwrap();
    assert_eq!(a, root.join("lib/quick-saves/quick-save_2026-10-28_15-04-12.syx"));
    assert_eq!(b, root.join("lib/quick-saves/quick-save_2026-10-28_15-04-12 (2).syx"));
    assert!(lib.patches().iter().all(|e| e.kind == Kind::QuickSave));
    assert_eq!(lib.patches().len(), 2);
    assert_eq!(gain_of(&lib.load(&b).unwrap()), Some(2));
}

#[test]
fn read_only_blocks_overwrite_and_delete() {
    let root = tmp("lib-ro-actions");
    let mut lib = Library::open(root.join("lib"));
    let a = lib.save_as("A", &patch("A", 1)).unwrap();
    let before = std::fs::read(&a).unwrap();
    lib.set_read_only(&a, true).unwrap();
    assert!(lib.overwrite(&a, &patch("A", 2)).is_err());
    assert!(lib.delete(&a).is_err());
    assert_eq!(std::fs::read(&a).unwrap(), before);
    lib.overwrite(&a, &patch("A", 2)).unwrap_err();
    lib.set_read_only(&a, false).unwrap();
    lib.overwrite(&a, &patch("A", 3)).unwrap();
    assert_eq!(gain_of(&lib.load(&a).unwrap()), Some(3));
    lib.delete(&a).unwrap();
    assert!(!a.exists());
    assert!(lib.patches().is_empty());
}

#[test]
fn delete_set_keeps_picked_patches() {
    let (_root, mut lib, set) = lib_with_set("lib-del-set", "S", &[patch("Iron Man", 1)]);
    let (picked, _) = lib.pick(&set, 0).unwrap();
    lib.delete(&set).unwrap();
    assert!(lib.sets().is_empty());
    assert!(!set.exists());
    assert!(picked.exists());
    assert_eq!(lib.patches().len(), 1);
}

#[test]
fn deleted_patch_does_not_pass_its_lock_to_a_new_file() {
    let root = tmp("lib-del-flag");
    let mut lib = Library::open(root.join("lib"));
    let a = lib.save_as("A", &patch("A", 1)).unwrap();
    lib.set_read_only(&a, true).unwrap();
    lib.set_read_only(&a, false).unwrap();
    lib.delete(&a).unwrap();
    let again = lib.save_as("A", &patch("A", 1)).unwrap();
    assert_eq!(again, a);
    assert!(!lib.is_read_only(&again));
}
