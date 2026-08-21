//! Multicast Listener Discovery Version 2 (MLDv2 - RFC 3810 / RFC 3569 / RFC 4607).
//!
//! Provides Source-Specific Multicast (SSM) group subscription and listener status reporting for IPv6.

use crate::ipv6::Ipv6Address;
use std::collections::{HashMap, HashSet};

pub const ICMPV6_TYPE_MLD_QUERY: u8 = 130;
pub const ICMPV6_TYPE_MLDV2_REPORT: u8 = 143;

// MLDv2 Record Types (RFC 3810 Section 5.1.4)
pub const MLD_MODE_IS_INCLUDE: u8 = 1;
pub const MLD_MODE_IS_EXCLUDE: u8 = 2;
pub const MLD_CHANGE_TO_INCLUDE: u8 = 3;
pub const MLD_CHANGE_TO_EXCLUDE: u8 = 4;
pub const MLD_ALLOW_NEW_SOURCES: u8 = 5;
pub const MLD_BLOCK_OLD_SOURCES: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MldGroupRecord {
    pub record_type: u8,
    pub multicast_address: Ipv6Address,
    pub source_addresses: Vec<Ipv6Address>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mldv2ReportPacket {
    pub records: Vec<MldGroupRecord>,
}

impl Mldv2ReportPacket {
    pub fn new(records: Vec<MldGroupRecord>) -> Self {
        Mldv2ReportPacket { records }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(0); // Reserved (1B)
        buf.push(0); // Reserved (1B)
        buf.extend_from_slice(&(self.records.len() as u16).to_be_bytes()); // Number of Group Records (2B)

        for rec in &self.records {
            buf.push(rec.record_type);
            buf.push(0); // Aux Data Len (0)
            buf.extend_from_slice(&(rec.source_addresses.len() as u16).to_be_bytes());
            buf.extend_from_slice(&rec.multicast_address.0);
            for src in &rec.source_addresses {
                buf.extend_from_slice(&src.0);
            }
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        let num_records = u16::from_be_bytes([data[2], data[3]]) as usize;
        let mut offset = 4;
        let mut records = Vec::with_capacity(num_records);

        for _ in 0..num_records {
            if offset + 20 > data.len() {
                return None;
            }

            let record_type = data[offset];
            let aux_len = data[offset + 1] as usize;
            let num_sources = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            let mut mc_bytes = [0u8; 16];
            mc_bytes.copy_from_slice(&data[offset + 4..offset + 20]);
            let multicast_address = Ipv6Address(mc_bytes);
            offset += 20;

            if offset + (num_sources * 16) + (aux_len * 4) > data.len() {
                return None;
            }

            let mut source_addresses = Vec::with_capacity(num_sources);
            for _ in 0..num_sources {
                let mut src_bytes = [0u8; 16];
                src_bytes.copy_from_slice(&data[offset..offset + 16]);
                source_addresses.push(Ipv6Address(src_bytes));
                offset += 16;
            }

            offset += aux_len * 4; // Skip aux data
            records.push(MldGroupRecord {
                record_type,
                multicast_address,
                source_addresses,
            });
        }

        Some(Mldv2ReportPacket { records })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MldTable {
    pub group_listeners: HashMap<Ipv6Address, HashSet<Ipv6Address>>, // Group -> Set of Allowed Sources
}

impl MldTable {
    pub fn new() -> Self {
        MldTable {
            group_listeners: HashMap::new(),
        }
    }

    pub fn process_report(&mut self, report: &Mldv2ReportPacket) {
        for rec in &report.records {
            let entry = self.group_listeners.entry(rec.multicast_address).or_default();
            match rec.record_type {
                MLD_MODE_IS_INCLUDE | MLD_CHANGE_TO_INCLUDE | MLD_ALLOW_NEW_SOURCES => {
                    for src in &rec.source_addresses {
                        entry.insert(*src);
                    }
                }
                MLD_MODE_IS_EXCLUDE | MLD_CHANGE_TO_EXCLUDE => {
                    entry.clear(); // Exclude mode accepts all (unconstrained)
                }
                MLD_BLOCK_OLD_SOURCES => {
                    for src in &rec.source_addresses {
                        entry.remove(src);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn is_listener_interested(&self, group: Ipv6Address, source: Ipv6Address) -> bool {
        if let Some(sources) = self.group_listeners.get(&group) {
            if sources.is_empty() {
                return true; // Exclude mode / wildcard
            }
            return sources.contains(&source);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_mldv2_report_codec_and_table() {
        let mut table = MldTable::new();

        let mc_group = Ipv6Address::from_str("ff3e::8000:1").unwrap();
        let src1 = Ipv6Address::from_str("2001:db8:1::10").unwrap();
        let src2 = Ipv6Address::from_str("2001:db8:1::20").unwrap();

        let record = MldGroupRecord {
            record_type: MLD_CHANGE_TO_INCLUDE,
            multicast_address: mc_group,
            source_addresses: vec![src1, src2],
        };

        let report = Mldv2ReportPacket::new(vec![record]);
        let raw = report.serialize();

        let parsed = Mldv2ReportPacket::parse(&raw).unwrap();
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].multicast_address, mc_group);
        assert_eq!(parsed.records[0].source_addresses.len(), 2);

        table.process_report(&parsed);
        assert!(table.is_listener_interested(mc_group, src1));
        assert!(table.is_listener_interested(mc_group, src2));

        let blocked_src = Ipv6Address::from_str("2001:db8:9::99").unwrap();
        assert!(!table.is_listener_interested(mc_group, blocked_src));
    }
}
