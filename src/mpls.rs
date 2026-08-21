//! Multi-Protocol Label Switching (MPLS - RFC 3031 / RFC 3032).
//!
//! Layer 2.5 label switching shim headers, label stack push/swap/pop operations,
//! and an in-memory Label Forwarding Information Base (LFIB).

use std::collections::HashMap;
use std::fmt;

pub const ETHERTYPE_MPLS_UNICAST: u16 = 0x8847;
pub const ETHERTYPE_MPLS_MULTICAST: u16 = 0x8848;

pub const MPLS_LABEL_EXPLICIT_NULL: u32 = 0;
pub const MPLS_LABEL_ROUTER_ALERT: u32 = 1;
pub const MPLS_LABEL_IPV6_EXPLICIT_NULL: u32 = 2;
pub const MPLS_LABEL_IMPLICIT_NULL: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MplsHeader {
    pub label: u32, // 20 bits (0..1048575)
    pub tc: u8,     // 3 bits Traffic Class / EXP
    pub bottom_of_stack: bool, // 1 bit S flag
    pub ttl: u8,    // 8 bits TTL
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MplsError {
    HeaderTooShort(usize),
    InvalidLabel(u32),
}

impl fmt::Display for MplsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MplsError::HeaderTooShort(len) => write!(f, "MPLS header too short ({} bytes, min 4)", len),
            MplsError::InvalidLabel(l) => write!(f, "MPLS label out of 20-bit range: {}", l),
        }
    }
}

impl std::error::Error for MplsError {}

impl MplsHeader {
    pub fn new(label: u32, tc: u8, bottom_of_stack: bool, ttl: u8) -> Self {
        MplsHeader {
            label: label & 0x000F_FFFF,
            tc: tc & 0x07,
            bottom_of_stack,
            ttl,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, MplsError> {
        if data.len() < 4 {
            return Err(MplsError::HeaderTooShort(data.len()));
        }

        let raw = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let label = (raw >> 12) & 0x000F_FFFF;
        let tc = ((raw >> 9) & 0x07) as u8;
        let bottom_of_stack = ((raw >> 8) & 0x01) == 1;
        let ttl = (raw & 0xFF) as u8;

        Ok(MplsHeader {
            label,
            tc,
            bottom_of_stack,
            ttl,
        })
    }

    pub fn serialize(&self) -> [u8; 4] {
        let mut raw = (self.label & 0x000F_FFFF) << 12;
        raw |= ((self.tc as u32) & 0x07) << 9;
        if self.bottom_of_stack {
            raw |= 1 << 8;
        }
        raw |= self.ttl as u32;

        raw.to_be_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MplsPacket {
    pub labels: Vec<MplsHeader>,
    pub payload: Vec<u8>,
}

impl MplsPacket {
    pub fn new(labels: Vec<MplsHeader>, payload: Vec<u8>) -> Self {
        MplsPacket { labels, payload }
    }

    pub fn parse(data: &[u8]) -> Result<Self, MplsError> {
        let mut labels = Vec::new();
        let mut offset = 0;

        loop {
            if offset + 4 > data.len() {
                return Err(MplsError::HeaderTooShort(data.len() - offset));
            }
            let hdr = MplsHeader::parse(&data[offset..offset + 4])?;
            let is_bos = hdr.bottom_of_stack;
            labels.push(hdr);
            offset += 4;
            if is_bos {
                break;
            }
        }

        let payload = data[offset..].to_vec();
        Ok(MplsPacket { labels, payload })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.labels.len() * 4 + self.payload.len());
        for (i, lbl) in self.labels.iter().enumerate() {
            let mut l = *lbl;
            // Ensure last label has bottom_of_stack set
            l.bottom_of_stack = i == self.labels.len() - 1;
            buf.extend_from_slice(&l.serialize());
        }
        buf.extend_from_slice(&self.payload);
        buf
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LfibAction {
    Push(u32),       // Ingress LER: Encapsulate with new Label
    Swap(u32, &'static str), // Core LSR: Replace incoming label with outgoing label + interface
    Pop,             // Egress LER / PHP: Strip outer label
}

impl fmt::Display for LfibAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LfibAction::Push(l) => write!(f, "PUSH label {}", l),
            LfibAction::Swap(l, iface) => write!(f, "SWAP label {} -> out: {}", l, iface),
            LfibAction::Pop => write!(f, "POP (PHP)"),
        }
    }
}

/// Label Forwarding Information Base (LFIB)
pub struct LfibTable {
    entries: HashMap<u32, LfibAction>,
}

impl Default for LfibTable {
    fn default() -> Self {
        Self::new()
    }
}

impl LfibTable {
    pub fn new() -> Self {
        let mut lfib = LfibTable {
            entries: HashMap::new(),
        };
        // Standard virtual MPLS paths
        lfib.insert(100, LfibAction::Swap(200, "eth1"));
        lfib.insert(200, LfibAction::Swap(300, "eth2"));
        lfib.insert(300, LfibAction::Pop);
        lfib
    }

    pub fn insert(&mut self, in_label: u32, action: LfibAction) {
        self.entries.insert(in_label, action);
    }

    pub fn lookup(&self, in_label: u32) -> Option<&LfibAction> {
        self.entries.get(&in_label)
    }

    pub fn all_entries(&self) -> &HashMap<u32, LfibAction> {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpls_header_bitfields() {
        let hdr = MplsHeader::new(1048500, 5, true, 64);
        let raw = hdr.serialize();

        let parsed = MplsHeader::parse(&raw).unwrap();
        assert_eq!(parsed.label, 1048500);
        assert_eq!(parsed.tc, 5);
        assert!(parsed.bottom_of_stack);
        assert_eq!(parsed.ttl, 64);
    }

    #[test]
    fn test_mpls_multi_label_stack() {
        let l1 = MplsHeader::new(100, 0, false, 64);
        let l2 = MplsHeader::new(200, 0, true, 63);

        let pkt = MplsPacket {
            labels: vec![l1, l2],
            payload: b"INNER_IP_PAYLOAD".to_vec(),
        };

        let raw = pkt.serialize();
        assert_eq!(raw.len(), 8 + 16);

        let parsed = MplsPacket::parse(&raw).unwrap();
        assert_eq!(parsed.labels.len(), 2);
        assert_eq!(parsed.labels[0].label, 100);
        assert!(!parsed.labels[0].bottom_of_stack);
        assert_eq!(parsed.labels[1].label, 200);
        assert!(parsed.labels[1].bottom_of_stack);
        assert_eq!(parsed.payload, b"INNER_IP_PAYLOAD");
    }

    #[test]
    fn test_lfib_lookup_and_actions() {
        let lfib = LfibTable::new();
        assert_eq!(lfib.lookup(100), Some(&LfibAction::Swap(200, "eth1")));
        assert_eq!(lfib.lookup(300), Some(&LfibAction::Pop));
    }
}
