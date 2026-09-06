//! MPLS Label Distribution Protocol (LDP - RFC 5036).
//!
//! Dynamic MPLS label signaling and FEC mapping over UDP/TCP port 646.

use crate::ipv4::Ipv4Address;
use std::collections::BTreeMap;
use std::fmt;

pub const LDP_PORT: u16 = 646;
pub const LDP_HEADER_LEN: usize = 10;

// LDP Message Types
pub const LDP_MSG_NOTIFICATION: u16 = 0x0001;
pub const LDP_MSG_HELLO: u16 = 0x0100;
pub const LDP_MSG_INITIALIZATION: u16 = 0x0200;
pub const LDP_MSG_KEEPALIVE: u16 = 0x0201;
pub const LDP_MSG_LABEL_MAPPING: u16 = 0x0400;
pub const LDP_MSG_LABEL_REQUEST: u16 = 0x0401;
pub const LDP_MSG_LABEL_WITHDRAW: u16 = 0x0402;
pub const LDP_MSG_LABEL_RELEASE: u16 = 0x0403;

// LDP TLV Types
pub const LDP_TLV_FEC: u16 = 0x0100;
pub const LDP_TLV_GENERIC_LABEL: u16 = 0x0200;
pub const LDP_TLV_COMMON_HELLO: u16 = 0x0400;
pub const LDP_TLV_IPV4_TRANSPORT_ADDRESS: u16 = 0x0401;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdpTlv {
    pub tlv_type: u16,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdpMessage {
    pub msg_type: u16,
    pub msg_id: u32,
    pub tlvs: Vec<LdpTlv>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdpPdu {
    pub version: u16,
    pub lsr_id: Ipv4Address,
    pub label_space: u16,
    pub messages: Vec<LdpMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdpBinding {
    pub prefix: Ipv4Address,
    pub prefix_len: u8,
    pub label: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LdpSession {
    pub peer_lsr_id: Ipv4Address,
    pub learned_bindings: BTreeMap<(Ipv4Address, u8), u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdpError {
    PacketTooShort(usize),
    InvalidVersion(u16),
    InvalidLength,
}

impl fmt::Display for LdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LdpError::PacketTooShort(l) => write!(f, "LDP packet too short ({} bytes)", l),
            LdpError::InvalidVersion(v) => write!(f, "Invalid LDP version: {}", v),
            LdpError::InvalidLength => write!(f, "Invalid LDP message or TLV length"),
        }
    }
}

impl std::error::Error for LdpError {}

impl LdpPdu {
    pub fn build_hello(lsr_id: Ipv4Address, holdtime: u16) -> Self {
        let mut tlvs = Vec::new();

        // 1. Common Hello Parameters TLV (Holdtime 2B + Flags 2B)
        let mut hello_val = Vec::new();
        hello_val.extend_from_slice(&holdtime.to_be_bytes());
        hello_val.extend_from_slice(&0u16.to_be_bytes()); // Targeted=0
        tlvs.push(LdpTlv {
            tlv_type: LDP_TLV_COMMON_HELLO,
            value: hello_val,
        });

        // 2. IPv4 Transport Address TLV
        tlvs.push(LdpTlv {
            tlv_type: LDP_TLV_IPV4_TRANSPORT_ADDRESS,
            value: lsr_id.0.to_vec(),
        });

        let msg = LdpMessage {
            msg_type: LDP_MSG_HELLO,
            msg_id: 1,
            tlvs,
        };

        LdpPdu {
            version: 1,
            lsr_id,
            label_space: 0,
            messages: vec![msg],
        }
    }

    pub fn build_label_mapping(
        lsr_id: Ipv4Address,
        msg_id: u32,
        prefix: Ipv4Address,
        prefix_len: u8,
        label: u32,
    ) -> Self {
        let mut tlvs = Vec::new();

        // 1. FEC TLV: Prefix FEC Element (Type 2 = Prefix FEC, AF 1 = IPv4, PreLen, PreBytes)
        let mut fec_val = Vec::new();
        fec_val.push(0x02); // Prefix FEC Element
        fec_val.extend_from_slice(&1u16.to_be_bytes()); // Address Family: IPv4 (1)
        fec_val.push(prefix_len);
        let prefix_bytes_len = (prefix_len as usize).div_ceil(8);
        fec_val.extend_from_slice(&prefix.0[..prefix_bytes_len.min(4)]);
        tlvs.push(LdpTlv {
            tlv_type: LDP_TLV_FEC,
            value: fec_val,
        });

        // 2. Generic Label TLV (20-bit label)
        let mut label_val = Vec::new();
        label_val.extend_from_slice(&(label & 0x000F_FFFF).to_be_bytes());
        tlvs.push(LdpTlv {
            tlv_type: LDP_TLV_GENERIC_LABEL,
            value: label_val,
        });

        let msg = LdpMessage {
            msg_type: LDP_MSG_LABEL_MAPPING,
            msg_id,
            tlvs,
        };

        LdpPdu {
            version: 1,
            lsr_id,
            label_space: 0,
            messages: vec![msg],
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut msg_bytes = Vec::new();
        for msg in &self.messages {
            let mut tlv_bytes = Vec::new();
            for tlv in &msg.tlvs {
                tlv_bytes.extend_from_slice(&tlv.tlv_type.to_be_bytes());
                tlv_bytes.extend_from_slice(&(tlv.value.len() as u16).to_be_bytes());
                tlv_bytes.extend_from_slice(&tlv.value);
            }

            msg_bytes.extend_from_slice(&msg.msg_type.to_be_bytes());
            let msg_len = (4 + tlv_bytes.len()) as u16;
            msg_bytes.extend_from_slice(&msg_len.to_be_bytes());
            msg_bytes.extend_from_slice(&msg.msg_id.to_be_bytes());
            msg_bytes.extend_from_slice(&tlv_bytes);
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(&self.version.to_be_bytes());
        let pdu_len = (6 + msg_bytes.len()) as u16;
        buf.extend_from_slice(&pdu_len.to_be_bytes());
        buf.extend_from_slice(&self.lsr_id.0);
        buf.extend_from_slice(&self.label_space.to_be_bytes());
        buf.extend_from_slice(&msg_bytes);

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, LdpError> {
        if data.len() < LDP_HEADER_LEN {
            return Err(LdpError::PacketTooShort(data.len()));
        }

        let version = u16::from_be_bytes([data[0], data[1]]);
        if version != 1 {
            return Err(LdpError::InvalidVersion(version));
        }

        let pdu_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < 4 + pdu_len {
            return Err(LdpError::PacketTooShort(data.len()));
        }

        let lsr_id = Ipv4Address([data[4], data[5], data[6], data[7]]);
        let label_space = u16::from_be_bytes([data[8], data[9]]);

        let mut messages = Vec::new();
        let mut offset = 10;
        let end = 4 + pdu_len;

        while offset + 8 <= end {
            let msg_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let msg_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            let msg_id = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            if offset + 4 + msg_len > end {
                return Err(LdpError::InvalidLength);
            }

            let mut tlvs = Vec::new();
            let mut tlv_offset = offset + 8;
            let msg_end = offset + 4 + msg_len;

            while tlv_offset + 4 <= msg_end {
                let tlv_type = u16::from_be_bytes([data[tlv_offset], data[tlv_offset + 1]]);
                let tlv_len =
                    u16::from_be_bytes([data[tlv_offset + 2], data[tlv_offset + 3]]) as usize;

                if tlv_offset + 4 + tlv_len > msg_end {
                    return Err(LdpError::InvalidLength);
                }

                let value = data[tlv_offset + 4..tlv_offset + 4 + tlv_len].to_vec();
                tlvs.push(LdpTlv { tlv_type, value });
                tlv_offset += 4 + tlv_len;
            }

            messages.push(LdpMessage {
                msg_type,
                msg_id,
                tlvs,
            });

            offset = msg_end;
        }

        Ok(LdpPdu {
            version,
            lsr_id,
            label_space,
            messages,
        })
    }

    pub fn extract_bindings(&self) -> Vec<LdpBinding> {
        let mut bindings = Vec::new();
        for msg in &self.messages {
            if msg.msg_type == LDP_MSG_LABEL_MAPPING {
                let mut prefix_opt = None;
                let mut prefix_len_opt = None;
                let mut label_opt = None;

                for tlv in &msg.tlvs {
                    if tlv.tlv_type == LDP_TLV_FEC && tlv.value.len() >= 4 {
                        let elem_type = tlv.value[0];
                        if elem_type == 0x02 {
                            let plen = tlv.value[3];
                            prefix_len_opt = Some(plen);
                            let mut p_bytes = [0u8; 4];
                            let copy_len = (tlv.value.len() - 4).min(4);
                            p_bytes[..copy_len].copy_from_slice(&tlv.value[4..4 + copy_len]);
                            prefix_opt = Some(Ipv4Address(p_bytes));
                        }
                    } else if tlv.tlv_type == LDP_TLV_GENERIC_LABEL && tlv.value.len() >= 4 {
                        let lbl = u32::from_be_bytes([
                            tlv.value[0],
                            tlv.value[1],
                            tlv.value[2],
                            tlv.value[3],
                        ]);
                        label_opt = Some(lbl & 0x000F_FFFF);
                    }
                }

                if let (Some(p), Some(plen), Some(l)) = (prefix_opt, prefix_len_opt, label_opt) {
                    bindings.push(LdpBinding {
                        prefix: p,
                        prefix_len: plen,
                        label: l,
                    });
                }
            }
        }
        bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldp_hello_pdu_roundtrip() {
        let lsr = Ipv4Address::new(10, 0, 0, 1);
        let pdu = LdpPdu::build_hello(lsr, 15);
        let raw = pdu.serialize();

        assert!(raw.len() >= LDP_HEADER_LEN);
        let parsed = LdpPdu::parse(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.lsr_id, lsr);
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].msg_type, LDP_MSG_HELLO);
    }

    #[test]
    fn test_ldp_label_mapping_and_binding_extraction() {
        let lsr = Ipv4Address::new(10, 0, 0, 1);
        let pdu =
            LdpPdu::build_label_mapping(lsr, 101, Ipv4Address::new(192, 168, 10, 0), 24, 1001);
        let raw = pdu.serialize();

        let parsed = LdpPdu::parse(&raw).unwrap();
        assert_eq!(parsed.messages[0].msg_type, LDP_MSG_LABEL_MAPPING);

        let bindings = parsed.extract_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].prefix, Ipv4Address::new(192, 168, 10, 0));
        assert_eq!(bindings[0].prefix_len, 24);
        assert_eq!(bindings[0].label, 1001);
    }
}
