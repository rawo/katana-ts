//! Protocol library for the BOSS KATANA Gen 3 USB-MIDI SysEx interface.

pub mod address;
pub mod capture;
pub mod checksum;
pub mod device;
pub mod error;

pub use address::{Address, Size};
pub use checksum::roland_checksum;
pub use error::ProtoError;
pub mod identity;
pub mod sysex;
pub mod syx;

pub use identity::{parse_identity_reply, IdentityReply, IDENTITY_REQUEST};
pub use sysex::{decode, encode, Command, Header};
pub use capture::{extract_sysex, load_tshark_export, CapturedMessage, Direction};

pub mod memmap;
pub mod params;
pub mod patch;
pub mod transport;
#[cfg(feature = "midi")]
pub mod midi;
#[cfg(any(test, feature = "testing"))]
pub mod fake;

pub use memmap::MemoryMap;
pub use transport::Transport;
#[cfg(feature = "midi")]
pub use midi::{MidirTransport, DEFAULT_PORT};
#[cfg(any(test, feature = "testing"))]
pub use fake::FakeAmp;
pub use syx::{read_syx, write_syx};
pub use patch::{read_patch, write_patch, Patch};
pub use params::{encode_param, find_param, set_param, Encoding, ParamDef, PARAMS};
