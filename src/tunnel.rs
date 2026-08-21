//! Network Tunneling & Encapsulation: GRE (RFC 2784) and IP-in-IP (RFC 2003 / RFC 2473).
//!
//! Provides point-to-point tunnel encapsulation and decapsulation for multi-site virtual overlays.

use crate::checksum::compute_checksum;
use crate::ethernet::{ETHERTYPE_IPV4, ETHERTYPE_IPV6};
use crate::ipv4::{Ipv4Address, Ipv4Packet};
use std::fmt;

pub const IP_PROTO_IP_IN_IP: u8 = 4;
pub const IP_PROTO_GRE: u8 = 47;
pub const IP_PROTO_IPV6_IN_IP: u8 = 41;

pub const GRE_FLAG_CHECKSUM: u16 = 0x8000;
pub const GRE_FLAG_KEY: u16 = 0x2000;
pub const GRE_FLAG_SEQUENCE: u16 = 0x1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreHeader {
    pub checksum_present: bool,
    pub key_present: bool,
    pub sequence_present: bool,
    pub version: u8,
    pub protocol_type: u16, // EtherType (e.g. 0x0800 IPv4, 0x86DD IPv6)
    pub checksum: Option<u16>,
    pub key: Option<u32>,
    pub sequence_number: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrePacket<'a> {
    pub header: GreHeader,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidChecksum { computed: u16, expected: u16 },
}

impl fmt::Display for TunnelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TunnelError::PacketTooShort(len) => write!(f, "GRE packet too short ({} bytes)", len),
            TunnelError::InvalidVersion(v) => write!(f, "Invalid GRE version: expected 0, found {}", v),
            TunnelError::InvalidChecksum { computed, expected } => {
                write!(f, "GRE checksum mismatch: computed 0x{:04x}, expected 0x{:04x}", computed, expected)
            }
        }
    }
}

impl std::error::Error for TunnelError {}

impl<'a> GrePacket<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, TunnelError> {
        if data.len() < 4 {
            return Err(TunnelError::PacketTooShort(data.len()));
        }

        let flags_and_version = u16::from_be_bytes([data[0], data[1]]);
        let version = (flags_and_version & 0x0007) as u8;
        if version != 0 {
            return Err(TunnelError::InvalidVersion(version));
        }

        let checksum_present = (flags_and_version & GRE_FLAG_CHECKSUM) != 0;
        let key_present = (flags_and_version & GRE_FLAG_KEY) != 0;
        let sequence_present = (flags_and_version & GRE_FLAG_SEQUENCE) != 0;

        let protocol_type = u16::from_be_bytes([data[2], data[3]]);

        let mut offset = 4;
        let mut checksum = None;
        if checksum_present {
            if data.len() < offset + 4 {
                return Err(TunnelError::PacketTooShort(data.len()));
            }
            checksum = Some(u16::from_be_bytes([data[offset], data[offset + 1]]));
            offset += 4; // 2B Checksum + 2B Reserved
        }

        let mut key = None;
        if key_present {
            if data.len() < offset + 4 {
                return Err(TunnelError::PacketTooShort(data.len()));
            }
            key = Some(u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }

        let mut sequence_number = None;
        if sequence_present {
            if data.len() < offset + 4 {
                return Err(TunnelError::PacketTooShort(data.len()));
            }
            sequence_number = Some(u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }

        let payload = &data[offset..];

        Ok(GrePacket {
            header: GreHeader {
                checksum_present,
                key_present,
                sequence_present,
                version,
                protocol_type,
                checksum,
                key,
                sequence_number,
            },
            payload,
        })
    }

    pub fn serialize(
        protocol_type: u16,
        key: Option<u32>,
        sequence_number: Option<u32>,
        with_checksum: bool,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut flags = 0u16;
        if with_checksum {
            flags |= GRE_FLAG_CHECKSUM;
        }
        if key.is_some() {
            flags |= GRE_FLAG_KEY;
        }
        if sequence_number.is_some() {
            flags |= GRE_FLAG_SEQUENCE;
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(&flags.to_be_bytes());
        buf.extend_from_slice(&protocol_type.to_be_bytes());

        let checksum_offset = if with_checksum {
            let pos = buf.len();
            buf.extend_from_slice(&[0, 0, 0, 0]); // 2B Checksum + 2B Reserved
            Some(pos)
        } else {
            None
        };

        if let Some(k) = key {
            buf.extend_from_slice(&k.to_be_bytes());
        }

        if let Some(s) = sequence_number {
            buf.extend_from_slice(&s.to_be_bytes());
        }

        buf.extend_from_slice(payload);

        if let Some(pos) = checksum_offset {
            let csum = compute_checksum(&buf);
            buf[pos] = (csum >> 8) as u8;
            buf[pos + 1] = (csum & 0xFF) as u8;
        }

        buf
    }

    /// Encapsulates an inner IPv4 packet inside an outer GRE tunnel packet
    pub fn encapsulate_gre_ipv4(
        outer_src: Ipv4Address,
        outer_dst: Ipv4Address,
        inner_packet: &[u8],
        key: Option<u32>,
    ) -> Vec<u8> {
        let gre_raw = Self::serialize(ETHERTYPE_IPV4, key, None, false, inner_packet);
        Ipv4Packet::serialize(outer_src, outer_dst, IP_PROTO_GRE, 1, 64, &gre_raw)
    }

    /// Encapsulates an inner IPv6 packet inside an outer GRE tunnel packet
    pub fn encapsulate_gre_ipv6(
        outer_src: Ipv4Address,
        outer_dst: Ipv4Address,
        inner_packet: &[u8],
        key: Option<u32>,
    ) -> Vec<u8> {
        let gre_raw = Self::serialize(ETHERTYPE_IPV6, key, None, false, inner_packet);
        Ipv4Packet::serialize(outer_src, outer_dst, IP_PROTO_GRE, 1, 64, &gre_raw)
    }

    /// Encapsulates an inner IPv4 packet directly in an outer IPv4 packet (IP-in-IP, Protocol 4)
    pub fn encapsulate_ip_in_ip(
        outer_src: Ipv4Address,
        outer_dst: Ipv4Address,
        inner_packet: &[u8],
    ) -> Vec<u8> {
        Ipv4Packet::serialize(outer_src, outer_dst, IP_PROTO_IP_IN_IP, 1, 64, inner_packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gre_packet_serialize_and_parse() {
        let payload = b"Inner GRE payload data";
        let raw = GrePacket::serialize(ETHERTYPE_IPV4, Some(0x1337), Some(42), true, payload);

        let parsed = GrePacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.protocol_type, ETHERTYPE_IPV4);
        assert_eq!(parsed.header.key, Some(0x1337));
        assert_eq!(parsed.header.sequence_number, Some(42));
        assert!(parsed.header.checksum.is_some());
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_gre_and_ip_in_ip_encapsulation() {
        let outer_src = Ipv4Address::new(203, 0, 113, 1);
        let outer_dst = Ipv4Address::new(198, 51, 100, 1);
        let inner_data = b"Private LAN packet";

        let encapsulated = GrePacket::encapsulate_gre_ipv4(outer_src, outer_dst, inner_data, None);
        let outer_ip = Ipv4Packet::parse(&encapsulated, true).unwrap();
        assert_eq!(outer_ip.header.protocol.to_u8(), IP_PROTO_GRE);

        let gre = GrePacket::parse(outer_ip.payload).unwrap();
        assert_eq!(gre.header.protocol_type, ETHERTYPE_IPV4);
        assert_eq!(gre.payload, inner_data);
    }
}
