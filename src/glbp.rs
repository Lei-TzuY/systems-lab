//! Cisco Gateway Load Balancing Protocol (GLBP).
//!
//! First-hop active-active default gateway redundancy and load balancing over UDP port 3222 and Multicast 224.0.0.102.

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::fmt;

pub const GLBP_UDP_PORT: u16 = 3222;
pub const GLBP_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 102]);
pub const GLBP_HEADER_LEN: usize = 24;

// GLBP OpCodes
pub const GLBP_OP_HELLO: u8 = 1;
pub const GLBP_OP_REQUEST: u8 = 2;
pub const GLBP_OP_REPLY: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbpRole {
    ActiveVirtualGateway,
    StandbyVirtualGateway,
    ActiveVirtualForwarder,
    Listen,
}

impl fmt::Display for GlbpRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlbpRole::ActiveVirtualGateway => write!(f, "Active Virtual Gateway (AVG)"),
            GlbpRole::StandbyVirtualGateway => write!(f, "Standby Virtual Gateway (SVG)"),
            GlbpRole::ActiveVirtualForwarder => write!(f, "Active Virtual Forwarder (AVF)"),
            GlbpRole::Listen => write!(f, "Listen"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbpLoadBalancing {
    RoundRobin,
    HostDependent,
    Weighted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbpPacket {
    pub version: u8,
    pub opcode: u8,
    pub group: u8,
    pub priority: u8,
    pub forwarder_num: u8,
    pub weight: u8,
    pub hellotime: u8,
    pub holdtime: u8,
    pub virtual_ip: Ipv4Address,
    pub virtual_mac: MacAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlbpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
}

impl fmt::Display for GlbpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlbpError::PacketTooShort(l) => write!(f, "GLBP packet too short ({} bytes)", l),
            GlbpError::InvalidVersion(v) => write!(f, "Invalid GLBP version: {}", v),
        }
    }
}

impl std::error::Error for GlbpError {}

impl GlbpPacket {
    /// Cisco GLBP Virtual MAC Address mapping: 00:07:b4:00:GG:FF
    pub fn virtual_mac(group: u8, forwarder: u8) -> MacAddress {
        MacAddress([0x00, 0x07, 0xB4, 0x00, group, forwarder])
    }

    pub fn build_hello(
        group: u8,
        priority: u8,
        forwarder_num: u8,
        weight: u8,
        virtual_ip: Ipv4Address,
    ) -> Self {
        GlbpPacket {
            version: 1,
            opcode: GLBP_OP_HELLO,
            group,
            priority,
            forwarder_num,
            weight,
            hellotime: 3,
            holdtime: 10,
            virtual_ip,
            virtual_mac: Self::virtual_mac(group, forwarder_num),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.version);
        buf.push(self.opcode);
        buf.push(self.group);
        buf.push(0x00); // Reserved
        buf.push(self.priority);
        buf.push(self.forwarder_num);
        buf.push(self.weight);
        buf.push(self.hellotime);
        buf.push(self.holdtime);
        buf.extend_from_slice(&[0u8; 3]); // Reserved2
        buf.extend_from_slice(&self.virtual_ip.0);
        buf.extend_from_slice(&self.virtual_mac.0);
        buf.extend_from_slice(&[0u8; 2]); // Reserved3 / Alignment
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, GlbpError> {
        if data.len() < GLBP_HEADER_LEN {
            return Err(GlbpError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if version != 1 {
            return Err(GlbpError::InvalidVersion(version));
        }

        let opcode = data[1];
        let group = data[2];
        let priority = data[4];
        let forwarder_num = data[5];
        let weight = data[6];
        let hellotime = data[7];
        let holdtime = data[8];

        let virtual_ip = Ipv4Address([data[12], data[13], data[14], data[15]]);
        let mut mac_bytes = [0u8; 6];
        mac_bytes.copy_from_slice(&data[16..22]);
        let virtual_mac = MacAddress(mac_bytes);

        Ok(GlbpPacket {
            version,
            opcode,
            group,
            priority,
            forwarder_num,
            weight,
            hellotime,
            holdtime,
            virtual_ip,
            virtual_mac,
        })
    }
}

#[derive(Debug, Clone)]
pub struct GlbpEngine {
    pub group: u8,
    pub priority: u8,
    pub weight: u8,
    pub virtual_ip: Ipv4Address,
    pub role: GlbpRole,
    pub active_forwarders: Vec<u8>,
    pub load_balancing: GlbpLoadBalancing,
    pub rr_index: usize,
}

impl GlbpEngine {
    pub fn new(group: u8, priority: u8, virtual_ip: Ipv4Address) -> Self {
        GlbpEngine {
            group,
            priority,
            weight: 100,
            virtual_ip,
            role: GlbpRole::ActiveVirtualGateway,
            active_forwarders: vec![1, 2],
            load_balancing: GlbpLoadBalancing::RoundRobin,
            rr_index: 0,
        }
    }

    pub fn build_advertisement(&self) -> GlbpPacket {
        GlbpPacket::build_hello(self.group, self.priority, 1, self.weight, self.virtual_ip)
    }

    /// Resolve virtual MAC to reply to ARP requests according to load balancing policy
    pub fn resolve_arp_reply_mac(&mut self) -> MacAddress {
        if self.active_forwarders.is_empty() {
            return GlbpPacket::virtual_mac(self.group, 1);
        }

        let f_num = self.active_forwarders[self.rr_index % self.active_forwarders.len()];
        self.rr_index = self.rr_index.wrapping_add(1);
        GlbpPacket::virtual_mac(self.group, f_num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glbp_packet_roundtrip_and_virtual_mac() {
        let vip = Ipv4Address::new(192, 168, 1, 254);
        let pkt = GlbpPacket::build_hello(10, 120, 1, 100, vip);
        let raw = pkt.serialize();

        assert!(raw.len() >= GLBP_HEADER_LEN);
        let parsed = GlbpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.group, 10);
        assert_eq!(parsed.priority, 120);
        assert_eq!(parsed.forwarder_num, 1);
        assert_eq!(parsed.virtual_ip, vip);

        let expected_mac = MacAddress([0x00, 0x07, 0xB4, 0x00, 10, 1]);
        assert_eq!(parsed.virtual_mac, expected_mac);
    }

    #[test]
    fn test_glbp_round_robin_mac_distribution() {
        let vip = Ipv4Address::new(10, 0, 0, 1);
        let mut engine = GlbpEngine::new(1, 100, vip);
        engine.active_forwarders = vec![1, 2, 3];

        let mac1 = engine.resolve_arp_reply_mac();
        let mac2 = engine.resolve_arp_reply_mac();
        let mac3 = engine.resolve_arp_reply_mac();
        let mac4 = engine.resolve_arp_reply_mac();

        assert_eq!(mac1, GlbpPacket::virtual_mac(1, 1));
        assert_eq!(mac2, GlbpPacket::virtual_mac(1, 2));
        assert_eq!(mac3, GlbpPacket::virtual_mac(1, 3));
        assert_eq!(mac4, GlbpPacket::virtual_mac(1, 1)); // Cycles back to 1
    }
}
