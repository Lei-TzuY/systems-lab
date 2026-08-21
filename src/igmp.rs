//! Internet Group Management Protocol Version 2 (IGMPv2 - RFC 2236) & Multicast MAC (RFC 1112).
//!
//! Manages IP multicast group subscriptions, IGMP query/report messages, and Ethernet multicast MAC address translation.

use crate::checksum::compute_checksum;
use crate::ethernet::MacAddress;
use crate::ipv4::Ipv4Address;
use std::collections::HashSet;
use std::fmt;

pub const IP_PROTO_IGMP: u8 = 2;

// IGMPv2 Message Types
pub const IGMP_TYPE_MEMBERSHIP_QUERY: u8 = 0x11;
pub const IGMP_TYPE_V1_MEMBERSHIP_REPORT: u8 = 0x12;
pub const IGMP_TYPE_V2_MEMBERSHIP_REPORT: u8 = 0x16;
pub const IGMP_TYPE_LEAVE_GROUP: u8 = 0x17;

pub const ALL_HOSTS_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 1]);
pub const ALL_ROUTERS_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 2]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgmpPacket {
    pub msg_type: u8,
    pub max_response_time: u8,
    pub checksum: u16,
    pub group_address: Ipv4Address,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgmpError {
    PacketTooShort(usize),
    InvalidChecksum { computed: u16, expected: u16 },
}

impl fmt::Display for IgmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IgmpError::PacketTooShort(len) => write!(f, "IGMP packet too short ({} bytes, min 8)", len),
            IgmpError::InvalidChecksum { computed, expected } => {
                write!(f, "IGMP checksum error: computed 0x{:04x}, expected 0x{:04x}", computed, expected)
            }
        }
    }
}

impl std::error::Error for IgmpError {}

impl IgmpPacket {
    pub fn parse(data: &[u8], verify_csum: bool) -> Result<Self, IgmpError> {
        if data.len() < 8 {
            return Err(IgmpError::PacketTooShort(data.len()));
        }

        let msg_type = data[0];
        let max_response_time = data[1];
        let checksum = u16::from_be_bytes([data[2], data[3]]);

        let mut group_bytes = [0u8; 4];
        group_bytes.copy_from_slice(&data[4..8]);
        let group_address = Ipv4Address(group_bytes);

        if verify_csum {
            let computed = compute_checksum(data);
            if computed != 0 {
                return Err(IgmpError::InvalidChecksum {
                    computed,
                    expected: 0,
                });
            }
        }

        Ok(IgmpPacket {
            msg_type,
            max_response_time,
            checksum,
            group_address,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 8];
        buf[0] = self.msg_type;
        buf[1] = self.max_response_time;
        buf[2] = 0;
        buf[3] = 0;
        buf[4..8].copy_from_slice(&self.group_address.0);

        let csum = compute_checksum(&buf);
        buf[2] = (csum >> 8) as u8;
        buf[3] = (csum & 0xFF) as u8;

        buf
    }

    pub fn build_membership_query(group: Ipv4Address, max_resp_time_sec: u8) -> Self {
        let mut pkt = IgmpPacket {
            msg_type: IGMP_TYPE_MEMBERSHIP_QUERY,
            max_response_time: max_resp_time_sec * 10,
            checksum: 0,
            group_address: group,
        };
        let raw = pkt.serialize();
        pkt.checksum = u16::from_be_bytes([raw[2], raw[3]]);
        pkt
    }

    pub fn build_v2_membership_report(group: Ipv4Address) -> Self {
        let mut pkt = IgmpPacket {
            msg_type: IGMP_TYPE_V2_MEMBERSHIP_REPORT,
            max_response_time: 0,
            checksum: 0,
            group_address: group,
        };
        let raw = pkt.serialize();
        pkt.checksum = u16::from_be_bytes([raw[2], raw[3]]);
        pkt
    }

    pub fn build_leave_group(group: Ipv4Address) -> Self {
        let mut pkt = IgmpPacket {
            msg_type: IGMP_TYPE_LEAVE_GROUP,
            max_response_time: 0,
            checksum: 0,
            group_address: group,
        };
        let raw = pkt.serialize();
        pkt.checksum = u16::from_be_bytes([raw[2], raw[3]]);
        pkt
    }
}

/// Converts an IPv4 Multicast IP into an Ethernet Multicast MAC (RFC 1112).
///
/// Format: 01:00:5E:[low 23 bits of IPv4 address]
pub fn multicast_ip_to_mac(ip: Ipv4Address) -> MacAddress {
    let bytes = ip.0;
    MacAddress([
        0x01,
        0x00,
        0x5e,
        bytes[1] & 0x7F,
        bytes[2],
        bytes[3],
    ])
}

/// Dynamic in-memory Multicast Group Subscription Manager
#[derive(Debug, Clone, Default)]
pub struct MulticastGroupTable {
    groups: HashSet<Ipv4Address>,
}

impl MulticastGroupTable {
    pub fn new() -> Self {
        let mut table = MulticastGroupTable {
            groups: HashSet::new(),
        };
        // Always member of all-hosts multicast
        table.groups.insert(ALL_HOSTS_MULTICAST_IP);
        table
    }

    pub fn join(&mut self, group: Ipv4Address) -> bool {
        self.groups.insert(group)
    }

    pub fn leave(&mut self, group: &Ipv4Address) -> bool {
        if *group == ALL_HOSTS_MULTICAST_IP {
            return false; // Cannot leave all-hosts
        }
        self.groups.remove(group)
    }

    pub fn is_member(&self, group: &Ipv4Address) -> bool {
        self.groups.contains(group)
    }

    pub fn all_groups(&self) -> Vec<Ipv4Address> {
        self.groups.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multicast_ip_to_mac_conversion() {
        // 224.0.0.1 -> 01:00:5E:00:00:01
        let mac1 = multicast_ip_to_mac(Ipv4Address::new(224, 0, 0, 1));
        assert_eq!(mac1, MacAddress([0x01, 0x00, 0x5e, 0x00, 0x00, 0x01]));

        // 239.255.1.2 -> 01:00:5E:7F:01:02
        let mac2 = multicast_ip_to_mac(Ipv4Address::new(239, 255, 1, 2));
        assert_eq!(mac2, MacAddress([0x01, 0x00, 0x5e, 0x7f, 0x01, 0x02]));
    }

    #[test]
    fn test_igmp_packet_roundtrip() {
        let report = IgmpPacket::build_v2_membership_report(Ipv4Address::new(239, 1, 2, 3));
        let raw = report.serialize();

        let parsed = IgmpPacket::parse(&raw, true).unwrap();
        assert_eq!(parsed.msg_type, IGMP_TYPE_V2_MEMBERSHIP_REPORT);
        assert_eq!(parsed.group_address, Ipv4Address::new(239, 1, 2, 3));
    }

    #[test]
    fn test_multicast_group_table() {
        let mut table = MulticastGroupTable::new();
        let target = Ipv4Address::new(239, 10, 20, 30);

        assert!(!table.is_member(&target));
        assert!(table.join(target));
        assert!(table.is_member(&target));

        assert!(table.leave(&target));
        assert!(!table.is_member(&target));
    }
}
