//! Session Traversal Utilities for NAT (STUN - RFC 8489 / RFC 5389).
//!
//! NAT discovery and reflexive public address resolution over UDP port 3478.

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const STUN_PORT: u16 = 3478;
pub const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
pub const STUN_HEADER_LEN: usize = 20;

// STUN Message Types
pub const STUN_BINDING_REQUEST: u16 = 0x0001;
pub const STUN_BINDING_RESPONSE: u16 = 0x0101;
pub const STUN_BINDING_ERROR_RESPONSE: u16 = 0x0111;

// STUN Attribute Types
pub const STUN_ATTR_MAPPED_ADDRESS: u16 = 0x0001;
pub const STUN_ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
pub const STUN_ATTR_SOFTWARE: u16 = 0x8022;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StunAttribute {
    pub attr_type: u16,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StunPacket {
    pub msg_type: u16,
    pub magic_cookie: u32,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<StunAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StunError {
    PacketTooShort(usize),
    InvalidMagicCookie(u32),
    InvalidAttributeLength,
}

impl fmt::Display for StunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StunError::PacketTooShort(l) => write!(f, "STUN packet too short ({} bytes)", l),
            StunError::InvalidMagicCookie(c) => write!(f, "Invalid STUN Magic Cookie: 0x{:08X}", c),
            StunError::InvalidAttributeLength => write!(f, "Invalid STUN attribute TLV length"),
        }
    }
}

impl std::error::Error for StunError {}

impl StunPacket {
    pub fn build_binding_request(trans_id: [u8; 12]) -> Self {
        StunPacket {
            msg_type: STUN_BINDING_REQUEST,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id: trans_id,
            attributes: vec![StunAttribute {
                attr_type: STUN_ATTR_SOFTWARE,
                value: b"ToyNetStack STUN Client 1.0".to_vec(),
            }],
        }
    }

    pub fn build_binding_response(req: &StunPacket, reflexive_ip: Ipv4Address, reflexive_port: u16) -> Self {
        // XOR-MAPPED-ADDRESS computation (RFC 8489)
        // Family: 0x01 IPv4
        // X-Port: Port ^ (MagicCookie >> 16)
        // X-Address: IP ^ MagicCookie
        let x_port = reflexive_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
        let mc_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
        let x_ip = [
            reflexive_ip.0[0] ^ mc_bytes[0],
            reflexive_ip.0[1] ^ mc_bytes[1],
            reflexive_ip.0[2] ^ mc_bytes[2],
            reflexive_ip.0[3] ^ mc_bytes[3],
        ];

        let mut xor_val = Vec::new();
        xor_val.push(0x00); // Reserved
        xor_val.push(0x01); // IPv4
        xor_val.extend_from_slice(&x_port.to_be_bytes());
        xor_val.extend_from_slice(&x_ip);

        let attrs = vec![
            StunAttribute {
                attr_type: STUN_ATTR_XOR_MAPPED_ADDRESS,
                value: xor_val,
            },
            StunAttribute {
                attr_type: STUN_ATTR_SOFTWARE,
                value: b"ToyNetStack STUN Server".to_vec(),
            },
        ];

        StunPacket {
            msg_type: STUN_BINDING_RESPONSE,
            magic_cookie: req.magic_cookie,
            transaction_id: req.transaction_id,
            attributes: attrs,
        }
    }

    pub fn get_xor_mapped_address(&self) -> Option<(Ipv4Address, u16)> {
        for attr in &self.attributes {
            if attr.attr_type == STUN_ATTR_XOR_MAPPED_ADDRESS && attr.value.len() >= 8 {
                let family = attr.value[1];
                if family == 0x01 {
                    let x_port = u16::from_be_bytes([attr.value[2], attr.value[3]]);
                    let port = x_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

                    let mc_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
                    let ip = Ipv4Address([
                        attr.value[4] ^ mc_bytes[0],
                        attr.value[5] ^ mc_bytes[1],
                        attr.value[6] ^ mc_bytes[2],
                        attr.value[7] ^ mc_bytes[3],
                    ]);
                    return Some((ip, port));
                }
            }
        }
        None
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.msg_type.to_be_bytes());

        // Calculate attributes length
        let mut attr_len = 0usize;
        for a in &self.attributes {
            attr_len += 4 + ((a.value.len() + 3) & !3);
        }

        buf.extend_from_slice(&(attr_len as u16).to_be_bytes());
        buf.extend_from_slice(&self.magic_cookie.to_be_bytes());
        buf.extend_from_slice(&self.transaction_id);

        for a in &self.attributes {
            buf.extend_from_slice(&a.attr_type.to_be_bytes());
            buf.extend_from_slice(&(a.value.len() as u16).to_be_bytes());
            buf.extend_from_slice(&a.value);

            // 4-byte alignment padding
            let pad = (4 - (a.value.len() % 4)) % 4;
            for _ in 0..pad {
                buf.push(0x00);
            }
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, StunError> {
        if data.len() < STUN_HEADER_LEN {
            return Err(StunError::PacketTooShort(data.len()));
        }

        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        let magic_cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        if magic_cookie != STUN_MAGIC_COOKIE {
            return Err(StunError::InvalidMagicCookie(magic_cookie));
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        if data.len() < STUN_HEADER_LEN + msg_len {
            return Err(StunError::PacketTooShort(data.len()));
        }

        let mut offset = STUN_HEADER_LEN;
        let end = STUN_HEADER_LEN + msg_len;
        let mut attributes = Vec::new();

        while offset + 4 <= end {
            let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            if offset + 4 + attr_len > end {
                return Err(StunError::InvalidAttributeLength);
            }

            let value = data[offset + 4..offset + 4 + attr_len].to_vec();
            attributes.push(StunAttribute { attr_type, value });

            let padded_len = (attr_len + 3) & !3;
            offset += 4 + padded_len;
        }

        Ok(StunPacket {
            msg_type,
            magic_cookie,
            transaction_id,
            attributes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_binding_request_and_response_roundtrip() {
        let tid = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];
        let req = StunPacket::build_binding_request(tid);
        let raw_req = req.serialize();

        assert_eq!(raw_req.len() >= STUN_HEADER_LEN, true);
        let parsed_req = StunPacket::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.msg_type, STUN_BINDING_REQUEST);

        let public_ip = Ipv4Address::new(203, 0, 113, 50);
        let public_port = 54321;
        let resp = StunPacket::build_binding_response(&parsed_req, public_ip, public_port);
        let raw_resp = resp.serialize();

        let parsed_resp = StunPacket::parse(&raw_resp).unwrap();
        assert_eq!(parsed_resp.msg_type, STUN_BINDING_RESPONSE);

        let (mapped_ip, mapped_port) = parsed_resp.get_xor_mapped_address().unwrap();
        assert_eq!(mapped_ip, public_ip);
        assert_eq!(mapped_port, public_port);
    }
}
