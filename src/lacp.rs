//! Link Aggregation Control Protocol (LACP - IEEE 802.1AX / IEEE 802.3ad).
//!
//! Dynamic bundling of physical network links into a logical Link Aggregation Group (LAG / Bond),
//! providing multi-link redundancy and hash-based load balancing over EtherType 0x8809.

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::fmt;

pub const ETHERTYPE_SLOW_PROTOCOLS: u16 = 0x8809;
pub const LACP_SUBTYPE: u8 = 0x01;
pub const LACP_VERSION: u8 = 0x01;

pub const LACP_TLV_ACTOR: u8 = 0x01;
pub const LACP_TLV_PARTNER: u8 = 0x02;
pub const LACP_TLV_COLLECTOR: u8 = 0x03;
pub const LACP_TLV_TERMINATOR: u8 = 0x00;

// LACP State Bitmask
pub const LACP_STATE_ACTIVITY: u8 = 1 << 0;     // 1 = Active, 0 = Passive
pub const LACP_STATE_TIMEOUT: u8 = 1 << 1;      // 1 = Short Timeout, 0 = Long
pub const LACP_STATE_AGGREGATION: u8 = 1 << 2;  // 1 = Aggregatable, 0 = Individual
pub const LACP_STATE_SYNCHRONIZATION: u8 = 1 << 3;
pub const LACP_STATE_COLLECTING: u8 = 1 << 4;   // 1 = Receiving traffic
pub const LACP_STATE_DISTRIBUTING: u8 = 1 << 5; // 1 = Transmitting traffic
pub const LACP_STATE_DEFAULTED: u8 = 1 << 6;
pub const LACP_STATE_EXPIRED: u8 = 1 << 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LacpPortInfo {
    pub system_priority: u16,
    pub system_mac: MacAddress,
    pub key: u16,
    pub port_priority: u16,
    pub port_number: u16,
    pub state: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LacpPacket {
    pub actor: LacpPortInfo,
    pub partner: LacpPortInfo,
    pub collector_max_delay: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LacpError {
    PacketTooShort(usize),
    InvalidSubtype(u8),
    InvalidTlv(u8),
}

impl fmt::Display for LacpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LacpError::PacketTooShort(l) => write!(f, "LACPDU packet too short ({} bytes, min 110)", l),
            LacpError::InvalidSubtype(s) => write!(f, "Invalid Slow Protocol subtype: 0x{:02x} (expected 0x01 LACP)", s),
            LacpError::InvalidTlv(t) => write!(f, "Unexpected LACP TLV type: {}", t),
        }
    }
}

impl std::error::Error for LacpError {}

impl LacpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, LacpError> {
        if data.len() < 110 {
            return Err(LacpError::PacketTooShort(data.len()));
        }

        if data[0] != LACP_SUBTYPE {
            return Err(LacpError::InvalidSubtype(data[0]));
        }

        // Actor TLV (offset 2..22)
        if data[2] != LACP_TLV_ACTOR || data[3] != 20 {
            return Err(LacpError::InvalidTlv(data[2]));
        }
        let actor = Self::parse_port_info(&data[4..22]);

        // Partner TLV (offset 22..42)
        if data[22] != LACP_TLV_PARTNER || data[23] != 20 {
            return Err(LacpError::InvalidTlv(data[22]));
        }
        let partner = Self::parse_port_info(&data[24..42]);

        // Collector TLV (offset 42..58)
        let collector_max_delay = u16::from_be_bytes([data[44], data[45]]);

