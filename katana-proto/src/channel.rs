//! The amp's stored channels A1–A4, B1–B4 (see docs/protocol.md, "Channels").

use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    A1,
    A2,
    A3,
    A4,
    B1,
    B2,
    B3,
    B4,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown channel {0:?}; use A1–A4 or B1–B4")]
pub struct ChannelError(pub String);

impl Channel {
    pub const ALL: [Channel; 8] =
        [Channel::A1, Channel::A2, Channel::A3, Channel::A4, Channel::B1, Channel::B2, Channel::B3, Channel::B4];

    /// The amp's channel selector (`SETUP PATCH_NUMBER`): A1 = 1 … B4 = 8; PANEL is 0.
    pub fn selector(self) -> u8 {
        Channel::ALL.iter().position(|&c| c == self).expect("in ALL") as u8 + 1
    }

    /// The stored slot PATCH(n) holding this channel, for `read_slot`.
    pub fn slot(self) -> u8 {
        self.selector() + 1
    }
}

impl FromStr for Channel {
    type Err = ChannelError;

    fn from_str(s: &str) -> Result<Channel, ChannelError> {
        Channel::ALL.into_iter().find(|c| c.to_string().eq_ignore_ascii_case(s)).ok_or_else(|| ChannelError(s.into()))
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_any_case() {
        assert_eq!("a3".parse(), Ok(Channel::A3));
        assert_eq!("B4".parse(), Ok(Channel::B4));
    }

    #[test]
    fn rejects_others() {
        for s in ["PANEL", "A5", "C1", ""] {
            assert!(s.parse::<Channel>().is_err(), "{s:?}");
        }
    }

    #[test]
    fn selector_and_slot() {
        assert_eq!(Channel::A1.selector(), 1);
        assert_eq!(Channel::A4.selector(), 4);
        assert_eq!(Channel::B1.selector(), 5);
        assert_eq!(Channel::B4.selector(), 8);
        assert_eq!(Channel::B4.slot(), 9);
    }

    #[test]
    fn display_round_trips() {
        for c in Channel::ALL {
            assert_eq!(c.to_string().parse(), Ok(c));
        }
    }
}
