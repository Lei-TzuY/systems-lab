//! Layer 2: Ethernet II frame parsing and serialization.

use std::fmt;
use std::str::FromStr;

pub const ETHER_ADDR_LEN: usize = 6;
pub const ETHER_HEADER_LEN: usize = 14;

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETHERTYPE_IPV6: u16 = 0x86DD;
pub const ETHERTYPE_VLAN: u16 = 0x8100;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub const BROADCAST: MacAddress = MacAddress([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    pub const ZERO: MacAddress = MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

    pub fn new(bytes: [u8; 6]) -> Self {
        MacAddress(bytes)
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    }

    pub fn is_multicast(&self) -> bool {
        (self.0[0] & 0x01) != 0 && !self.is_broadcast()
    }

    pub fn is_unicast(&self) -> bool {
        !self.is_multicast() && !self.is_broadcast()
    }
}

impl fmt::Debug for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl FromStr for MacAddress {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(
                "Invalid MAC address format (expected 6 colon-separated hex bytes)".to_string(),
            );
        }
        let mut bytes = [0u8; 6];
        for (i, p) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(p, 16)
                .map_err(|e| format!("Invalid hex byte '{}': {}", p, e))?;
        }
        Ok(MacAddress(bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtherType {
    IPv4,
    Arp,
    IPv6,
    Vlan,
    Unknown(u16),
}

impl EtherType {
    pub fn from_u16(val: u16) -> Self {
        match val {
            ETHERTYPE_IPV4 => EtherType::IPv4,
            ETHERTYPE_ARP => EtherType::Arp,
            ETHERTYPE_IPV6 => EtherType::IPv6,
            ETHERTYPE_VLAN => EtherType::Vlan,
            other => EtherType::Unknown(other),
        }
    }

    pub fn to_u16(&self) -> u16 {
        match self {
            EtherType::IPv4 => ETHERTYPE_IPV4,
            EtherType::Arp => ETHERTYPE_ARP,
            EtherType::IPv6 => ETHERTYPE_IPV6,
            EtherType::Vlan => ETHERTYPE_VLAN,
            EtherType::Unknown(val) => *val,
        }
    }
}

impl fmt::Display for EtherType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EtherType::IPv4 => write!(f, "IPv4 (0x0800)"),
            EtherType::Arp => write!(f, "ARP (0x0806)"),
            EtherType::IPv6 => write!(f, "IPv6 (0x86DD)"),
            EtherType::Vlan => write!(f, "802.1Q VLAN (0x8100)"),
            EtherType::Unknown(val) => write!(f, "Unknown (0x{:04x})", val),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetFrame<'a> {
    pub dst_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: EtherType,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EthernetError {
    FrameTooShort(usize),
    InvalidLength,
}

impl fmt::Display for EthernetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EthernetError::FrameTooShort(len) => {
                write!(f, "Ethernet frame too short ({} bytes, min 14)", len)
            }
            EthernetError::InvalidLength => write!(f, "Invalid ethernet frame length"),
        }
    }
}

impl std::error::Error for EthernetError {}

impl<'a> EthernetFrame<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, EthernetError> {
        if data.len() < ETHER_HEADER_LEN {
            return Err(EthernetError::FrameTooShort(data.len()));
        }

        let mut dst = [0u8; 6];
        dst.copy_from_slice(&data[0..6]);

        let mut src = [0u8; 6];
        src.copy_from_slice(&data[6..12]);

        let ethertype_raw = u16::from_be_bytes([data[12], data[13]]);
        let ethertype = EtherType::from_u16(ethertype_raw);
        let payload = &data[ETHER_HEADER_LEN..];

        Ok(EthernetFrame {
            dst_mac: MacAddress(dst),
            src_mac: MacAddress(src),
            ethertype,
            payload,
        })
    }

    pub fn serialize(dst: MacAddress, src: MacAddress, ethertype: u16, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(ETHER_HEADER_LEN + payload.len());
        buf.extend_from_slice(&dst.0);
        buf.extend_from_slice(&src.0);
        buf.extend_from_slice(&ethertype.to_be_bytes());
        buf.extend_from_slice(payload);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethernet_parse_and_serialize() {
        let dst = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let src = MacAddress([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        let payload = b"Hello Ethernet!";

        let raw = EthernetFrame::serialize(dst, src, ETHERTYPE_IPV4, payload);
        assert_eq!(raw.len(), 14 + payload.len());

        let frame = EthernetFrame::parse(&raw).unwrap();
        assert_eq!(frame.dst_mac, dst);
        assert_eq!(frame.src_mac, src);
        assert_eq!(frame.ethertype, EtherType::IPv4);
        assert_eq!(frame.payload, payload);
    }
}
