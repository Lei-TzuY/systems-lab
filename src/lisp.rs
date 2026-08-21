//! Locator/ID Separation Protocol (LISP - RFC 9300 Data / RFC 9301 Control).
//!
//! EID-to-RLOC overlay mapping and encapsulation for VM mobility and multihoming over UDP 4341/4342.

use crate::ipv4::Ipv4Address;
use std::collections::BTreeMap;
use std::fmt;

pub const LISP_DATA_PORT: u16 = 4341;
pub const LISP_CONTROL_PORT: u16 = 4342;

// LISP Control Message Types
pub const LISP_MSG_MAP_REQUEST: u8 = 1;
pub const LISP_MSG_MAP_REPLY: u8 = 2;
pub const LISP_MSG_MAP_REGISTER: u8 = 3;
pub const LISP_MSG_MAP_NOTIFY: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LispDataHeader {
    pub flags: u8,
    pub nonce: u32, // 24-bit Nonce
    pub lsb: u32,   // Locator-Status-Bits (32-bit)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LispDataPacket {
    pub header: LispDataHeader,
    pub inner_payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LispLocator {
    pub priority: u8,
    pub weight: u8,
    pub rloc_ip: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LispMapRequest {
    pub nonce: u64,
    pub source_eid: Ipv4Address,
    pub itr_rloc: Ipv4Address,
    pub target_eid: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LispMapReply {
    pub nonce: u64,
    pub target_eid: Ipv4Address,
    pub eid_mask_len: u8,
    pub record_ttl_s: u32,
    pub locators: Vec<LispLocator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LispError {
    PacketTooShort(usize),
    InvalidLength,
}

impl fmt::Display for LispError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispError::PacketTooShort(l) => write!(f, "LISP packet too short ({} bytes)", l),
            LispError::InvalidLength => write!(f, "Invalid LISP packet length"),
        }
    }
}

impl std::error::Error for LispError {}

impl LispDataPacket {
    pub fn encapsulate(nonce: u32, lsb: u32, inner_ip_pkt: &[u8]) -> Self {
        LispDataPacket {
            header: LispDataHeader {
                flags: 0x08, // Nonce-present bit
                nonce: nonce & 0x00FF_FFFF,
                lsb,
            },
            inner_payload: inner_ip_pkt.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.header.flags);
        let n_bytes = self.header.nonce.to_be_bytes();
        buf.extend_from_slice(&n_bytes[1..4]); // 24-bit nonce
        buf.extend_from_slice(&self.header.lsb.to_be_bytes());
        buf.extend_from_slice(&self.inner_payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, LispError> {
        if data.len() < 8 {
            return Err(LispError::PacketTooShort(data.len()));
        }

        let flags = data[0];
        let nonce = u32::from_be_bytes([0, data[1], data[2], data[3]]);
        let lsb = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let inner_payload = data[8..].to_vec();

        Ok(LispDataPacket {
            header: LispDataHeader { flags, nonce, lsb },
            inner_payload,
        })
    }
}

impl LispMapRequest {
    pub fn build(nonce: u64, source_eid: Ipv4Address, itr_rloc: Ipv4Address, target_eid: Ipv4Address) -> Self {
        LispMapRequest {
            nonce,
            source_eid,
            itr_rloc,
            target_eid,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(LISP_MSG_MAP_REQUEST << 4); // Type 1
        buf.push(0); // Flags
        buf.push(0); // Reserved
        buf.push(1); // Record Count = 1
        buf.extend_from_slice(&self.nonce.to_be_bytes());

        // Source EID (AFI 1 IPv4)
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&self.source_eid.0);

        // ITR RLOC (AFI 1 IPv4)
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&self.itr_rloc.0);

        // Target EID Record (AFI 1 IPv4, Mask 32)
        buf.push(0); // Reserved
        buf.push(32); // Mask length
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.extend_from_slice(&self.target_eid.0);

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 32 {
            return None;
        }

        let msg_type = data[0] >> 4;
        if msg_type != LISP_MSG_MAP_REQUEST {
            return None;
        }

        let nonce = u64::from_be_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]);
        let source_eid = Ipv4Address([data[14], data[15], data[16], data[17]]);
        let itr_rloc = Ipv4Address([data[20], data[21], data[22], data[23]]);
        let target_eid = Ipv4Address([data[28], data[29], data[30], data[31]]);

        Some(LispMapRequest {
            nonce,
            source_eid,
            itr_rloc,
            target_eid,
        })
    }
}

