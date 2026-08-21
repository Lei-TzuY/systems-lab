//! Virtual Router Redundancy Protocol Version 3 (VRRPv3 - RFC 5798).
//!
//! Provides default gateway high-availability, automatic Master/Backup election,
//! and virtual MAC address (`00:00:5E:00:01:XX`) translation over IP protocol 112.

use crate::checksum::compute_checksum;
use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::fmt;

pub const IP_PROTO_VRRP: u8 = 112;
pub const VRRP_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 18]);
pub const VRRP_VERSION_3: u8 = 3;
pub const VRRP_TYPE_ADVERTISEMENT: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrrpState {
    Initialize,
    Master,
    Backup,
}

impl fmt::Display for VrrpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VrrpState::Initialize => write!(f, "INITIALIZE"),
            VrrpState::Master => write!(f, "MASTER"),
            VrrpState::Backup => write!(f, "BACKUP"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrrpPacket {
    pub version: u8,
    pub msg_type: u8,
    pub vrid: u8,
    pub priority: u8,
    pub count_ip: u8,
    pub max_adver_int: u16, // in centiseconds (100 = 1 sec)
    pub checksum: u16,
    pub ip_addresses: Vec<Ipv4Address>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VrrpError {
    PacketTooShort(usize),
    InvalidChecksum { computed: u16, expected: u16 },
    InvalidVersion(u8),
}

impl fmt::Display for VrrpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VrrpError::PacketTooShort(len) => write!(f, "VRRP packet too short ({} bytes, min 8)", len),
            VrrpError::InvalidChecksum { computed, expected } => {
                write!(f, "VRRP checksum mismatch: computed 0x{:04x}, expected 0x{:04x}", computed, expected)
            }
            VrrpError::InvalidVersion(v) => write!(f, "Invalid VRRP version: expected 3, found {}", v),
        }
    }
}

impl std::error::Error for VrrpError {}

