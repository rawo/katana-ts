use thiserror::Error;

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
    #[error("MIDI error: {0}")]
    Midi(String),
}
