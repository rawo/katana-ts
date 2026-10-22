//! Protocol library for the BOSS KATANA Gen 3 USB-MIDI SysEx interface.

pub mod address;
pub mod checksum;
pub mod error;

pub use address::{Address, Size};
pub use checksum::roland_checksum;
pub use error::ProtoError;
