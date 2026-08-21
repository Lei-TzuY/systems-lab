//! Traversal Using Relays around NAT (TURN - RFC 5766 / RFC 8656).
//!
//! Relay protocol extending STUN for media and data relaying behind Symmetric NATs.

use crate::ipv4::Ipv4Address;
use crate::stun::{StunAttribute, StunError, STUN_HEADER_LEN, STUN_MAGIC_COOKIE};
use std::collections::BTreeMap;

pub const TURN_ALLOCATE_REQUEST: u16 = 0x0003;
pub const TURN_ALLOCATE_RESPONSE: u16 = 0x0103;
pub const TURN_CREATE_PERMISSION_REQUEST: u16 = 0x0008;
pub const TURN_CREATE_PERMISSION_RESPONSE: u16 = 0x0108;
pub const TURN_SEND_INDICATION: u16 = 0x0016;
pub const TURN_DATA_INDICATION: u16 = 0x0017;
pub const TURN_CHANNEL_BIND_REQUEST: u16 = 0x0009;
pub const TURN_CHANNEL_BIND_RESPONSE: u16 = 0x0109;

// TURN Specific Attributes
pub const TURN_ATTR_CHANNEL_NUMBER: u16 = 0x000C;
pub const TURN_ATTR_LIFETIME: u16 = 0x000D;
pub const TURN_ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
pub const TURN_ATTR_DATA: u16 = 0x0013;
pub const TURN_ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
pub const TURN_ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;
pub const TURN_ATTR_SOFTWARE: u16 = 0x8022;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnPacket {
    pub msg_type: u16,
    pub magic_cookie: u32,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<StunAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnAllocation {
    pub client_ip: Ipv4Address,
    pub client_port: u16,
    pub relayed_ip: Ipv4Address,
    pub relayed_port: u16,
    pub lifetime_sec: u32,
}

#[derive(Debug, Clone, Default)]
pub struct TurnAllocationTable {
    pub allocations: BTreeMap<(Ipv4Address, u16), TurnAllocation>,
}

fn encode_xor_address(ip: Ipv4Address, port: u16) -> Vec<u8> {
    let x_port = port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
    let mc_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
    let x_ip = [
        ip.0[0] ^ mc_bytes[0],
        ip.0[1] ^ mc_bytes[1],
        ip.0[2] ^ mc_bytes[2],
        ip.0[3] ^ mc_bytes[3],
    ];

    let mut buf = Vec::new();
    buf.push(0x00); // Reserved
    buf.push(0x01); // IPv4
    buf.extend_from_slice(&x_port.to_be_bytes());
    buf.extend_from_slice(&x_ip);
    buf
}

fn decode_xor_address(val: &[u8]) -> Option<(Ipv4Address, u16)> {
    if val.len() >= 8 && val[1] == 0x01 {
        let x_port = u16::from_be_bytes([val[2], val[3]]);
        let port = x_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

        let mc_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
        let ip = Ipv4Address([
            val[4] ^ mc_bytes[0],
            val[5] ^ mc_bytes[1],
            val[6] ^ mc_bytes[2],
            val[7] ^ mc_bytes[3],
        ]);
        Some((ip, port))
    } else {
        None
    }
}

impl TurnPacket {
    pub fn build_allocate_request(trans_id: [u8; 12], lifetime_sec: u32) -> Self {
        let mut attrs = Vec::new();

        // Requested transport: UDP (Protocol 17) -> 1 byte proto (17) + 3 bytes reserved
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_REQUESTED_TRANSPORT,
            value: vec![17, 0, 0, 0],
        });

        // Lifetime
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_LIFETIME,
            value: lifetime_sec.to_be_bytes().to_vec(),
        });

        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_SOFTWARE,
            value: b"ToyNetStack TURN Client 1.0".to_vec(),
        });

        TurnPacket {
            msg_type: TURN_ALLOCATE_REQUEST,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id: trans_id,
            attributes: attrs,
        }
    }

    pub fn build_allocate_response(
        req: &TurnPacket,
        relayed_ip: Ipv4Address,
        relayed_port: u16,
        lifetime_sec: u32,
    ) -> Self {
        let mut attrs = Vec::new();

        // XOR-RELAYED-ADDRESS
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_XOR_RELAYED_ADDRESS,
            value: encode_xor_address(relayed_ip, relayed_port),
        });

        // Lifetime
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_LIFETIME,
            value: lifetime_sec.to_be_bytes().to_vec(),
        });

        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_SOFTWARE,
            value: b"ToyNetStack TURN Server".to_vec(),
        });

        TurnPacket {
            msg_type: TURN_ALLOCATE_RESPONSE,
            magic_cookie: req.magic_cookie,
            transaction_id: req.transaction_id,
            attributes: attrs,
        }
    }

    pub fn build_send_indication(peer_ip: Ipv4Address, peer_port: u16, data: &[u8]) -> Self {
        let mut attrs = Vec::new();

        // XOR-PEER-ADDRESS
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_XOR_PEER_ADDRESS,
            value: encode_xor_address(peer_ip, peer_port),
        });

        // DATA
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_DATA,
            value: data.to_vec(),
        });

        TurnPacket {
            msg_type: TURN_SEND_INDICATION,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id: [0u8; 12],
            attributes: attrs,
        }
    }

    pub fn build_data_indication(peer_ip: Ipv4Address, peer_port: u16, data: &[u8]) -> Self {
        let mut attrs = Vec::new();

        // XOR-PEER-ADDRESS
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_XOR_PEER_ADDRESS,
            value: encode_xor_address(peer_ip, peer_port),
        });

        // DATA
        attrs.push(StunAttribute {
            attr_type: TURN_ATTR_DATA,
            value: data.to_vec(),
        });

        TurnPacket {
            msg_type: TURN_DATA_INDICATION,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id: [0u8; 12],
            attributes: attrs,
        }
    }

    pub fn get_xor_relayed_address(&self) -> Option<(Ipv4Address, u16)> {
        for attr in &self.attributes {
            if attr.attr_type == TURN_ATTR_XOR_RELAYED_ADDRESS {
                return decode_xor_address(&attr.value);
            }
        }
        None
    }

    pub fn get_xor_peer_address(&self) -> Option<(Ipv4Address, u16)> {
        for attr in &self.attributes {
            if attr.attr_type == TURN_ATTR_XOR_PEER_ADDRESS {
                return decode_xor_address(&attr.value);
            }
        }
        None
    }

    pub fn get_data_payload(&self) -> Option<&[u8]> {
        for attr in &self.attributes {
            if attr.attr_type == TURN_ATTR_DATA {
                return Some(&attr.value);
            }
        }
        None
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.msg_type.to_be_bytes());

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

        Ok(TurnPacket {
            msg_type,
            magic_cookie,
            transaction_id,
            attributes,
        })
    }
}

