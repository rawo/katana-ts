use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("byte {byte:#04x} at index {index} is not 7-bit")]
    NotSevenBit { index: usize, byte: u8 },
    #[error("address does not fit the requested width")]
    AddressOverflow,
}
