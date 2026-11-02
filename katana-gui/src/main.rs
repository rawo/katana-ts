use std::sync::atomic::Ordering;

use eframe::egui;
use katana_gui::amp::{AmpLink, Worker};
use katana_gui::app::{KatanaApp, MIN_WINDOW};
use katana_gui::library::Library;
use katana_proto::{MidirTransport, Transport, DEFAULT_PORT};

/// The amp worker on the real MIDI port.
fn real_worker() -> Worker {
    Worker::new(
        Box::new(|| MidirTransport::open(DEFAULT_PORT).map(|t| Box::new(t) as Box<dyn Transport + Send>)),
        Box::new(|| MidirTransport::list_ports().is_ok_and(|ports| ports.iter().any(|p| p.contains(DEFAULT_PORT)))),
    )
}

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
    eframe::run_native(
        "KATANA Tone Studio",
        options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            let link = AmpLink::spawn(real_worker(), Box::new(move || ctx.request_repaint()));
            let quit = app.quit_signal.clone();
            let ctx = cc.egui_ctx.clone();
            // SIGINT, SIGTERM, SIGHUP: close normally, so editor mode goes off.
            if let Err(e) = ctrlc::set_handler(move || {
                quit.store(true, Ordering::SeqCst);
                ctx.request_repaint();
            }) {
                eprintln!("can't handle quit signals: {e}");
            }
            Ok(Box::new(app.with_amp(link)))
        }),
    )
}