impl TurnAllocationTable {
    pub fn new() -> Self {
        TurnAllocationTable {
            allocations: BTreeMap::new(),
        }
    }

    pub fn create_allocation(
        &mut self,
        client_ip: Ipv4Address,
        client_port: u16,
        relayed_ip: Ipv4Address,
        relayed_port: u16,
        lifetime_sec: u32,
    ) -> TurnAllocation {
        let alloc = TurnAllocation {
            client_ip,
            client_port,
            relayed_ip,
            relayed_port,
            lifetime_sec,
        };
        self.allocations.insert((client_ip, client_port), alloc.clone());
        alloc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_allocate_and_relayed_address() {
        let tid = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];
        let req = TurnPacket::build_allocate_request(tid, 600);
        let raw_req = req.serialize();

        let parsed_req = TurnPacket::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.msg_type, TURN_ALLOCATE_REQUEST);

        let relay_ip = Ipv4Address::new(198, 51, 100, 1);
        let relay_port = 49152;
        let resp = TurnPacket::build_allocate_response(&parsed_req, relay_ip, relay_port, 600);
        let raw_resp = resp.serialize();

        let parsed_resp = TurnPacket::parse(&raw_resp).unwrap();
        assert_eq!(parsed_resp.msg_type, TURN_ALLOCATE_RESPONSE);

        let (mapped_ip, mapped_port) = parsed_resp.get_xor_relayed_address().unwrap();
        assert_eq!(mapped_ip, relay_ip);
        assert_eq!(mapped_port, relay_port);
    }

    #[test]
    fn test_turn_send_and_data_indication() {
        let peer_ip = Ipv4Address::new(203, 0, 113, 99);
        let peer_port = 5004;
        let payload = b"Real-time VoIP audio packet through TURN relay";

        let send_ind = TurnPacket::build_send_indication(peer_ip, peer_port, payload);
        let raw_send = send_ind.serialize();

        let parsed_send = TurnPacket::parse(&raw_send).unwrap();
        assert_eq!(parsed_send.msg_type, TURN_SEND_INDICATION);
        assert_eq!(parsed_send.get_xor_peer_address(), Some((peer_ip, peer_port)));
        assert_eq!(parsed_send.get_data_payload(), Some(payload.as_ref()));
    }
}
