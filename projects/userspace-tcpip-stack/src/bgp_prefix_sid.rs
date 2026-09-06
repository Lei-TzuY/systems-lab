//! BGP Prefix-SID Attribute for Segment Routing (RFC 8669).
//!
//! Implements BGP Path Attribute 40 (BGP Prefix-SID) for advertising SR-MPLS Label Indices,
//! Originator SRGB ranges, and SRv6 Service SIDs in BGP-4 / MP-BGP UPDATE messages.

pub const BGP_ATTR_PREFIX_SID: u8 = 40;

pub const BGP_PREFIX_SID_TLV_LABEL_INDEX: u8 = 1;
pub const BGP_PREFIX_SID_TLV_IPV6_NODE_SID: u8 = 2;
pub const BGP_PREFIX_SID_TLV_ORIGINATOR_SRGB: u8 = 3;

/// BGP Prefix-SID Label-Index TLV (RFC 8669 Section 3.1)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelIndexTlv {
    pub flags: u8,
    pub label_index: u32,
}

impl LabelIndexTlv {
    pub fn new(label_index: u32) -> Self {
        LabelIndexTlv {
            flags: 0,
            label_index,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(7);
        buf.push(BGP_PREFIX_SID_TLV_LABEL_INDEX);
        buf.extend_from_slice(&7u16.to_be_bytes()); // Length of value
        buf.extend_from_slice(&0u16.to_be_bytes()); // Reserved
        buf.push(self.flags);
        buf.extend_from_slice(&self.label_index.to_be_bytes());
        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 10 {
            return None;
        }
        if buf[0] != BGP_PREFIX_SID_TLV_LABEL_INDEX {
            return None;
        }
        let flags = buf[5];
        let label_index = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
        Some(LabelIndexTlv { flags, label_index })
    }
}

/// BGP Originator SRGB TLV (RFC 8669 Section 3.3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginatorSrgbTlv {
    pub flags: u8,
    pub srgb_base: u32,
    pub srgb_range: u32,
}

impl OriginatorSrgbTlv {
    pub fn new(srgb_base: u32, srgb_range: u32) -> Self {
        OriginatorSrgbTlv {
            flags: 0,
            srgb_base,
            srgb_range,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3 + 2 + 1 + 3 + 3);
        buf.push(BGP_PREFIX_SID_TLV_ORIGINATOR_SRGB);
        let len: u16 = 2 + 1 + 3 + 3; // 9 bytes value
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes()); // Reserved
        buf.push(self.flags);

        // 3-byte base
        let base_bytes = self.srgb_base.to_be_bytes();
        buf.extend_from_slice(&base_bytes[1..4]);

        // 3-byte range
        let range_bytes = self.srgb_range.to_be_bytes();
        buf.extend_from_slice(&range_bytes[1..4]);

        buf
    }

    pub fn parse(buf: &[u8]) -> Option<Self> {
        if buf.len() < 12 {
            return None;
        }
        if buf[0] != BGP_PREFIX_SID_TLV_ORIGINATOR_SRGB {
            return None;
        }
        let flags = buf[5];
        let srgb_base = u32::from_be_bytes([0, buf[6], buf[7], buf[8]]);
        let srgb_range = u32::from_be_bytes([0, buf[9], buf[10], buf[11]]);

        Some(OriginatorSrgbTlv {
            flags,
            srgb_base,
            srgb_range,
        })
    }
}

/// BGP Prefix-SID Attribute Container
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BgpPrefixSidAttribute {
    pub label_index_tlv: Option<LabelIndexTlv>,
    pub srgb_tlv: Option<OriginatorSrgbTlv>,
}

impl BgpPrefixSidAttribute {
    pub fn new(label_index: Option<u32>, srgb_base: Option<u32>, srgb_range: Option<u32>) -> Self {
        let label_index_tlv = label_index.map(LabelIndexTlv::new);
        let srgb_tlv = match (srgb_base, srgb_range) {
            (Some(base), Some(range)) => Some(OriginatorSrgbTlv::new(base, range)),
            _ => None,
        };

        BgpPrefixSidAttribute {
            label_index_tlv,
            srgb_tlv,
        }
    }

    /// Calculates absolute MPLS label for a given SRGB base
    pub fn calculate_absolute_label(&self, local_srgb_base: u32) -> Option<u32> {
        self.label_index_tlv
            .as_ref()
            .map(|li| local_srgb_base + li.label_index)
    }

    /// Serializes BGP Prefix-SID Attribute value
    pub fn serialize_value(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        if let Some(ref li) = self.label_index_tlv {
            buf.extend_from_slice(&li.serialize());
        }
        if let Some(ref srgb) = self.srgb_tlv {
            buf.extend_from_slice(&srgb.serialize());
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgp_prefix_sid_label_index_codec() {
        let li = LabelIndexTlv::new(100);
        let bytes = li.serialize();
        assert_eq!(bytes[0], BGP_PREFIX_SID_TLV_LABEL_INDEX);

        let parsed = LabelIndexTlv::parse(&bytes).unwrap();
        assert_eq!(parsed, li);
    }

    #[test]
    fn test_bgp_prefix_sid_srgb_codec_and_label_calculation() {
        let attr = BgpPrefixSidAttribute::new(Some(50), Some(16000), Some(8000));
        let abs_label = attr.calculate_absolute_label(16000).unwrap();
        assert_eq!(abs_label, 16050);

        let serialized = attr.serialize_value();
        assert!(!serialized.is_empty());
    }
}
