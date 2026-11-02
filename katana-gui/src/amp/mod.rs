//! The connection to the amp: one worker thread owns it and runs one job at a
//! time; the UI sends `Job`s and drains `Event`s through an `AmpLink`.

use std::path::PathBuf;

use katana_proto::{Channel, Patch};

pub mod link;
pub mod worker;

pub use link::{AmpLink, LinkEnd, Stopper};
pub use worker::{coalesce, Opener, PortCheck, Worker};

/// Work for the amp. `u8` values are selectors: 0 = PANEL, 1–8 = A1–B4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    LiveOn,
    LiveOff,
    ReadSlot(u8),
    Select(u8),
    WritePatch(Patch),
    SetParam { addr: u32, data: Vec<u8> },
    Peek(Channel),
    Store { ch: Channel, patch: Patch, backup_dir: PathBuf, stamp: String },
    Shutdown,
}

/// What the worker tells the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connected,
    Disconnected,
    AmpOn(u8),
    Slot(u8, Patch),
    Current(u8, Patch),
    PanelChange {
        addr: u32,
        data: Vec<u8>,
    },
    Peeked(Channel, Patch),
    Stored {
        ch: Channel,
        channel_backup: PathBuf,
        current_backup: PathBuf,
        solo_delay_stored: bool,
    },
    StoreFailed(String),
    Failed(String),
    /// Live edit couldn't start; the worker's Live is off.
    LiveFailed(String),
    /// A long job started (`Some(what)`) or ended (`None`).
    Busy(Option<String>),
}
