use thiserror::Error;

use crate::Address;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("byte {byte:#04x} at index {index} is not 7-bit")]
    NotSevenBit { index: usize, byte: u8 },
    #[error("address does not fit the requested width")]
    AddressOverflow,
    #[error("not a SysEx message")]
    NotSysEx,
    #[error("SysEx message is truncated")]
    Truncated,
    #[error("manufacturer {0:#04x} is not Roland (0x41)")]
    WrongManufacturer(u8),
    #[error("model ID {got:02X?} does not match expected {expected:02X?}")]
    WrongModel { expected: Vec<u8>, got: Vec<u8> },
    #[error("unknown command byte {0:#04x}")]
    UnknownCommand(u8),
    #[error("bad checksum: expected {expected:#04x}, got {got:#04x}")]
    BadChecksum { expected: u8, got: u8 },
    #[error("malformed capture export at line {line}")]
    BadExport { line: usize },
    #[error("no MIDI port matching {wanted:?}; available: {available:?}")]
    PortNotFound { wanted: String, available: Vec<String> },
    #[error("malformed .syx data at byte {offset}")]
    BadSyx { offset: usize },
    #[error("no complete reply from the amp for block {addr}")]
    Timeout { addr: Address },
    #[error("patch data does not cover block {addr}")]
    IncompletePatch { addr: Address },
    #[error("{name}: value {value} outside {min}..={max}")]
    OutOfRange { name: &'static str, value: u32, min: u32, max: u32 },
    #[error("write-back check failed at {addr}: expected {expected:#04x}, read {got:?}")]
    WriteBackMismatch { addr: Address, expected: u8, got: Option<u8> },
    #[error("scratch patch equals the current patch, so a write-back check would prove nothing")]
    ScratchEqualsCurrent,
    #[error("MIDI error: {0}")]
    Midi(String),
}
