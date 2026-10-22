//! Protocol library for the BOSS KATANA Gen 3 USB-MIDI SysEx interface.

pub mod address;
pub mod checksum;
pub mod error;

pub use address::{Address, Size};
pub use checksum::roland_checksum;
pub use error::ProtoError;
pub mod identity;
pub mod sysex;

pub use identity::{parse_identity_reply, IdentityReply, IDENTITY_REQUEST};
pub use sysex::{decode, encode, Command, Header};
