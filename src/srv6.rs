//! Segment Routing over IPv6 (SRv6 - RFC 8754 / RFC 8986).
//!
//! Modern source-routed network programming over IPv6 Extension Headers.

use crate::ipv6::{Ipv6Address, Ipv6Header};
use std::fmt;

pub const IPV6_EXT_ROUTING: u8 = 43;
pub const SRV6_ROUTING_TYPE: u8 = 4;
pub const SRV6_FIXED_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv6Header {
    pub next_header: u8,
    pub segments_left: u8,
    pub last_entry: u8,
    pub flags: u8,
    pub tag: u16,
    pub segment_list: Vec<Ipv6Address>, // SIDs from Segment[0] to Segment[Last Entry]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv6Packet {
    pub srh: Srv6Header,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Srv6Error {
    HeaderTooShort(usize),
    InvalidRoutingType(u8),
    InvalidLength,
}

impl fmt::Display for Srv6Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Srv6Error::HeaderTooShort(l) => write!(f, "SRv6 header too short ({} bytes)", l),
            Srv6Error::InvalidRoutingType(t) => write!(f, "Invalid IPv6 Routing Type: expected 4, found {}", t),
            Srv6Error::InvalidLength => write!(f, "Invalid SRv6 Segment List length"),
        }
    }
}

impl std::error::Error for Srv6Error {}

impl Srv6Header {
    pub fn build(next_header: u8, segments: &[Ipv6Address]) -> Self {
        assert!(!segments.is_empty(), "SRv6 segment list cannot be empty");
        let last_entry = (segments.len() - 1) as u8;
        Srv6Header {
            next_header,
            segments_left: last_entry,
            last_entry,
            flags: 0,
            tag: 0,
            segment_list: segments.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.next_header);
        // Hdr Ext Len in 8-octet units, not including first 8 octets
        // Total bytes = 8 + 16 * (last_entry + 1)
        // Hdr Ext Len = (16 * (last_entry + 1)) / 8 = 2 * (last_entry + 1)
        let hdr_ext_len = 2 * (self.last_entry + 1);
        buf.push(hdr_ext_len);
        buf.push(SRV6_ROUTING_TYPE);
        buf.push(self.segments_left);
        buf.push(self.last_entry);
        buf.push(self.flags);
        buf.extend_from_slice(&self.tag.to_be_bytes());

        for sid in &self.segment_list {
            buf.extend_from_slice(&sid.0);
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Result<(Self, usize), Srv6Error> {
        if data.len() < SRV6_FIXED_HEADER_LEN {
            return Err(Srv6Error::HeaderTooShort(data.len()));
        }

        let next_header = data[0];
        let hdr_ext_len = data[1] as usize;
        let routing_type = data[2];
        if routing_type != SRV6_ROUTING_TYPE {
            return Err(Srv6Error::InvalidRoutingType(routing_type));
        }

        let segments_left = data[3];
        let last_entry = data[4];
        let flags = data[5];
        let tag = u16::from_be_bytes([data[6], data[7]]);

        let total_len = 8 + hdr_ext_len * 8;
        if data.len() < total_len {
            return Err(Srv6Error::InvalidLength);
        }

        let num_segments = (last_entry as usize) + 1;
        let mut segment_list = Vec::new();
        let mut offset = 8;

        for _ in 0..num_segments {
            if offset + 16 > data.len() {
                return Err(Srv6Error::InvalidLength);
            }
            let mut sid_bytes = [0u8; 16];
            sid_bytes.copy_from_slice(&data[offset..offset + 16]);
            segment_list.push(Ipv6Address(sid_bytes));
            offset += 16;
        }

        Ok((
            Srv6Header {
                next_header,
                segments_left,
                last_entry,
                flags,
                tag,
                segment_list,
            },
            total_len,
        ))
    }

    /// Advance active SID pointer and update IPv6 destination address
    pub fn advance_hop(&mut self, ip_header: &mut Ipv6Header) -> bool {
        if self.segments_left > 0 {
            self.segments_left -= 1;
            let next_sid = self.segment_list[self.segments_left as usize];
            ip_header.dst_ip = next_sid;
            true
        } else {
            false // Final destination reached
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_srv6_header_and_hop_advancement() {
        let sid1 = Ipv6Address::from_str("2001:db8:1::1").unwrap();
        let sid2 = Ipv6Address::from_str("2001:db8:2::1").unwrap();
        let sid3 = Ipv6Address::from_str("2001:db8:3::1").unwrap();

        let mut srh = Srv6Header::build(59, &[sid1, sid2, sid3]);
        let raw = srh.serialize();

        assert_eq!(srh.segments_left, 2);
        assert_eq!(srh.last_entry, 2);

        let (parsed_srh, len) = Srv6Header::parse(&raw).unwrap();
        assert_eq!(len, 8 + 16 * 3);
        assert_eq!(parsed_srh.segment_list.len(), 3);

        let mut ip_hdr = Ipv6Header {
            version: 6,
            traffic_class: 0,
            flow_label: 0,
            payload_length: 0,
            next_header: IPV6_EXT_ROUTING,
            hop_limit: 64,
            src_ip: Ipv6Address::from_str("2001:db8::100").unwrap(),
            dst_ip: sid3,
        };

        // Advance hop 1
        assert!(srh.advance_hop(&mut ip_hdr));
        assert_eq!(srh.segments_left, 1);
        assert_eq!(ip_hdr.dst_ip, sid2);

        // Advance hop 2
        assert!(srh.advance_hop(&mut ip_hdr));
        assert_eq!(srh.segments_left, 0);
        assert_eq!(ip_hdr.dst_ip, sid1);

        // No more hops
        assert!(!srh.advance_hop(&mut ip_hdr));
    }
}
