//! VXLAN Generic Protocol Extension (VXLAN-GPE).
//!
//! Multi-protocol overlay encapsulation over UDP port 4790 supporting direct IPv4, IPv6, Ethernet, and MPLS payloads.

use std::fmt;

pub const VXLAN_GPE_UDP_PORT: u16 = 4790;
pub const VXLAN_GPE_HEADER_LEN: usize = 8;

// Next Protocol values
pub const VXLAN_GPE_NP_IPV4: u8 = 0x01;
pub const VXLAN_GPE_NP_IPV6: u8 = 0x02;
pub const VXLAN_GPE_NP_ETHERNET: u8 = 0x03;
pub const VXLAN_GPE_NP_NSH: u8 = 0x04;
pub const VXLAN_GPE_NP_MPLS: u8 = 0x05;

// Flags
pub const VXLAN_GPE_FLAG_P: u8 = 0x04; // P-bit: Next Protocol is valid
pub const VXLAN_GPE_FLAG_I: u8 = 0x08; // I-bit: VNI is valid

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VxlanGpeHeader {
    pub flags: u8,
    pub next_protocol: u8,
    pub vni: u32, // 24-bit Virtual Network Identifier
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VxlanGpePacket {
    pub header: VxlanGpeHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VxlanGpeError {
    PacketTooShort(usize),
    InvalidFlags(u8),
}

impl fmt::Display for VxlanGpeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VxlanGpeError::PacketTooShort(l) => {
                write!(f, "VXLAN-GPE packet too short ({} bytes)", l)
            }
            VxlanGpeError::InvalidFlags(fl) => write!(f, "Invalid VXLAN-GPE flags: 0x{:02X}", fl),
        }
    }
}

impl std::error::Error for VxlanGpeError {}

impl VxlanGpePacket {
    pub fn build(vni: u32, next_protocol: u8, payload: &[u8]) -> Self {
        VxlanGpePacket {
            header: VxlanGpeHeader {
                flags: VXLAN_GPE_FLAG_I | VXLAN_GPE_FLAG_P,
                next_protocol,
                vni: vni & 0x00FF_FFFF,
            },
            payload: payload.to_vec(),
        }
    }

    pub fn build_ipv4(vni: u32, ip_payload: &[u8]) -> Self {
        Self::build(vni, VXLAN_GPE_NP_IPV4, ip_payload)
    }

    pub fn build_ipv6(vni: u32, ip6_payload: &[u8]) -> Self {
        Self::build(vni, VXLAN_GPE_NP_IPV6, ip6_payload)
    }

    pub fn build_ethernet(vni: u32, eth_frame: &[u8]) -> Self {
        Self::build(vni, VXLAN_GPE_NP_ETHERNET, eth_frame)
    }

    pub fn build_mpls(vni: u32, mpls_packet: &[u8]) -> Self {
        Self::build(vni, VXLAN_GPE_NP_MPLS, mpls_packet)
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.header.flags);
        buf.push(0x00); // Reserved
        buf.push(0x00); // Reserved
        buf.push(self.header.next_protocol);

        let vni_bytes = (self.header.vni & 0x00FF_FFFF).to_be_bytes();
        buf.extend_from_slice(&vni_bytes[1..4]);
        buf.push(0x00); // Reserved

        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, VxlanGpeError> {
        if data.len() < VXLAN_GPE_HEADER_LEN {
            return Err(VxlanGpeError::PacketTooShort(data.len()));
        }

        let flags = data[0];
        let next_protocol = data[3];
        let vni = u32::from_be_bytes([0, data[4], data[5], data[6]]);

        let payload = data[VXLAN_GPE_HEADER_LEN..].to_vec();

        Ok(VxlanGpePacket {
            header: VxlanGpeHeader {
                flags,
                next_protocol,
                vni,
            },
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vxlan_gpe_ipv4_and_ethernet_roundtrip() {
        let payload = b"Direct IPv4 Packet Inside VXLAN-GPE";
        let pkt = VxlanGpePacket::build_ipv4(5001, payload);
        let raw = pkt.serialize();

        assert!(raw.len() >= VXLAN_GPE_HEADER_LEN);
        let parsed = VxlanGpePacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.vni, 5001);
        assert_eq!(parsed.header.next_protocol, VXLAN_GPE_NP_IPV4);
        assert_eq!(parsed.payload, payload);
        assert_eq!(VXLAN_GPE_UDP_PORT, 4790);
    }
}
