//! In-memory amp double: answers RQ1 from `mem`, applies DT1 to it.

use std::collections::VecDeque;
use std::time::Duration;

use crate::{decode, encode, Command, Header, MemoryMap, ProtoError, Transport};

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
    pending: VecDeque<Vec<u8>>,
}

impl FakeAmp {
    pub fn new(header: Header, max_dt1: usize) -> FakeAmp {
        FakeAmp {
            header,
            mem: MemoryMap::default(),
            sent: Vec::new(),
            max_dt1,
            silent: false,
            noise: Vec::new(),
            pending: VecDeque::new(),
        }
    }
}

impl Transport for FakeAmp {
    fn send(&mut self, msg: &[u8]) -> Result<(), ProtoError> {
        self.sent.push(msg.to_vec());
        match decode(&self.header, msg) {
            Ok(Command::Dt1 { addr, data }) => self.mem.apply(addr, &data),
            Ok(Command::Rq1 { addr, size }) if !self.silent => {
                self.pending.extend(self.noise.iter().cloned());
                let data: Vec<u8> = (addr.0..addr.0 + size.0)
                    .map(|k| self.mem.0.get(&k).copied().unwrap_or(0))
                    .collect();
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
