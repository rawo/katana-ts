use eframe::egui;
use katana_gui::app::{KatanaApp, MIN_WINDOW};
use katana_gui::library::Library;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 960.0])
            .with_min_inner_size(MIN_WINDOW)
            .with_title("KATANA Tone Studio"),
        ..Default::default()
    };
    let app = match Library::default_root() {
        Some(root) => KatanaApp::start(Library::open(root)),
        None => {
            let mut app = KatanaApp::new();
            app.error = Some("No home folder found; the library is unavailable".into());
            app
        }
    };
    eframe::run_native("KATANA Tone Studio", options, Box::new(|_cc| Ok(Box::new(app))))
}
