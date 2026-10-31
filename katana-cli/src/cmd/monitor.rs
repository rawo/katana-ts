use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use katana_proto::amp::{notification, set_editor_mode};
use katana_proto::{Header, MidirTransport, Transport};

use super::{describe, hex};

pub fn run(port: &str, h: &Header, editor: bool) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let stop = Arc::new(AtomicBool::new(false));
    if editor {
        let flag = stop.clone();
        ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)).context("installing the Ctrl-C handler")?;
        set_editor_mode(&mut t, true)?;
    }
    eprintln!("listening on {port:?}{} (Ctrl-C to stop)", if editor { " in editor mode" } else { "" });
    while !stop.load(Ordering::SeqCst) {
        if let Some(msg) = t.recv(Duration::from_millis(200))? {
            let line = match describe(h, &msg) {
                Some(d) => format!("{}  ; {d}", hex(&msg)),
                None => hex(&msg),
            };
            match notification(h, &msg).filter(|_| editor) {
                Some(n) => println!("{line}  ; {n:?}"),
                None => println!("{line}"),
            }
        }
    }
    set_editor_mode(&mut t, false)?;
    eprintln!("editor mode off");
    Ok(())
}
