//! Sparse byte image of the amp's address space, built from DT1 messages.

use std::collections::BTreeMap;

use crate::{Address, Command};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryMap(pub BTreeMap<u32, u8>);

impl MemoryMap {
    pub fn apply(&mut self, addr: Address, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            self.0.insert(addr.0 + i as u32, b);
        }
    }

    /// Applies every DT1 in order; RQ1s are ignored.
    pub fn from_commands<'a>(cmds: impl IntoIterator<Item = &'a Command>) -> MemoryMap {
        let mut m = MemoryMap::default();
        for c in cmds {
            if let Command::Dt1 { addr, data } = c {
                m.apply(*addr, data);
            }
        }
        m
    }

    /// Every address whose byte differs (or exists on one side only), ascending.
    pub fn diff(&self, other: &MemoryMap) -> Vec<(Address, Option<u8>, Option<u8>)> {
        let keys: std::collections::BTreeSet<u32> = self.0.keys().chain(other.0.keys()).copied().collect();
        keys.into_iter()
            .filter_map(|k| {
                let (a, b) = (self.0.get(&k).copied(), other.0.get(&k).copied());
                (a != b).then_some((Address(k), a, b))
            })
            .collect()
    }

    /// `len` bytes from `addr`, or `None` if any of them is missing.
    pub fn read(&self, addr: Address, len: u32) -> Option<Vec<u8>> {
        (addr.0..addr.0 + len).map(|k| self.0.get(&k).copied()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Address;

    #[test]
    fn diff_reports_changed_byte() {
        let mut a = MemoryMap::default();
        a.apply(Address(0x100), &[1, 2, 3]);
        let mut b = a.clone();
        b.apply(Address(0x101), &[9]);
        assert_eq!(a.diff(&b), vec![(Address(0x101), Some(2), Some(9))]);
    }

    #[test]
    fn read_requires_full_coverage() {
        let mut m = MemoryMap::default();
        m.apply(Address(0), &[1, 2]);
        assert_eq!(m.read(Address(0), 2), Some(vec![1, 2]));
        assert_eq!(m.read(Address(0), 3), None);
    }
}
