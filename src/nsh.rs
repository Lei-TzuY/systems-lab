//! Network Service Header (NSH - RFC 8300).
//!
//! Service Function Chaining (SFC) encapsulation for SDN networks, firewalls, and DPI service paths.

use std::fmt;

pub const NSH_BASE_HEADER_LEN: usize = 4;
pub const NSH_SERVICE_PATH_HEADER_LEN: usize = 4;
pub const NSH_MD_TYPE_1_CONTEXT_LEN: usize = 16;
pub const NSH_MD_TYPE_1_TOTAL_HEADER_LEN: usize = 24;

// Next Protocol values
pub const NSH_NP_IPV4: u8 = 0x01;
pub const NSH_NP_IPV6: u8 = 0x02;
pub const NSH_NP_ETHERNET: u8 = 0x03;
pub const NSH_NP_NSH: u8 = 0x04;
pub const NSH_NP_MPLS: u8 = 0x05;

// Metadata Types
pub const NSH_MD_TYPE_1: u8 = 0x01;
pub const NSH_MD_TYPE_2: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NshHeader {
    pub oam: bool,
    pub critical: bool,
    pub md_type: u8,
    pub next_protocol: u8,
    pub service_path_id: u32, // 24-bit SPI
    pub service_index: u8,    // 8-bit SI
    pub context_c1: u32,      // Network Platform Context
    pub context_c2: u32,      // Network Shared Context (e.g., VRF/Tenant)
    pub context_c3: u32,      // Service Platform Context
    pub context_c4: u32,      // Service Shared Context (Flow Hash)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NshPacket {
    pub header: NshHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NshError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidLength,
}

impl fmt::Display for NshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NshError::PacketTooShort(l) => write!(f, "NSH packet too short ({} bytes)", l),
            NshError::InvalidVersion(v) => write!(f, "Unsupported NSH version: {}", v),
            NshError::InvalidLength => write!(f, "Invalid NSH length"),
        }
    }
}

impl std::error::Error for NshError {}

impl NshHeader {
    pub fn new_type1(spi: u32, si: u8, next_proto: u8, tenant_id: u32, flow_hash: u32) -> Self {
        NshHeader {
            oam: false,
            critical: false,
            md_type: NSH_MD_TYPE_1,
            next_protocol: next_proto,
            service_path_id: spi & 0x00FF_FFFF,
            service_index: si,
            context_c1: 0x00000001, // Ingress Port #1
            context_c2: tenant_id,  // Tenant / VRF
            context_c3: 0,
            context_c4: flow_hash, // 5-tuple flow hash
        }
    }

    pub fn serialize(&self) -> [u8; 24] {
        let mut buf = [0u8; 24];
        let mut b0 = 0u8; // Version = 0
        if self.oam {
            b0 |= 0x20;
        }
        if self.critical {
            b0 |= 0x10;
        }
        buf[0] = b0;
        buf[1] = 6; // Length in 4-byte words (6 * 4 = 24 bytes)
        buf[2] = self.md_type;
        buf[3] = self.next_protocol;

        let spi_bytes = (self.service_path_id & 0x00FF_FFFF).to_be_bytes();
        buf[4..7].copy_from_slice(&spi_bytes[1..4]);
        buf[7] = self.service_index;

        buf[8..12].copy_from_slice(&self.context_c1.to_be_bytes());
        buf[12..16].copy_from_slice(&self.context_c2.to_be_bytes());
        buf[16..20].copy_from_slice(&self.context_c3.to_be_bytes());
        buf[20..24].copy_from_slice(&self.context_c4.to_be_bytes());

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, NshError> {
        if data.len() < NSH_MD_TYPE_1_TOTAL_HEADER_LEN {
            return Err(NshError::PacketTooShort(data.len()));
        }

        let version = (data[0] >> 6) & 0x03;
        if version != 0 {
            return Err(NshError::InvalidVersion(version));
        }

        let oam = (data[0] & 0x20) != 0;
        let critical = (data[0] & 0x10) != 0;
        let md_type = data[2];
        let next_protocol = data[3];

        let service_path_id = u32::from_be_bytes([0, data[4], data[5], data[6]]);
        let service_index = data[7];

        let context_c1 = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let context_c2 = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let context_c3 = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let context_c4 = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

        Ok(NshHeader {
            oam,
            critical,
            md_type,
            next_protocol,
            service_path_id,
            service_index,
            context_c1,
            context_c2,
            context_c3,
            context_c4,
        })
    }
}

impl NshPacket {
    pub fn build_ipv4(spi: u32, si: u8, tenant_id: u32, flow_hash: u32, ip_payload: &[u8]) -> Self {
        let header = NshHeader::new_type1(spi, si, NSH_NP_IPV4, tenant_id, flow_hash);
        NshPacket {
            header,
            payload: ip_payload.to_vec(),
        }
    }

    pub fn build_ethernet(
        spi: u32,
        si: u8,
        tenant_id: u32,
        flow_hash: u32,
        eth_frame: &[u8],
    ) -> Self {
        let header = NshHeader::new_type1(spi, si, NSH_NP_ETHERNET, tenant_id, flow_hash);
        NshPacket {
            header,
            payload: eth_frame.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.header.serialize());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, NshError> {
        if data.len() < NSH_MD_TYPE_1_TOTAL_HEADER_LEN {
            return Err(NshError::PacketTooShort(data.len()));
        }

        let header = NshHeader::parse(&data[..NSH_MD_TYPE_1_TOTAL_HEADER_LEN])?;
        let payload = data[NSH_MD_TYPE_1_TOTAL_HEADER_LEN..].to_vec();

        Ok(NshPacket { header, payload })
    }
}

/// Service Function Forwarder (SFF) step execution
#[derive(Debug, Clone, Default)]
pub struct ServiceFunctionForwarder;

impl ServiceFunctionForwarder {
    pub fn forward_next_service_hop(pkt: &mut NshPacket) -> bool {
        if pkt.header.service_index == 0 {
            return false; // Reached end of Service Function Path
        }
        pkt.header.service_index -= 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nsh_header_and_sff_service_hop() {
        let payload = b"Classified Payload traversing Firewall -> IPS -> DPI";
        let pkt = NshPacket::build_ipv4(0x00002A, 255, 1001, 0x12345678, payload);
        let raw = pkt.serialize();

        assert_eq!(raw.len(), NSH_MD_TYPE_1_TOTAL_HEADER_LEN + payload.len());
        let mut parsed = NshPacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.service_path_id, 42);
        assert_eq!(parsed.header.service_index, 255);
        assert_eq!(parsed.header.context_c2, 1001);
        assert_eq!(parsed.header.next_protocol, NSH_NP_IPV4);

        // Hop 1: Firewall
        let hop1 = ServiceFunctionForwarder::forward_next_service_hop(&mut parsed);
        assert!(hop1);
        assert_eq!(parsed.header.service_index, 254);

        // Hop 2: DPI
        let hop2 = ServiceFunctionForwarder::forward_next_service_hop(&mut parsed);
        assert!(hop2);
        assert_eq!(parsed.header.service_index, 253);
    }
}
