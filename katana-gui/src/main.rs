use eframe::egui;
use katana_gui::app::{KatanaApp, MIN_WINDOW};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 960.0])
            .with_min_inner_size(MIN_WINDOW)
            .with_title("KATANA Tone Studio"),
        ..Default::default()
    };
    eframe::run_native("KATANA Tone Studio", options, Box::new(|_cc| Ok(Box::new(KatanaApp::new()))))
}
