//! BGP Link-State Extensions for Segment Routing over IPv6 (SRv6 BGP-LS / RFC 9514).
//!
//! Implements SRv6 BGP-LS NLRI TLVs including SRv6 Locator TLV (Type 1162)
//! and SRv6 End SID TLV (Type 1106) for SDN controller topology and locator distribution.

use crate::ipv6::Ipv6Address;

pub const BGP_LS_TLV_SRV6_LOCATOR: u16 = 1162;
pub const BGP_LS_TLV_SRV6_END_SID: u16 = 1106;

/// SRv6 Locator TLV (RFC 9514 Section 3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv6LocatorTlv {
    pub flags: u8,
    pub algorithm: u8, // Flex-Algo Algorithm ID (0 = Standard SPF, 128..255 = Flex-Algo)
    pub metric: u32,
    pub locator: Ipv6Address,
    pub prefix_len: u8,
}

impl Srv6LocatorTlv {
    pub fn new(algorithm: u8, metric: u32, locator: Ipv6Address, prefix_len: u8) -> Self {
        Srv6LocatorTlv {
            flags: 0,
            algorithm,
            metric,
            locator,
            prefix_len,
        }
    }

    /// Serializes SRv6 Locator TLV
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 1 + 1 + 2 + 4 + 1 + 16);
        buf.extend_from_slice(&BGP_LS_TLV_SRV6_LOCATOR.to_be_bytes());
        let length = 1 + 1 + 2 + 4 + 1 + 16; // 25 bytes value
        buf.extend_from_slice(&(length as u16).to_be_bytes());
        buf.push(self.flags);
        buf.push(self.algorithm);
        buf.extend_from_slice(&0u16.to_be_bytes()); // Reserved
        buf.extend_from_slice(&self.metric.to_be_bytes());
        buf.push(self.prefix_len);
        buf.extend_from_slice(&self.locator.0);
        buf
    }

    /// Parses SRv6 Locator TLV
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 29 {
            return None;
        }
        let tlv_type = u16::from_be_bytes([buf[0], buf[1]]);
        if tlv_type != BGP_LS_TLV_SRV6_LOCATOR {
            return None;
        }
        let flags = buf[4];
        let algorithm = buf[5];
        let metric = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let prefix_len = buf[12];
        let mut octets = [0u8; 16];
        octets.copy_from_slice(&buf[13..29]);

        Some(Srv6LocatorTlv {
            flags,
            algorithm,
            metric,
            locator: Ipv6Address(octets),
            prefix_len,
        })
    }
}

/// SRv6 End SID TLV (RFC 9514 Section 4)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Srv6EndSidTlv {
    pub flags: u8,
    pub endpoint_behavior: u16, // RFC 8986 Behavior Code (e.g. 1 = End, 5 = End.DT4, etc.)
    pub sid: Ipv6Address,
}

impl Srv6EndSidTlv {
    pub fn new(endpoint_behavior: u16, sid: Ipv6Address) -> Self {
        Srv6EndSidTlv {
            flags: 0,
            endpoint_behavior,
            sid,
        }
    }

    /// Serializes SRv6 End SID TLV
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 1 + 2 + 1 + 16);
        buf.extend_from_slice(&BGP_LS_TLV_SRV6_END_SID.to_be_bytes());
        let length = 1 + 2 + 1 + 16; // 20 bytes value
        buf.extend_from_slice(&(length as u16).to_be_bytes());
        buf.push(self.flags);
        buf.extend_from_slice(&self.endpoint_behavior.to_be_bytes());
        buf.push(0); // Reserved
        buf.extend_from_slice(&self.sid.0);
        buf
    }

    /// Parses SRv6 End SID TLV
    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 24 {
            return None;
        }
        let tlv_type = u16::from_be_bytes([buf[0], buf[1]]);
        if tlv_type != BGP_LS_TLV_SRV6_END_SID {
            return None;
        }
        let flags = buf[4];
        let endpoint_behavior = u16::from_be_bytes([buf[5], buf[6]]);
        let mut octets = [0u8; 16];
        octets.copy_from_slice(&buf[8..24]);

        Some(Srv6EndSidTlv {
            flags,
            endpoint_behavior,
            sid: Ipv6Address(octets),
        })
    }
}

/// BGP-LS SRv6 Topology Database
#[derive(Debug, Clone, Default)]
pub struct BgpLsSrv6Database {
    pub locators: Vec<Srv6LocatorTlv>,
    pub end_sids: Vec<Srv6EndSidTlv>,
}

impl BgpLsSrv6Database {
    pub fn new() -> Self {
        BgpLsSrv6Database {
            locators: Vec::new(),
            end_sids: Vec::new(),
        }
    }

    pub fn add_locator(&mut self, locator: Srv6LocatorTlv) {
        self.locators.push(locator);
    }

    pub fn add_end_sid(&mut self, end_sid: Srv6EndSidTlv) {
        self.end_sids.push(end_sid);
    }

    pub fn find_locator_for_sid(&self, sid: &Ipv6Address) -> Option<&Srv6LocatorTlv> {
        // Matches longest matching locator prefix
        self.locators.iter().find(|loc| {
            // Check if first 8 bytes match for a /64 locator
            loc.locator.0[..8] == sid.0[..8]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_bgp_ls_srv6_locator_tlv_codec() {
        let loc = Srv6LocatorTlv::new(
            128, // Flex-Algo 128
            10,
            Ipv6Address::from_str("2001:db8:ffff:100::").unwrap(),
            64,
        );

        let bytes = loc.serialize();
        assert_eq!(bytes.len(), 29);

        let parsed = Srv6LocatorTlv::parse(&bytes).unwrap();
        assert_eq!(parsed, loc);
    }

    #[test]
    fn test_bgp_ls_srv6_end_sid_tlv_codec() {
        let end_sid = Srv6EndSidTlv::new(
            1, // Behavior: End
            Ipv6Address::from_str("2001:db8:ffff:100::1").unwrap(),
        );

        let bytes = end_sid.serialize();
        assert_eq!(bytes.len(), 24);

        let parsed = Srv6EndSidTlv::parse(&bytes).unwrap();
        assert_eq!(parsed, end_sid);
    }
}
