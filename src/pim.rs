//! Protocol Independent Multicast - Sparse Mode (PIM-SM - RFC 7761).
//!
//! Enterprise IP Multicast Dynamic Routing over IP Protocol 103 (Multicast 224.0.0.13).
//! Implements PIM Hello neighbor discovery, Rendezvous Point (RP) trees (*, G), and Join/Prune signaling.

use crate::checksum::compute_checksum;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;
use std::fmt;

pub const IP_PROTO_PIM: u8 = 103;
pub const ALL_PIM_ROUTERS_MULTICAST: Ipv4Address = Ipv4Address([224, 0, 0, 13]);
pub const PIM_HEADER_LEN: usize = 4;

// PIM Message Types
pub const PIM_TYPE_HELLO: u8 = 0;
pub const PIM_TYPE_REGISTER: u8 = 1;
pub const PIM_TYPE_REGISTER_STOP: u8 = 2;
pub const PIM_TYPE_JOIN_PRUNE: u8 = 3;
pub const PIM_TYPE_BOOTSTRAP: u8 = 4;
pub const PIM_TYPE_ASSERT: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PimHeader {
    pub version: u8,
    pub msg_type: u8,
    pub checksum: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PimPacket {
    pub header: PimHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PimError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidChecksum,
}

impl fmt::Display for PimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PimError::PacketTooShort(l) => write!(f, "PIM packet too short ({} bytes, min 4)", l),
            PimError::InvalidVersion(v) => {
                write!(f, "Invalid PIM version: expected 2, found {}", v)
            }
            PimError::InvalidChecksum => write!(f, "PIM header checksum verification failed"),
        }
    }
}

impl std::error::Error for PimError {}

impl PimPacket {
    pub fn parse(data: &[u8], verify_csum: bool) -> Result<Self, PimError> {
        if data.len() < PIM_HEADER_LEN {
            return Err(PimError::PacketTooShort(data.len()));
        }

        let ver_type = data[0];
        let version = ver_type >> 4;
        let msg_type = ver_type & 0x0F;

        if version != 2 {
            return Err(PimError::InvalidVersion(version));
        }

        if verify_csum && compute_checksum(data) != 0 {
            return Err(PimError::InvalidChecksum);
        }

        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let payload = data[PIM_HEADER_LEN..].to_vec();

        Ok(PimPacket {
            header: PimHeader {
                version,
                msg_type,
                checksum,
            },
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; PIM_HEADER_LEN + self.payload.len()];
        buf[0] = (self.header.version << 4) | (self.header.msg_type & 0x0F);
        buf[1] = 0x00; // Reserved
        buf[2..4].copy_from_slice(&0u16.to_be_bytes()); // Checksum placeholder
        buf[4..].copy_from_slice(&self.payload);

        let csum = compute_checksum(&buf);
        buf[2] = (csum >> 8) as u8;
        buf[3] = (csum & 0xFF) as u8;

        buf
    }

    pub fn build_hello(hold_time: u16, dr_priority: u32) -> Self {
        let mut payload = Vec::new();
        // Option 1: HoldTime (Len 2)
        payload.extend_from_slice(&1u16.to_be_bytes());
        payload.extend_from_slice(&2u16.to_be_bytes());
        payload.extend_from_slice(&hold_time.to_be_bytes());

        // Option 19: DR Priority (Len 4)
        payload.extend_from_slice(&19u16.to_be_bytes());
        payload.extend_from_slice(&4u16.to_be_bytes());
        payload.extend_from_slice(&dr_priority.to_be_bytes());

        PimPacket {
            header: PimHeader {
                version: 2,
                msg_type: PIM_TYPE_HELLO,
                checksum: 0,
            },
            payload,
        }
    }

    pub fn build_join_group(
        upstream_neighbor: Ipv4Address,
        group: Ipv4Address,
        rp: Ipv4Address,
    ) -> Self {
        let mut payload = Vec::new();
        // Upstream Neighbor Unicast Address (Family 1 = IPv4)
        payload.push(1); // Addr Family IPv4
        payload.push(0); // Encoding Type Native
        payload.extend_from_slice(&upstream_neighbor.0);

        payload.push(0); // Reserved
        payload.push(1); // Num Groups = 1
        payload.extend_from_slice(&105u16.to_be_bytes()); // HoldTime = 105s

        // Multicast Group Address
        payload.push(1); // Addr Family IPv4
        payload.push(0); // Encoding Native
        payload.push(0); // Reserved
        payload.push(32); // Mask Len 32
        payload.extend_from_slice(&group.0);

        payload.extend_from_slice(&1u16.to_be_bytes()); // Num Joined Sources = 1
        payload.extend_from_slice(&0u16.to_be_bytes()); // Num Pruned Sources = 0

        // Joined Source (Rendezvous Point (*, G) tree flag: WC=1, RPT=1)
        payload.push(1); // Family IPv4
        payload.push(0); // Encoding
        payload.push(0x06); // Flags: WC=1, RPT=1
        payload.push(32); // Mask Len 32
        payload.extend_from_slice(&rp.0);

        PimPacket {
            header: PimHeader {
                version: 2,
                msg_type: PIM_TYPE_JOIN_PRUNE,
                checksum: 0,
            },
            payload,
        }
    }
}

/// PIM Multicast Distribution Tree State Manager
pub struct PimMulticastRouter {
    pub rendezvous_point: Ipv4Address,
    pub active_groups: HashMap<Ipv4Address, Ipv4Address>, // Group IP -> Upstream RP/Source
}

impl Default for PimMulticastRouter {
    fn default() -> Self {
        Self::new(Ipv4Address::new(10, 254, 1, 1))
    }
}

impl PimMulticastRouter {
    pub fn new(rp: Ipv4Address) -> Self {
        let mut router = PimMulticastRouter {
            rendezvous_point: rp,
            active_groups: HashMap::new(),
        };
        // Pre-configure enterprise multicast group
        router.join_shared_tree(Ipv4Address::new(239, 1, 1, 100));
        router
    }

    pub fn join_shared_tree(&mut self, group: Ipv4Address) {
        self.active_groups.insert(group, self.rendezvous_point);
    }

    pub fn prune_group(&mut self, group: &Ipv4Address) {
        self.active_groups.remove(group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pim_hello_packet_roundtrip() {
        let hello = PimPacket::build_hello(105, 100);
        let raw = hello.serialize();

        assert!(raw.len() >= PIM_HEADER_LEN);
        let parsed = PimPacket::parse(&raw, true).unwrap();
        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.msg_type, PIM_TYPE_HELLO);
    }

    #[test]
    fn test_pim_join_prune_shared_tree() {
        let up = Ipv4Address::new(192, 168, 1, 1);
        let grp = Ipv4Address::new(239, 255, 10, 1);
        let rp = Ipv4Address::new(10, 254, 1, 1);

        let join = PimPacket::build_join_group(up, grp, rp);
        let raw = join.serialize();

        let parsed = PimPacket::parse(&raw, true).unwrap();
        assert_eq!(parsed.header.msg_type, PIM_TYPE_JOIN_PRUNE);

        let mut router = PimMulticastRouter::new(rp);
        router.join_shared_tree(grp);
        assert_eq!(router.active_groups.get(&grp), Some(&rp));
    }
}
