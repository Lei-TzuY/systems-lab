//! Layer 2.5: Address Resolution Protocol (ARP - RFC 826).
//!
//! ARP maps 32-bit IPv4 addresses to 48-bit Ethernet MAC addresses.

use crate::ethernet::MacAddress;
use std::collections::HashMap;
use std::fmt;

pub const ARP_HTYPE_ETHERNET: u16 = 1;
pub const ARP_PTYPE_IPV4: u16 = 0x0800;
pub const ARP_HLEN_ETHERNET: u8 = 6;
pub const ARP_PLEN_IPV4: u8 = 4;

pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

pub const ARP_PACKET_LEN: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpOpcode {
    Request,
    Reply,
    Unknown(u16),
}

impl ArpOpcode {
    pub fn from_u16(val: u16) -> Self {
        match val {
            ARP_OP_REQUEST => ArpOpcode::Request,
            ARP_OP_REPLY => ArpOpcode::Reply,
            other => ArpOpcode::Unknown(other),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            ArpOpcode::Request => ARP_OP_REQUEST,
            ArpOpcode::Reply => ARP_OP_REPLY,
            ArpOpcode::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for ArpOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArpOpcode::Request => write!(f, "Request (1)"),
            ArpOpcode::Reply => write!(f, "Reply (2)"),
            ArpOpcode::Unknown(val) => write!(f, "Unknown ({})", val),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub opcode: ArpOpcode,
    pub sender_mac: MacAddress,
    pub sender_ip: [u8; 4],
    pub target_mac: MacAddress,
    pub target_ip: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArpError {
    PacketTooShort(usize),
    InvalidHardwareType(u16),
    InvalidProtocolType(u16),
    InvalidAddressLengths(u8, u8),
}

impl fmt::Display for ArpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArpError::PacketTooShort(len) => {
                write!(f, "ARP packet too short ({} bytes, min 28)", len)
            }
            ArpError::InvalidHardwareType(h) => write!(f, "Unsupported hardware type: {}", h),
            ArpError::InvalidProtocolType(p) => write!(f, "Unsupported protocol type: 0x{:04x}", p),
            ArpError::InvalidAddressLengths(h, p) => {
                write!(f, "Invalid address lengths: hlen={}, plen={}", h, p)
            }
        }
    }
}

impl std::error::Error for ArpError {}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, ArpError> {
        if data.len() < ARP_PACKET_LEN {
            return Err(ArpError::PacketTooShort(data.len()));
        }

        let htype = u16::from_be_bytes([data[0], data[1]]);
        let ptype = u16::from_be_bytes([data[2], data[3]]);
        let hlen = data[4];
        let plen = data[5];

        if hlen != ARP_HLEN_ETHERNET || plen != ARP_PLEN_IPV4 {
            return Err(ArpError::InvalidAddressLengths(hlen, plen));
        }

        let opcode_raw = u16::from_be_bytes([data[6], data[7]]);
        let opcode = ArpOpcode::from_u16(opcode_raw);

        let mut sender_mac = [0u8; 6];
        sender_mac.copy_from_slice(&data[8..14]);

        let mut sender_ip = [0u8; 4];
        sender_ip.copy_from_slice(&data[14..18]);

        let mut target_mac = [0u8; 6];
        target_mac.copy_from_slice(&data[18..24]);

        let mut target_ip = [0u8; 4];
        target_ip.copy_from_slice(&data[24..28]);

        Ok(ArpPacket {
            htype,
            ptype,
            hlen,
            plen,
            opcode,
            sender_mac: MacAddress(sender_mac),
            sender_ip,
            target_mac: MacAddress(target_mac),
            target_ip,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(ARP_PACKET_LEN);
        buf.extend_from_slice(&self.htype.to_be_bytes());
        buf.extend_from_slice(&self.ptype.to_be_bytes());
        buf.push(self.hlen);
        buf.push(self.plen);
        buf.extend_from_slice(&self.opcode.to_u16().to_be_bytes());
        buf.extend_from_slice(&self.sender_mac.0);
        buf.extend_from_slice(&self.sender_ip);
        buf.extend_from_slice(&self.target_mac.0);
        buf.extend_from_slice(&self.target_ip);
        buf
    }

    pub fn build_request(sender_mac: MacAddress, sender_ip: [u8; 4], target_ip: [u8; 4]) -> Self {
        ArpPacket {
            htype: ARP_HTYPE_ETHERNET,
            ptype: ARP_PTYPE_IPV4,
            hlen: ARP_HLEN_ETHERNET,
            plen: ARP_PLEN_IPV4,
            opcode: ArpOpcode::Request,
            sender_mac,
            sender_ip,
            target_mac: MacAddress::ZERO,
            target_ip,
        }
    }

    pub fn build_reply(
        sender_mac: MacAddress,
        sender_ip: [u8; 4],
        target_mac: MacAddress,
        target_ip: [u8; 4],
    ) -> Self {
        ArpPacket {
            htype: ARP_HTYPE_ETHERNET,
            ptype: ARP_PTYPE_IPV4,
            hlen: ARP_HLEN_ETHERNET,
            plen: ARP_PLEN_IPV4,
            opcode: ArpOpcode::Reply,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }
}

/// Dynamic ARP Cache table
#[derive(Debug, Default, Clone)]
pub struct ArpTable {
    table: HashMap<[u8; 4], MacAddress>,
}

impl ArpTable {
    pub fn new() -> Self {
        ArpTable {
            table: HashMap::new(),
        }
    }

    pub fn insert(&mut self, ip: [u8; 4], mac: MacAddress) {
        self.table.insert(ip, mac);
    }

    pub fn lookup(&self, ip: &[u8; 4]) -> Option<MacAddress> {
        self.table.get(ip).copied()
    }

    pub fn remove(&mut self, ip: &[u8; 4]) -> Option<MacAddress> {
        self.table.remove(ip)
    }

    pub fn entries(&self) -> &HashMap<[u8; 4], MacAddress> {
        &self.table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arp_packet_roundtrip() {
        let req = ArpPacket::build_request(
            MacAddress([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]),
            [192, 168, 1, 10],
            [192, 168, 1, 1],
        );
        let raw = req.serialize();
        assert_eq!(raw.len(), ARP_PACKET_LEN);

        let parsed = ArpPacket::parse(&raw).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn test_arp_cache() {
        let mut table = ArpTable::new();
        let ip = [10, 0, 0, 1];
        let mac = MacAddress([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);

        assert_eq!(table.lookup(&ip), None);
        table.insert(ip, mac);
        assert_eq!(table.lookup(&ip), Some(mac));
    }
}
