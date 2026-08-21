//! Layer 2: IEEE 802.1Q Virtual LAN (VLAN) Tagging & Trunking.
//!
//! Handles 4-byte 802.1Q Tag Control Information (TCI) encapsulation, 12-bit VLAN IDs (1..4094),
//! 3-bit Priority Code Point (PCP), and frame tagging / stripping.

use crate::ethernet::{ETHER_HEADER_LEN, EtherType, MacAddress};
use std::fmt;

pub const TPID_8021Q: u16 = 0x8100;
pub const VLAN_HEADER_LEN: usize = 18; // 6 dst + 6 src + 2 TPID + 2 TCI + 2 EtherType

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlanTag {
    pub pcp: u8,   // Priority Code Point (3 bits: 0..7)
    pub dei: bool, // Drop Eligible Indicator (1 bit)
    pub vid: u16,  // VLAN ID (12 bits: 1..4094)
}

impl VlanTag {
    pub fn new(vid: u16, pcp: u8) -> Self {
        VlanTag {
            pcp: pcp & 0x07,
            dei: false,
            vid: vid & 0x0FFF,
        }
    }

    pub fn to_tci(&self) -> u16 {
        let mut tci = (self.vid & 0x0FFF) | (((self.pcp & 0x07) as u16) << 13);
        if self.dei {
            tci |= 0x1000;
        }
        tci
    }

    pub fn from_tci(tci: u16) -> Self {
        VlanTag {
            pcp: ((tci >> 13) & 0x07) as u8,
            dei: (tci & 0x1000) != 0,
            vid: tci & 0x0FFF,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedEthernetFrame<'a> {
    pub dst_mac: MacAddress,
    pub src_mac: MacAddress,
    pub vlan: VlanTag,
    pub inner_ethertype: EtherType,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VlanError {
    FrameTooShort(usize),
    InvalidTpid(u16),
}

impl fmt::Display for VlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VlanError::FrameTooShort(len) => {
                write!(f, "VLAN frame too short ({} bytes, min 18)", len)
            }
            VlanError::InvalidTpid(tpid) => write!(
                f,
                "Invalid VLAN TPID: expected 0x8100, found 0x{:04x}",
                tpid
            ),
        }
    }
}

impl std::error::Error for VlanError {}

impl<'a> TaggedEthernetFrame<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, VlanError> {
        if data.len() < VLAN_HEADER_LEN {
            return Err(VlanError::FrameTooShort(data.len()));
        }

        let mut dst_bytes = [0u8; 6];
        dst_bytes.copy_from_slice(&data[0..6]);
        let dst_mac = MacAddress(dst_bytes);

        let mut src_bytes = [0u8; 6];
        src_bytes.copy_from_slice(&data[6..12]);
        let src_mac = MacAddress(src_bytes);

        let tpid = u16::from_be_bytes([data[12], data[13]]);
        if tpid != TPID_8021Q {
            return Err(VlanError::InvalidTpid(tpid));
        }

        let tci = u16::from_be_bytes([data[14], data[15]]);
        let vlan = VlanTag::from_tci(tci);

        let raw_ethertype = u16::from_be_bytes([data[16], data[17]]);
        let inner_ethertype = EtherType::from_u16(raw_ethertype);

        let payload = &data[VLAN_HEADER_LEN..];

        Ok(TaggedEthernetFrame {
            dst_mac,
            src_mac,
            vlan,
            inner_ethertype,
            payload,
        })
    }

    pub fn serialize(
        dst_mac: MacAddress,
        src_mac: MacAddress,
        vlan: VlanTag,
        inner_ethertype: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(VLAN_HEADER_LEN + payload.len());
        buf.extend_from_slice(&dst_mac.0);
        buf.extend_from_slice(&src_mac.0);
        buf.extend_from_slice(&TPID_8021Q.to_be_bytes());
        buf.extend_from_slice(&vlan.to_tci().to_be_bytes());
        buf.extend_from_slice(&inner_ethertype.to_be_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Strips the 802.1Q header and returns a standard untagged 14-byte Ethernet II frame.
    pub fn strip_vlan(data: &'a [u8]) -> Result<Vec<u8>, VlanError> {
        let tagged = Self::parse(data)?;
        let mut untagged = Vec::with_capacity(ETHER_HEADER_LEN + tagged.payload.len());
        untagged.extend_from_slice(&tagged.dst_mac.0);
        untagged.extend_from_slice(&tagged.src_mac.0);
        untagged.extend_from_slice(&tagged.inner_ethertype.to_u16().to_be_bytes());
        untagged.extend_from_slice(tagged.payload);
        Ok(untagged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethernet::ETHERTYPE_IPV4;

    #[test]
    fn test_vlan_tag_tci_roundtrip() {
        let tag = VlanTag::new(100, 5);
        let tci = tag.to_tci();
        let parsed = VlanTag::from_tci(tci);

        assert_eq!(parsed.vid, 100);
        assert_eq!(parsed.pcp, 5);
        assert!(!parsed.dei);
    }

    #[test]
    fn test_tagged_frame_serialize_parse_strip() {
        let dst = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let src = MacAddress([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        let vlan = VlanTag::new(20, 3);
        let payload = b"VLAN Encapsulated Data";

        let raw = TaggedEthernetFrame::serialize(dst, src, vlan, ETHERTYPE_IPV4, payload);
        let parsed = TaggedEthernetFrame::parse(&raw).unwrap();

        assert_eq!(parsed.dst_mac, dst);
        assert_eq!(parsed.src_mac, src);
        assert_eq!(parsed.vlan.vid, 20);
        assert_eq!(parsed.vlan.pcp, 3);
        assert_eq!(parsed.inner_ethertype, EtherType::IPv4);
        assert_eq!(parsed.payload, payload);

        let stripped = TaggedEthernetFrame::strip_vlan(&raw).unwrap();
        assert_eq!(stripped.len(), ETHER_HEADER_LEN + payload.len());
    }
}
