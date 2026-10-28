mod common;

use std::path::{Path, PathBuf};

use common::{gain_of, names_in, patch, tmp, tsl};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use katana_gui::app::{KatanaApp, Tab};
use katana_gui::editor::Origin;
use katana_gui::library::Library;

/// A library in `root` with patch "A" (gain 60), a quick save and the set
/// "Black Sabbath" (Iron Man, Paranoid), shown on the Library tab.
fn filled(root: &Path) -> Harness<'static, KatanaApp> {
    for d in ["patches", "quick-saves", "sets"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("patches/A.syx"), patch("A", 60).to_syx()).unwrap();
    std::fs::write(root.join("quick-saves/quick-save_2026-10-28_15-04-12.syx"), patch("Q", 1).to_syx()).unwrap();
    std::fs::write(root.join("sets/bs.tsl"), tsl("Black Sabbath", &[patch("Iron Man", 33), patch("Paranoid", 2)]))
        .unwrap();
    harness(root)
}

fn harness(root: &Path) -> Harness<'static, KatanaApp> {
    let mut app = KatanaApp::with_library(Library::open(root.to_path_buf()));
    app.tab = Tab::Library;
    let mut h =
        Harness::builder().with_size([1600.0, 960.0]).build_ui_state(|ui, app: &mut KatanaApp| app.show(ui), app);
    h.run();
    h
}

fn a(root: &Path) -> PathBuf {
    root.join("patches/A.syx")
}

#[test]
fn lists_my_patches_quick_saves_and_sets() {
    let d = tmp("tab-list");
    let h = filled(&d);
    for label in
        ["MY PATCHES", "QUICK SAVES", "DOWNLOADED SETS", "A", "quick-save_2026-10-28_15-04-12", "Black Sabbath"]
    {
        h.get_by_label(label);
    }
    let left = h.get_by_label("A").rect().center().x;
    let right = h.get_by_label("Black Sabbath").rect().center().x;
    assert!(left < right, "sets are in the right column");
    assert!(h.get_by_label("A").rect().top() < h.get_by_label("QUICK SAVES").rect().top());
}

#[test]
fn clicking_patch_opens_it() {
    let d = tmp("tab-open");
    let mut h = filled(&d);
    h.get_by_label("A").click();
    h.run();
    assert_eq!(gain_of(h.state().editor.patch()), Some(60));
    assert_eq!(h.state().editor.origin(), &Origin::Patch(a(&d)));
    assert_eq!(h.state().editor.display_name(), "A");
}

#[test]
fn opening_set_patch_picks_it() {
    let d = tmp("tab-pick");
    let mut h = filled(&d);
    h.get_by_label("Black Sabbath").click();
    h.run();
    h.get_by_label("Open Iron Man").click();
    h.run();
    assert!(d.join("patches/Iron Man.syx").exists());
    assert_eq!(gain_of(h.state().editor.patch()), Some(33));
    // The set's row, the new row in My patches and the top bar.
    assert_eq!(h.get_all_by_label("Iron Man").count(), 3);
    assert_eq!(h.state().editor.display_name(), "Iron Man");
}

#[test]
fn search_filters_both_lists() {
    let d = tmp("tab-search");
    let mut h = filled(&d);
    h.get_by_label("Search").set_text_value_accesskit("iron");
    h.run();
    assert!(h.query_by_label("A").is_none());
    h.get_by_label("Black Sabbath");
    h.get_by_label("Open Iron Man");
    assert!(h.query_by_label("Open Paranoid").is_none());
    h.get_by_label("Search").set_text_value_accesskit("sabbath");
    h.run();
    h.get_by_label("Black Sabbath");
}

#[test]
fn sort_by_name_orders_my_patches() {
    let d = tmp("tab-sort");
    let mut h = filled(&d);
    std::fs::write(d.join("patches/b.syx"), patch("b", 1).to_syx()).unwrap();
    std::fs::write(d.join("patches/C.syx"), patch("C", 1).to_syx()).unwrap();
    h.state_mut().library.refresh();
    h.state_mut().sort = katana_gui::library::Sort::NameZa;
    h.run();
    let y = |l: &str| h.get_by_label(l).rect().top();
    assert!(y("C") < y("b") && y("b") < y("A"));
    h.get_by_label("Sort").click();
    h.run();
    h.get_by_label("Name A–Z").click();
    h.run();
    let y = |l: &str| h.get_by_label(l).rect().top();
    assert!(y("A") < y("b") && y("b") < y("C"));
}

#[test]
fn delete_asks_and_respects_read_only() {
    let d = tmp("tab-delete");
    let mut h = filled(&d);
    h.get_by_label("Lock A").click();
    h.run();
    assert!(h.state().library.is_read_only(&a(&d)));
    assert!(h.get_by_label("Delete A").accesskit_node().is_disabled());
    h.get_by_label("Lock A").click();
    h.run();
    h.get_by_label("Delete A").click();
    h.run();
    h.get_by_label("Delete A?");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(a(&d).exists());
    h.get_by_label("Delete A").click();
    h.run();
    h.get_by_label("Delete").click();
    h.run();
    assert!(!a(&d).exists());
    assert!(h.query_by_label("Delete A").is_none());
}

#[test]
fn deleting_the_open_patch_leaves_editor_without_origin() {
    let d = tmp("tab-delete-open");
    let mut h = filled(&d);
    h.get_by_label("A").click();
    h.run();
    h.get_by_label("Delete A").click();
    h.run();
    h.get_by_label("Delete").click();
    h.run();
    assert_eq!(h.state().editor.origin(), &Origin::None);
    assert_eq!(gain_of(h.state().editor.patch()), Some(60));
}

#[test]
fn delete_set_asks_and_keeps_picked_patches() {
    let d = tmp("tab-delete-set");
    let mut h = filled(&d);
    h.get_by_label("Black Sabbath").click();
    h.run();
    h.get_by_label("Open Iron Man").click();
    h.run();
    h.get_by_label("Delete set Black Sabbath").click();
    h.run();
    h.get_by_label("Delete Black Sabbath?");
    h.get_by_label("Delete").click();
    h.run();
    assert!(names_in(&d.join("sets")).is_empty());
    assert!(d.join("patches/Iron Man.syx").exists());
}

#[test]
fn empty_library_says_so() {
    let d = tmp("tab-empty");
    let h = harness(&d);
    h.get_by_label("No patches yet.");
    h.get_by_label("No downloaded sets yet. Import a .tsl from Tone Exchange.");
}