        Ok(LacpPacket {
            actor,
            partner,
            collector_max_delay,
        })
    }

    fn parse_port_info(b: &[u8]) -> LacpPortInfo {
        let system_priority = u16::from_be_bytes([b[0], b[1]]);
        let system_mac = MacAddress([b[2], b[3], b[4], b[5], b[6], b[7]]);
        let key = u16::from_be_bytes([b[8], b[9]]);
        let port_priority = u16::from_be_bytes([b[10], b[11]]);
        let port_number = u16::from_be_bytes([b[12], b[13]]);
        let state = b[14];

        LacpPortInfo {
            system_priority,
            system_mac,
            key,
            port_priority,
            port_number,
            state,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 110];
        buf[0] = LACP_SUBTYPE;
        buf[1] = LACP_VERSION;

        // Actor TLV
        buf[2] = LACP_TLV_ACTOR;
        buf[3] = 20;
        self.serialize_port_info(&self.actor, &mut buf[4..22]);

        // Partner TLV
        buf[22] = LACP_TLV_PARTNER;
        buf[23] = 20;
        self.serialize_port_info(&self.partner, &mut buf[24..42]);

        // Collector TLV
        buf[42] = LACP_TLV_COLLECTOR;
        buf[43] = 16;
        buf[44..46].copy_from_slice(&self.collector_max_delay.to_be_bytes());

        // Terminator TLV
        buf[58] = LACP_TLV_TERMINATOR;
        buf[59] = 0;

        buf
    }

    fn serialize_port_info(&self, info: &LacpPortInfo, b: &mut [u8]) {
        b[0..2].copy_from_slice(&info.system_priority.to_be_bytes());
        b[2..8].copy_from_slice(&info.system_mac.0);
        b[8..10].copy_from_slice(&info.key.to_be_bytes());
        b[10..12].copy_from_slice(&info.port_priority.to_be_bytes());
        b[12..14].copy_from_slice(&info.port_number.to_be_bytes());
        b[14] = info.state;
    }

    pub fn build(actor: LacpPortInfo, partner: LacpPortInfo) -> Self {
        LacpPacket {
            actor,
            partner,
            collector_max_delay: 0,
        }
    }
}

/// Link Aggregation Group (LAG / Bond)
pub struct LinkAggregationGroup {
    pub bond_name: String,
    pub slave_ports: Vec<String>,
    pub lacp_key: u16,
}

impl LinkAggregationGroup {
    pub fn new(bond_name: &str, slave_ports: Vec<String>, lacp_key: u16) -> Self {
        LinkAggregationGroup {
            bond_name: bond_name.to_string(),
            slave_ports,
            lacp_key,
        }
    }

    /// Computes egress slave port index using Layer 3 + Layer 4 hash policy
    pub fn select_slave_port(&self, src_ip: Ipv4Address, dst_ip: Ipv4Address, src_port: u16, dst_port: u16) -> &str {
        if self.slave_ports.is_empty() {
            return "none";
        }

        let mut hash: u32 = 2166136261; // FNV offset basis
        for &b in &src_ip.0 {
            hash = (hash ^ (b as u32)).wrapping_mul(16777619);
        }
        for &b in &dst_ip.0 {
            hash = (hash ^ (b as u32)).wrapping_mul(16777619);
        }
        hash = (hash ^ (src_port as u32)).wrapping_mul(16777619);
        hash = (hash ^ (dst_port as u32)).wrapping_mul(16777619);

        let idx = (hash as usize) % self.slave_ports.len();
        &self.slave_ports[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lacpdu_packet_roundtrip() {
        let actor = LacpPortInfo {
            system_priority: 32768,
            system_mac: MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
            key: 1,
            port_priority: 128,
            port_number: 1,
            state: LACP_STATE_ACTIVITY | LACP_STATE_AGGREGATION | LACP_STATE_SYNCHRONIZATION | LACP_STATE_COLLECTING | LACP_STATE_DISTRIBUTING,
        };

        let partner = LacpPortInfo {
            system_priority: 32768,
            system_mac: MacAddress([0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
            key: 1,
            port_priority: 128,
            port_number: 2,
            state: LACP_STATE_ACTIVITY | LACP_STATE_AGGREGATION | LACP_STATE_SYNCHRONIZATION | LACP_STATE_COLLECTING | LACP_STATE_DISTRIBUTING,
        };

        let pkt = LacpPacket::build(actor.clone(), partner.clone());
        let raw = pkt.serialize();
        assert_eq!(raw.len(), 110);

        let parsed = LacpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.actor, actor);
        assert_eq!(parsed.partner, partner);
    }

    #[test]
    fn test_lag_load_balancing_hash() {
        let lag = LinkAggregationGroup::new("bond0", vec!["eth0".to_string(), "eth1".to_string()], 1);
        let src = Ipv4Address::new(192, 168, 1, 100);
        let dst = Ipv4Address::new(192, 168, 1, 10);

        let p1 = lag.select_slave_port(src, dst, 50000, 80);
        let p2 = lag.select_slave_port(src, dst, 50001, 80);

        assert!(p1 == "eth0" || p1 == "eth1");
        assert!(p2 == "eth0" || p2 == "eth1");
    }
}
