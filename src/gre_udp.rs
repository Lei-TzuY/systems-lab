//! GRE-in-UDP Encapsulation (RFC 8086).
//!
//! Encapsulates Generic Routing Encapsulation (GRE) packets in UDP to enable ECMP flow hashing,
//! hardware hashing, and NAT traversal across datacenter IP fabrics (UDP Port 4754).

use crate::tunnel::{GreHeader, GrePacket};

pub const GRE_IN_UDP_PORT: u16 = 4754;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreUdpPacket<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub header: GreHeader,
    pub payload: &'a [u8],
}

impl<'a> GreUdpPacket<'a> {
    pub fn new(
        src_port: u16,
        protocol_type: u16,
        key: Option<u32>,
        seq_num: Option<u32>,
        payload: &'a [u8],
    ) -> Self {
        GreUdpPacket {
            src_port,
            dst_port: GRE_IN_UDP_PORT,
            header: GreHeader {
                checksum_present: false,
                key_present: key.is_some(),
                sequence_present: seq_num.is_some(),
                version: 0,
                protocol_type,
                checksum: None,
                key,
                sequence_number: seq_num,
            },
            payload,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        GrePacket::serialize(
            self.header.protocol_type,
            self.header.key,
            self.header.sequence_number,
            self.header.checksum_present,
            self.payload,
        )
    }

    pub fn parse(udp_src_port: u16, udp_dst_port: u16, data: &'a [u8]) -> Option<Self> {
        if udp_dst_port != GRE_IN_UDP_PORT {
            return None;
        }

        let gre_packet = GrePacket::parse(data).ok()?;
        Some(GreUdpPacket {
            src_port: udp_src_port,
            dst_port: udp_dst_port,
            header: gre_packet.header,
            payload: gre_packet.payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gre_udp_encapsulation_and_parse() {
        let payload = b"Enterprise Overlay Payload over GRE-in-UDP";
        let pkt = GreUdpPacket::new(51234, 0x0800, Some(0xA1B2C3D4), Some(42), payload);

        let raw = pkt.serialize();
        assert!(raw.len() >= 8 + payload.len());

        let parsed = GreUdpPacket::parse(51234, GRE_IN_UDP_PORT, &raw).unwrap();
        assert_eq!(parsed.src_port, 51234);
        assert_eq!(parsed.dst_port, 4754);
        assert_eq!(parsed.header.key, Some(0xA1B2C3D4));
        assert_eq!(parsed.header.sequence_number, Some(42));
        assert_eq!(parsed.payload, payload);
    }
}
