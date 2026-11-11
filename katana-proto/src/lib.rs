//! Protocol library for the BOSS KATANA Gen 3 USB-MIDI SysEx interface.

pub mod address;
pub mod amp;
pub mod backup;
pub mod capture;
pub mod channel;
pub mod check;
pub mod checksum;
pub mod device;
pub mod error;

pub use address::{Address, Size};
pub use channel::Channel;
pub use checksum::roland_checksum;
pub use error::ProtoError;
pub mod identity;
pub mod sysex;
pub mod syx;

pub use capture::{extract_sysex, load_tshark_export, CapturedMessage, Direction};
pub use identity::{parse_identity_reply, IdentityReply, IDENTITY_REQUEST};
pub use sysex::{decode, encode, Command, Header};

#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod layout;
pub mod memmap;
#[cfg(feature = "midi")]
pub mod midi;
pub mod modfx;
pub mod params;
pub mod settings;
pub mod patch;
pub mod transport;
pub mod tsl;

#[cfg(any(test, feature = "testing"))]
pub use fake::FakeAmp;
pub use memmap::MemoryMap;
#[cfg(feature = "midi")]
pub use midi::{MidirTransport, DEFAULT_PORT};
pub use params::{all_params, encode_param, find_param, format_value, parse_shown, set_param, Encoding, Format, ParamDef, PARAMS};
pub use patch::{read_blocks, read_patch, read_slot, verify_write_back, write_patch, Patch, SLOTS};
pub use syx::{read_syx, write_syx};
pub use transport::Transport;
