//! GRE-over-IPv6 Tunneling (RFC 7676 / RFC 2473).
//!
//! Multi-protocol overlay encapsulation (IPv4, IPv6, MPLS, Ethernet) over native IPv6 networks using Next Header 47 (GRE).

use crate::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_GRE};
use std::fmt;

pub const ETHERTYPE_IPV4_IN_GRE: u16 = 0x0800;
pub const ETHERTYPE_IPV6_IN_GRE: u16 = 0x86DD;
pub const ETHERTYPE_MPLS_IN_GRE: u16 = 0x8847;
pub const ETHERTYPE_ETHERNET_IN_GRE: u16 = 0x6558;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreIpv6Packet {
    pub src_ip6: Ipv6Address,
    pub dst_ip6: Ipv6Address,
    pub protocol_type: u16,
    pub key: Option<u32>,
    pub sequence: Option<u32>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreIpv6Error {
    PacketTooShort(usize),
    InvalidNextHeader,
}

impl fmt::Display for GreIpv6Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GreIpv6Error::PacketTooShort(l) => {
                write!(f, "GRE-over-IPv6 packet too short ({} bytes)", l)
            }
            GreIpv6Error::InvalidNextHeader => write!(f, "Invalid Next Header (expected GRE 47)"),
        }
    }
}

impl std::error::Error for GreIpv6Error {}

impl GreIpv6Packet {
    pub fn new(
        src_ip6: Ipv6Address,
        dst_ip6: Ipv6Address,
        protocol_type: u16,
        key: Option<u32>,
        sequence: Option<u32>,
        payload: &[u8],
    ) -> Self {
        GreIpv6Packet {
            src_ip6,
            dst_ip6,
            protocol_type,
            key,
            sequence,
            payload: payload.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut gre_buf = Vec::new();
        let mut flags: u16 = 0;
        if self.key.is_some() {
            flags |= 0x2000;
        }
        if self.sequence.is_some() {
            flags |= 0x1000;
        }

        gre_buf.extend_from_slice(&flags.to_be_bytes());
        gre_buf.extend_from_slice(&self.protocol_type.to_be_bytes());

        if let Some(k) = self.key {
            gre_buf.extend_from_slice(&k.to_be_bytes());
        }
        if let Some(s) = self.sequence {
            gre_buf.extend_from_slice(&s.to_be_bytes());
        }

        gre_buf.extend_from_slice(&self.payload);

        Ipv6Packet::serialize(self.src_ip6, self.dst_ip6, NEXT_HEADER_GRE, 64, &gre_buf)
    }

    pub fn parse(ipv6_raw: &[u8]) -> Result<Self, GreIpv6Error> {
        let ip6 = Ipv6Packet::parse(ipv6_raw)
            .map_err(|_| GreIpv6Error::PacketTooShort(ipv6_raw.len()))?;
        if ip6.header.next_header != NEXT_HEADER_GRE {
            return Err(GreIpv6Error::InvalidNextHeader);
        }

        let gre_data = ip6.payload;
        if gre_data.len() < 4 {
            return Err(GreIpv6Error::PacketTooShort(gre_data.len()));
        }

        let flags = u16::from_be_bytes([gre_data[0], gre_data[1]]);
        let protocol_type = u16::from_be_bytes([gre_data[2], gre_data[3]]);

        let has_checksum = (flags & 0x8000) != 0;
        let has_key = (flags & 0x2000) != 0;
        let has_seq = (flags & 0x1000) != 0;

        let mut offset = 4;
        if has_checksum {
            offset += 4;
        }

        let key = if has_key {
            if offset + 4 > gre_data.len() {
                return Err(GreIpv6Error::PacketTooShort(gre_data.len()));
            }
            let k = u32::from_be_bytes([
                gre_data[offset],
                gre_data[offset + 1],
                gre_data[offset + 2],
                gre_data[offset + 3],
            ]);
            offset += 4;
            Some(k)
        } else {
            None
        };

        let sequence = if has_seq {
            if offset + 4 > gre_data.len() {
                return Err(GreIpv6Error::PacketTooShort(gre_data.len()));
            }
            let s = u32::from_be_bytes([
                gre_data[offset],
                gre_data[offset + 1],
                gre_data[offset + 2],
                gre_data[offset + 3],
            ]);
            offset += 4;
            Some(s)
        } else {
            None
        };

        let payload = gre_data[offset..].to_vec();

        Ok(GreIpv6Packet {
            src_ip6: ip6.header.src_ip,
            dst_ip6: ip6.header.dst_ip,
            protocol_type,
            key,
            sequence,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_gre_ipv6_encapsulation_and_parse() {
        let src6 = Ipv6Address::from_str("2001:db8:1::1").unwrap();
        let dst6 = Ipv6Address::from_str("2001:db8:2::2").unwrap();

        let inner_payload = b"Multi-Protocol Overlay Packet inside GRE-over-IPv6";
        let gre_pkt = GreIpv6Packet::new(
            src6,
            dst6,
            ETHERTYPE_IPV4_IN_GRE,
            Some(0x00AABBCC),
            Some(1),
            inner_payload,
        );

        let raw = gre_pkt.serialize();
        assert!(raw.len() >= 40 + 12 + inner_payload.len());

        let parsed = GreIpv6Packet::parse(&raw).unwrap();
        assert_eq!(parsed.src_ip6, src6);
        assert_eq!(parsed.dst_ip6, dst6);
        assert_eq!(parsed.protocol_type, ETHERTYPE_IPV4_IN_GRE);
        assert_eq!(parsed.key, Some(0x00AABBCC));
        assert_eq!(parsed.sequence, Some(1));
        assert_eq!(&parsed.payload, inner_payload);
    }
}