impl VrrpPacket {
    pub fn parse(data: &[u8], verify_csum: bool) -> Result<Self, VrrpError> {
        if data.len() < 8 {
            return Err(VrrpError::PacketTooShort(data.len()));
        }

        let version = (data[0] >> 4) & 0x0F;
        let msg_type = data[0] & 0x0F;
        if version != VRRP_VERSION_3 {
            return Err(VrrpError::InvalidVersion(version));
        }

        let vrid = data[1];
        let priority = data[2];
        let count_ip = data[3];
        let max_adver_int = u16::from_be_bytes([data[4], data[5]]);
        let checksum = u16::from_be_bytes([data[6], data[7]]);

        let expected_len = 8 + (count_ip as usize) * 4;
        if data.len() < expected_len {
            return Err(VrrpError::PacketTooShort(data.len()));
        }

        if verify_csum {
            let computed = compute_checksum(data);
            if computed != 0 {
                return Err(VrrpError::InvalidChecksum {
                    computed,
                    expected: 0,
                });
            }
        }

        let mut ip_addresses = Vec::new();
        for chunk in data[8..expected_len].chunks_exact(4) {
            let mut ip_b = [0u8; 4];
            ip_b.copy_from_slice(chunk);
            ip_addresses.push(Ipv4Address(ip_b));
        }

        Ok(VrrpPacket {
            version,
            msg_type,
            vrid,
            priority,
            count_ip,
            max_adver_int,
            checksum,
            ip_addresses,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = 8 + self.ip_addresses.len() * 4;
        let mut buf = vec![0u8; total_len];

        buf[0] = ((self.version & 0x0F) << 4) | (self.msg_type & 0x0F);
        buf[1] = self.vrid;
        buf[2] = self.priority;
        buf[3] = self.ip_addresses.len() as u8;
        buf[4..6].copy_from_slice(&self.max_adver_int.to_be_bytes());
        buf[6] = 0;
        buf[7] = 0;

        for (i, ip) in self.ip_addresses.iter().enumerate() {
            buf[8 + i * 4..8 + (i + 1) * 4].copy_from_slice(&ip.0);
        }

        let csum = compute_checksum(&buf);
        buf[6] = (csum >> 8) as u8;
        buf[7] = (csum & 0xFF) as u8;

        buf
    }

    /// Computes the Virtual Router MAC address (RFC 5798: 00:00:5E:00:01:VRID)
    pub fn virtual_mac(vrid: u8) -> MacAddress {
        MacAddress([0x00, 0x00, 0x5e, 0x00, 0x01, vrid])
    }
}

/// VRRPv3 Virtual Router State Machine
pub struct VrrpEngine {
    pub vrid: u8,
    pub priority: u8,
    pub virtual_ip: Ipv4Address,
    pub state: VrrpState,
    pub master_priority: u8,
}

impl VrrpEngine {
    pub fn new(vrid: u8, priority: u8, virtual_ip: Ipv4Address) -> Self {
        let initial_state = if priority == 255 {
            VrrpState::Master // Priority 255 is IP Owner
        } else {
            VrrpState::Backup
        };

        VrrpEngine {
            vrid,
            priority,
            virtual_ip,
            state: initial_state,
            master_priority: priority,
        }
    }

    pub fn build_advertisement(&self) -> VrrpPacket {
        let mut pkt = VrrpPacket {
            version: VRRP_VERSION_3,
            msg_type: VRRP_TYPE_ADVERTISEMENT,
            vrid: self.vrid,
            priority: self.priority,
            count_ip: 1,
            max_adver_int: 100, // 100 centiseconds = 1.0 sec
            checksum: 0,
            ip_addresses: vec![self.virtual_ip],
        };
        let raw = pkt.serialize();
        pkt.checksum = u16::from_be_bytes([raw[6], raw[7]]);
        pkt
    }

    /// Ingests advertisement from another router and resolves Master/Backup election
    pub fn process_advertisement(&mut self, pkt: &VrrpPacket) -> bool {
        if pkt.vrid != self.vrid {
            return false;
        }

        if pkt.priority > self.priority {
            // Master has higher priority, we step down or stay Backup
            self.state = VrrpState::Backup;
            self.master_priority = pkt.priority;
            true
        } else if pkt.priority < self.priority && self.state == VrrpState::Backup {
            // We have higher priority than current sender -> Preempt and become Master
            self.state = VrrpState::Master;
            self.master_priority = self.priority;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vrrp_packet_serialize_and_parse() {
        let pkt = VrrpPacket {
            version: VRRP_VERSION_3,
            msg_type: VRRP_TYPE_ADVERTISEMENT,
            vrid: 10,
            priority: 150,
            count_ip: 1,
            max_adver_int: 100,
            checksum: 0,
            ip_addresses: vec![Ipv4Address::new(192, 168, 1, 1)],
        };

        let raw = pkt.serialize();
        let parsed = VrrpPacket::parse(&raw, true).unwrap();

        assert_eq!(parsed.vrid, 10);
        assert_eq!(parsed.priority, 150);
        assert_eq!(parsed.ip_addresses[0], Ipv4Address::new(192, 168, 1, 1));
        assert_eq!(VrrpPacket::virtual_mac(10), MacAddress([0x00, 0x00, 0x5e, 0x00, 0x01, 10]));
    }

    #[test]
    fn test_vrrp_master_backup_election() {
        let mut router_a = VrrpEngine::new(1, 100, Ipv4Address::new(192, 168, 1, 1));
        let router_b = VrrpEngine::new(1, 200, Ipv4Address::new(192, 168, 1, 1));

        assert_eq!(router_a.state, VrrpState::Backup);

        // Router B advertises priority 200 to Router A
        let adv_b = router_b.build_advertisement();
        router_a.process_advertisement(&adv_b);
        assert_eq!(router_a.state, VrrpState::Backup);
        assert_eq!(router_a.master_priority, 200);
    }
}