impl LispMapReply {
    pub fn build(nonce: u64, target_eid: Ipv4Address, mask_len: u8, ttl_s: u32, locators: &[LispLocator]) -> Self {
        LispMapReply {
            nonce,
            target_eid,
            eid_mask_len: mask_len,
            record_ttl_s: ttl_s,
            locators: locators.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(LISP_MSG_MAP_REPLY << 4); // Type 2
        buf.push(0); // Flags
        buf.push(0); // Reserved
        buf.push(1); // Record Count = 1
        buf.extend_from_slice(&self.nonce.to_be_bytes());

        // EID Record
        buf.extend_from_slice(&self.record_ttl_s.to_be_bytes());
        buf.push(self.locators.len() as u8);
        buf.push(self.eid_mask_len);
        buf.push(0); // Action = No-Action
        buf.push(1); // Authoritative = 1

        buf.extend_from_slice(&1u16.to_be_bytes()); // AFI 1 IPv4
        buf.extend_from_slice(&self.target_eid.0);

        for loc in &self.locators {
            buf.push(loc.priority);
            buf.push(loc.weight);
            buf.extend_from_slice(&[0, 0]); // Flags + Reserved
            buf.extend_from_slice(&1u16.to_be_bytes()); // AFI 1 IPv4
            buf.extend_from_slice(&loc.rloc_ip.0);
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 24 {
            return None;
        }

        let msg_type = data[0] >> 4;
        if msg_type != LISP_MSG_MAP_REPLY {
            return None;
        }

        let nonce = u64::from_be_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]);
        let record_ttl_s = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let loc_count = data[16] as usize;
        let eid_mask_len = data[17];
        let target_eid = Ipv4Address([data[22], data[23], data[24], data[25]]);

        let mut locators = Vec::new();
        let mut offset = 26;

        for _ in 0..loc_count {
            if offset + 10 > data.len() {
                break;
            }
            let priority = data[offset];
            let weight = data[offset + 1];
            let rloc_ip = Ipv4Address([data[offset + 6], data[offset + 7], data[offset + 8], data[offset + 9]]);
            locators.push(LispLocator { priority, weight, rloc_ip });
            offset += 10;
        }

        Some(LispMapReply {
            nonce,
            target_eid,
            eid_mask_len,
            record_ttl_s,
            locators,
        })
    }
}

/// Simulated in-memory LISP Map-Server / Map-Resolver
#[derive(Debug, Clone, Default)]
pub struct LispMapResolver {
    pub database: BTreeMap<Ipv4Address, Vec<LispLocator>>,
}

impl LispMapResolver {
    pub fn new() -> Self {
        LispMapResolver {
            database: BTreeMap::new(),
        }
    }

    pub fn register_eid(&mut self, eid: Ipv4Address, rloc: Ipv4Address, priority: u8, weight: u8) {
        self.database
            .entry(eid)
            .or_default()
            .push(LispLocator { priority, weight, rloc_ip: rloc });
    }

    pub fn resolve(&self, req: &LispMapRequest) -> Option<LispMapReply> {
        let locs = self.database.get(&req.target_eid)?;
        Some(LispMapReply::build(req.nonce, req.target_eid, 32, 1440, locs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lisp_data_and_control_mapping() {
        let eid_dst = Ipv4Address::new(10, 1, 1, 50);
        let rloc_gateway = Ipv4Address::new(198, 51, 100, 1);

        let mut resolver = LispMapResolver::new();
        resolver.register_eid(eid_dst, rloc_gateway, 1, 100);

        // Control Plane: Map-Request -> Map-Reply
        let req = LispMapRequest::build(0x1122334455667788, Ipv4Address::new(10, 0, 0, 1), Ipv4Address::new(192, 0, 2, 1), eid_dst);
        let raw_req = req.serialize();
        assert_eq!(raw_req.len() >= 30, true);

        let parsed_req = LispMapRequest::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.target_eid, eid_dst);

        let rep = resolver.resolve(&parsed_req).unwrap();
        let raw_rep = rep.serialize();
        let parsed_rep = LispMapReply::parse(&raw_rep).unwrap();
        assert_eq!(parsed_rep.locators.len(), 1);
        assert_eq!(parsed_rep.locators[0].rloc_ip, rloc_gateway);

        // Data Plane: LISP Encapsulation
        let inner_ip = vec![0x45, 0x00, 0x00, 0x20];
        let data_pkt = LispDataPacket::encapsulate(0x123456, 0x00000001, &inner_ip);
        let raw_data = data_pkt.serialize();
        let parsed_data = LispDataPacket::parse(&raw_data).unwrap();
        assert_eq!(parsed_data.header.nonce, 0x123456);
        assert_eq!(parsed_data.inner_payload, inner_ip);

        assert_eq!(LISP_DATA_PORT, 4341);
        assert_eq!(LISP_CONTROL_PORT, 4342);
    }
}
