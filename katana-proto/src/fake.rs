//! In-memory amp double: answers RQ1 from `mem`, applies DT1 to it.

use std::collections::VecDeque;
use std::time::Duration;

use crate::device::{self, PATCH_BLOCKS};
use crate::patch::slot_offset;
use crate::{decode, encode, Address, Command, Header, MemoryMap, ProtoError, Transport};

pub struct FakeAmp {
    pub header: Header,
    pub mem: MemoryMap,
    /// Every message the host sent, in order.
    pub sent: Vec<Vec<u8>>,
    /// Largest DT1 payload in a reply.
    pub max_dt1: usize,
    /// Never reply to anything.
    pub silent: bool,
    /// Unrelated messages delivered before each RQ1 reply.
    pub noise: Vec<Vec<u8>>,
    /// Accept DT1 without applying it (an amp that silently drops writes).
    pub ignore_dt1: bool,
    /// (addr, len) ranges whose DT1 bytes are accepted but not applied.
    pub read_only: Vec<(u32, u32)>,
    /// Editor mode (`device::EDITOR_MODE`); channel select and store need it.
    pub editor: bool,
    /// Send the amp's own messages after a channel select or a store.
    pub echo: bool,
    pending: VecDeque<Vec<u8>>,
}

impl FakeAmp {
    /// Queues `msg` as if the amp had sent it by itself.
    pub fn queue(&mut self, msg: Vec<u8>) {
        self.pending.push_back(msg);
    }

    pub fn new(header: Header, max_dt1: usize) -> FakeAmp {
        FakeAmp {
            header,
            mem: MemoryMap::default(),
            sent: Vec::new(),
            max_dt1,
            silent: false,
            noise: Vec::new(),
            ignore_dt1: false,
            read_only: Vec::new(),
            editor: false,
            echo: true,
            pending: VecDeque::new(),
        }
    }
}

/// Command area and selector: never stored as patch data.
fn is_control(a: u32) -> bool {
    a == device::SELECTOR || a >> 21 == 0x7F
}

impl FakeAmp {
    fn control(&mut self, a: u32, data: &[u8]) -> Result<(), ProtoError> {
        match (a, data) {
            (device::EDITOR_MODE, &[on]) => self.editor = on == 1,
            (device::PATCH_SELECT, &[0, n]) if self.editor => {
                self.mem.apply(Address(device::SELECTOR), &[0, n]);
                self.copy_patch(slot_offset(n + 1), 0);
                self.notify(device::SELECTOR, &[0, n])?;
            }
            (device::PATCH_WRITE, &[0, n]) if self.editor => {
                self.copy_patch(0, slot_offset(n + 1));
                self.notify(device::PATCH_WRITE, &[0, n])?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Copies every patch block from offset `from` to offset `to` (0 = current patch).
    fn copy_patch(&mut self, from: u32, to: u32) {
        for &(a, s) in PATCH_BLOCKS {
            for k in a..a + s {
                if let Some(&b) = self.mem.0.get(&(k + from)) {
                    self.mem.0.insert(k + to, b);
                }
            }
        }
    }

    fn notify(&mut self, a: u32, data: &[u8]) -> Result<(), ProtoError> {
        if self.echo {
            self.pending.push_back(encode(&self.header, &Command::Dt1 { addr: Address(a), data: data.to_vec() })?);
        }
        Ok(())
    }
}

impl Transport for FakeAmp {
    fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError> {
        self.sent.push(msg.to_vec());
        match decode(&self.header, msg) {
            Ok(Command::Dt1 { addr, data }) if !self.ignore_dt1 && is_control(addr.0) => {
                self.control(addr.0, &data)?;
            }
            Ok(Command::Dt1 { addr, data }) if !self.ignore_dt1 => {
                for (i, &b) in data.iter().enumerate() {
                    let k = addr.0 + i as u32;
                    if !self.read_only.iter().any(|&(a, n)| (a..a + n).contains(&k)) {
                        self.mem.0.insert(k, b);
                    }
                }
            }
            Ok(Command::Rq1 { addr, size }) if !self.silent => {
                self.pending.extend(self.noise.iter().cloned());
                let data: Vec<u8> =
                    (addr.0..addr.0 + size.0).map(|k| self.mem.0.get(&k).copied().unwrap_or(0)).collect();
                for (i, chunk) in data.chunks(self.max_dt1).enumerate() {
                    let c = Command::Dt1 { addr: addr + (i * self.max_dt1) as u32, data: chunk.to_vec() };
                    self.pending.push_back(encode(&self.header, &c)?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn recv(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, ProtoError> {
        Ok(self.pending.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Address;

    #[test]
    fn fake_amp_answers_rq1_in_chunks() {
        let h = Header { device_id: 0x10, model_id: vec![0x01], addr_width: 4 };
        let mut amp = FakeAmp::new(h.clone(), 4);
        amp.mem.apply(Address(0), &[1, 2, 3, 4, 5, 6]);
        amp.send(&encode(&h, &Command::Rq1 { addr: Address(0), size: Address(6) }).unwrap()).unwrap();
        let first = decode(&h, &amp.recv(Duration::ZERO).unwrap().unwrap()).unwrap();
        assert_eq!(first, Command::Dt1 { addr: Address(0), data: vec![1, 2, 3, 4] });
    }

    #[test]
    fn fake_amp_applies_dt1_and_records_sent() {
        let h = Header { device_id: 0x10, model_id: vec![0x01], addr_width: 4 };
        let mut amp = FakeAmp::new(h.clone(), 4);
        let msg = encode(&h, &Command::Dt1 { addr: Address(5), data: vec![7] }).unwrap();
        amp.send(&msg).unwrap();
        assert_eq!(amp.mem.read(Address(5), 1), Some(vec![7]));
        assert_eq!(amp.sent, vec![msg]);
        assert_eq!(amp.recv(Duration::ZERO).unwrap(), None);
    }
}
