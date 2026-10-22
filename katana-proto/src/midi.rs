//! `Transport` over a real MIDI port via `midir` (ALSA sequencer on Linux).

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::{ProtoError, Transport};

/// Substring of the amp's ALSA port name (`KATANA3 MIDI 1`).
pub const DEFAULT_PORT: &str = "KATANA3";

const CLIENT: &str = "katana-proto";

pub struct MidirTransport {
    _input: MidiInputConnection<()>,
    output: MidiOutputConnection,
    rx: Receiver<Vec<u8>>,
}

fn midi_err(e: impl std::fmt::Display) -> ProtoError {
    ProtoError::Midi(e.to_string())
}

impl MidirTransport {
    pub fn list_ports() -> Result<Vec<String>, ProtoError> {
        let input = MidiInput::new(CLIENT).map_err(midi_err)?;
        input.ports().iter().map(|p| input.port_name(p).map_err(midi_err)).collect()
    }

    /// Opens the first input and output port whose name contains `port_substring`.
    pub fn open(port_substring: &str) -> Result<Self, ProtoError> {
        let not_found = || ProtoError::PortNotFound {
            wanted: port_substring.to_string(),
            available: Self::list_ports().unwrap_or_default(),
        };

        let mut input = MidiInput::new(CLIENT).map_err(midi_err)?;
        // midir drops SysEx by default.
        input.ignore(Ignore::None);
        let in_port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).is_ok_and(|n| n.contains(port_substring)))
            .ok_or_else(not_found)?;

        let output = MidiOutput::new(CLIENT).map_err(midi_err)?;
        let out_port = output
            .ports()
            .into_iter()
            .find(|p| output.port_name(p).is_ok_and(|n| n.contains(port_substring)))
            .ok_or_else(not_found)?;

        let (tx, rx) = channel();
        let mut partial: Vec<u8> = Vec::new();
        let input_conn = input
            .connect(
                &in_port,
                "katana-in",
                move |_, bytes, _| {
                    // Join SysEx the backend may deliver in pieces; drop everything else.
                    if bytes.first() == Some(&0xF0) {
                        partial.clear();
                    } else if partial.is_empty() {
                        return;
                    }
                    partial.extend_from_slice(bytes);
                    if partial.last() == Some(&0xF7) {
                        let _ = tx.send(std::mem::take(&mut partial));
                    }
                },
                (),
            )
            .map_err(midi_err)?;
        let output_conn = output.connect(&out_port, "katana-out").map_err(midi_err)?;

        Ok(MidirTransport { _input: input_conn, output: output_conn, rx })
    }
}

impl Transport for MidirTransport {
    fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError> {
        self.output.send(msg).map_err(midi_err)
    }

    fn recv(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError> {
        match self.rx.recv_timeout(timeout) {
            Ok(m) => Ok(Some(m)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(ProtoError::Midi("input disconnected".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtoError;

    #[test]
    fn open_missing_port_errors() {
        // Without an ALSA sequencer this surfaces as Midi(_); either way it must be an Err.
        assert!(matches!(
            MidirTransport::open("no-such-port-xyz"),
            Err(ProtoError::PortNotFound { .. }) | Err(ProtoError::Midi(_))
        ));
    }
}
