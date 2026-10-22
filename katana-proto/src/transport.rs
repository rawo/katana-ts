//! Byte-level link to the amp, abstracted so protocol logic can be tested
//! against `FakeAmp` without hardware.

use std::time::Duration;

use crate::ProtoError;

pub trait Transport {
    fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError>;
    /// Next complete SysEx message, or `Ok(None)` after `timeout`.
    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError>;
}
