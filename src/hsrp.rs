//! Cisco Hot Standby Router Protocol (HSRPv1 - RFC 2281).
//!
//! First-hop default gateway redundancy and failover over UDP port 1985 and Multicast 224.0.0.2.

use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::fmt;

pub const HSRP_UDP_PORT: u16 = 1985;
pub const HSRP_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 2]);
pub const HSRP_HEADER_LEN: usize = 20;

// HSRP OpCodes
pub const HSRP_OP_HELLO: u8 = 0;
pub const HSRP_OP_COUP: u8 = 1;
pub const HSRP_OP_RESIGN: u8 = 2;

// HSRP States
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HsrpState {
    Initial = 0,
    Learn = 1,
    Listen = 2,
    Speak = 4,
    Standby = 8,
    Active = 16,
}

impl fmt::Display for HsrpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HsrpState::Initial => write!(f, "Initial"),
            HsrpState::Learn => write!(f, "Learn"),
            HsrpState::Listen => write!(f, "Listen"),
            HsrpState::Speak => write!(f, "Speak"),
            HsrpState::Standby => write!(f, "Standby"),
            HsrpState::Active => write!(f, "Active"),
        }
    }
}

impl HsrpState {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => HsrpState::Initial,
            1 => HsrpState::Learn,
            2 => HsrpState::Listen,
            4 => HsrpState::Speak,
            8 => HsrpState::Standby,
            16 => HsrpState::Active,
            _ => HsrpState::Listen,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HsrpPacket {
    pub version: u8,
    pub opcode: u8,
    pub state: HsrpState,
    pub hellotime: u8,
    pub holdtime: u8,
    pub priority: u8,
    pub group: u8,
    pub reserved: u8,
    pub auth_data: [u8; 8],
    pub virtual_ip: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HsrpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
}

impl fmt::Display for HsrpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HsrpError::PacketTooShort(l) => write!(f, "HSRP packet too short ({} bytes)", l),
            HsrpError::InvalidVersion(v) => write!(f, "Invalid HSRP version: {}", v),
        }
    }
}

impl std::error::Error for HsrpError {}

impl HsrpPacket {
    pub fn build_hello(state: HsrpState, group: u8, priority: u8, virtual_ip: Ipv4Address) -> Self {
        let mut auth = [0u8; 8];
        auth[..5].copy_from_slice(b"cisco");
        HsrpPacket {
            version: 0,
            opcode: HSRP_OP_HELLO,
            state,
            hellotime: 3,
            holdtime: 10,
            priority,
            group,
            reserved: 0,
            auth_data: auth,
            virtual_ip,
        }
    }

    /// RFC 2281 Cisco Virtual MAC address calculation: 00:00:0c:07:ac:XX
    pub fn virtual_mac(group: u8) -> MacAddress {
        MacAddress([0x00, 0x00, 0x0C, 0x07, 0xAC, group])
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.version);
        buf.push(self.opcode);
        buf.push(self.state as u8);
        buf.push(self.hellotime);
        buf.push(self.holdtime);
        buf.push(self.priority);
        buf.push(self.group);
        buf.push(self.reserved);
        buf.extend_from_slice(&self.auth_data);
        buf.extend_from_slice(&self.virtual_ip.0);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, HsrpError> {
        if data.len() < HSRP_HEADER_LEN {
            return Err(HsrpError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if version != 0 {
            return Err(HsrpError::InvalidVersion(version));
        }

        let opcode = data[1];
        let state = HsrpState::from_u8(data[2]);
        let hellotime = data[3];
        let holdtime = data[4];
        let priority = data[5];
        let group = data[6];
        let reserved = data[7];

        let mut auth_data = [0u8; 8];
        auth_data.copy_from_slice(&data[8..16]);

        let virtual_ip = Ipv4Address([data[16], data[17], data[18], data[19]]);

        Ok(HsrpPacket {
            version,
            opcode,
            state,
            hellotime,
            holdtime,
            priority,
            group,
            reserved,
            auth_data,
            virtual_ip,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HsrpEngine {
    pub group: u8,
    pub priority: u8,
    pub virtual_ip: Ipv4Address,
    pub state: HsrpState,
    pub preempt: bool,
    pub active_router: Option<Ipv4Address>,
}

impl HsrpEngine {
    pub fn new(group: u8, priority: u8, virtual_ip: Ipv4Address, preempt: bool) -> Self {
        HsrpEngine {
            group,
            priority,
            virtual_ip,
            state: HsrpState::Speak,
            preempt,
            active_router: None,
        }
    }

    pub fn build_advertisement(&self) -> HsrpPacket {
        HsrpPacket::build_hello(self.state, self.group, self.priority, self.virtual_ip)
    }

    pub fn process_packet(&mut self, pkt: &HsrpPacket, src_ip: Ipv4Address) {
        if pkt.group != self.group {
            return;
        }

        if pkt.state == HsrpState::Active {
            self.active_router = Some(src_ip);
            if self.state == HsrpState::Active {
                if pkt.priority > self.priority {
                    self.state = HsrpState::Standby;
                }
            } else if self.preempt && self.priority > pkt.priority {
                self.state = HsrpState::Active;
            } else {
                self.state = HsrpState::Standby;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsrp_packet_roundtrip_and_virtual_mac() {
        let v_ip = Ipv4Address::new(192, 168, 1, 254);
        let pkt = HsrpPacket::build_hello(HsrpState::Active, 1, 110, v_ip);
        let raw = pkt.serialize();

        assert_eq!(raw.len(), HSRP_HEADER_LEN);
        let parsed = HsrpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.state, HsrpState::Active);
        assert_eq!(parsed.priority, 110);
        assert_eq!(parsed.virtual_ip, v_ip);

        let v_mac = HsrpPacket::virtual_mac(1);
        assert_eq!(v_mac.0, [0x00, 0x00, 0x0C, 0x07, 0xAC, 0x01]);
    }

    #[test]
    fn test_hsrp_election_and_preemption() {
        let v_ip = Ipv4Address::new(10, 0, 0, 1);
        let mut engine = HsrpEngine::new(5, 120, v_ip, true);

        // Lower priority peer advertises Active
        let peer_pkt = HsrpPacket::build_hello(HsrpState::Active, 5, 100, v_ip);
        engine.process_packet(&peer_pkt, Ipv4Address::new(10, 0, 0, 2));

        // Preempts and becomes Active because 120 > 100
        assert_eq!(engine.state, HsrpState::Active);

        // Higher priority peer advertises Active (150 > 120)
        let higher_pkt = HsrpPacket::build_hello(HsrpState::Active, 5, 150, v_ip);
        engine.process_packet(&higher_pkt, Ipv4Address::new(10, 0, 0, 3));

        // Steps down to Standby
        assert_eq!(engine.state, HsrpState::Standby);
    }
}
