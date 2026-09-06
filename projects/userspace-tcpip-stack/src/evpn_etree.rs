//! EVPN E-Tree Root/Leaf Tree Service Architecture (RFC 8317).
//!
//! Implements BGP EVPN E-Tree Extended Community (Type 0x06, Subtype 0x05),
//! Leaf Indication Flag (L-bit), Leaf-Label encoding, and E-Tree Split-Horizon
//! filtering rules (Root-to-Leaf / Leaf-to-Root allowed; Leaf-to-Leaf blocked).

use crate::ethernet::MacAddress;
use std::collections::HashMap;

/// EVPN E-Tree Extended Community Type and Subtype (RFC 8317 Section 4.1).
pub const BGP_EXT_COMM_TYPE_EVPN: u8 = 0x06;
pub const BGP_EXT_COMM_SUBTYPE_ETREE: u8 = 0x05;

/// E-Tree Node / Endpoint Role (RFC 8317).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ETreeRole {
    Root,
    Leaf,
}

/// EVPN E-Tree Extended Community (RFC 8317 Section 4.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnETreeExtCommunity {
    pub is_leaf: bool,
    pub leaf_label: u32, // 20-bit MPLS label or 24-bit VNI
}

impl EvpnETreeExtCommunity {
    pub fn new_leaf(leaf_label: u32) -> Self {
        EvpnETreeExtCommunity {
            is_leaf: true,
            leaf_label,
        }
    }

    pub fn new_root() -> Self {
        EvpnETreeExtCommunity {
            is_leaf: false,
            leaf_label: 0,
        }
    }

    /// Serializes the 8-octet BGP Extended Community.
    pub fn serialize(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0] = BGP_EXT_COMM_TYPE_EVPN;
        buf[1] = BGP_EXT_COMM_SUBTYPE_ETREE;
        buf[2] = if self.is_leaf { 0x04 } else { 0x00 }; // L-bit (Bit 5 / 0x04)
        buf[3] = 0x00; // Reserved
        buf[4] = 0x00; // Reserved

        // 3-octet Leaf Label (RFC 8317 Section 4.1.2)
        buf[5] = ((self.leaf_label >> 12) & 0xFF) as u8;
        buf[6] = ((self.leaf_label >> 4) & 0xFF) as u8;
        buf[7] = ((self.leaf_label & 0x0F) << 4) as u8 | 0x01;
        buf
    }

    /// Parses the 8-octet BGP Extended Community.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data[0] != BGP_EXT_COMM_TYPE_EVPN || data[1] != BGP_EXT_COMM_SUBTYPE_ETREE {
            return None;
        }
        let is_leaf = (data[2] & 0x04) != 0;
        let label = ((data[5] as u32) << 12) | ((data[6] as u32) << 4) | ((data[7] as u32) >> 4);

        Some(EvpnETreeExtCommunity {
            is_leaf,
            leaf_label: label,
        })
    }
}

/// E-Tree Forwarding Decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ETreeDecision {
    Permitted,
    DroppedLeafToLeaf,
    UnknownEndpoint,
}

/// EVPN E-Tree Forwarding and Role Filtering Engine.
#[derive(Debug, Clone, Default)]
pub struct EvpnETreeEngine {
    pub endpoint_roles: HashMap<(u32, MacAddress), ETreeRole>, // (VNI, MAC) -> Role
    pub blocked_leaf_to_leaf_count: usize,
    pub forwarded_packets_count: usize,
}

impl EvpnETreeEngine {
    pub fn new() -> Self {
        EvpnETreeEngine {
            endpoint_roles: HashMap::new(),
            blocked_leaf_to_leaf_count: 0,
            forwarded_packets_count: 0,
        }
    }

    /// Registers a tenant endpoint role (Root or Leaf) within a VNI.
    pub fn register_endpoint(&mut self, vni: u32, mac: MacAddress, role: ETreeRole) {
        self.endpoint_roles.insert((vni, mac), role);
    }

    /// Evaluates E-Tree policy for traffic between source and destination MACs in a VNI.
    pub fn evaluate_forwarding(
        &mut self,
        vni: u32,
        src_mac: MacAddress,
        dst_mac: MacAddress,
    ) -> ETreeDecision {
        let src_role = match self.endpoint_roles.get(&(vni, src_mac)) {
            Some(r) => *r,
            None => return ETreeDecision::UnknownEndpoint,
        };
        let dst_role = match self.endpoint_roles.get(&(vni, dst_mac)) {
            Some(r) => *r,
            None => return ETreeDecision::UnknownEndpoint,
        };

        match (src_role, dst_role) {
            (ETreeRole::Leaf, ETreeRole::Leaf) => {
                self.blocked_leaf_to_leaf_count += 1;
                ETreeDecision::DroppedLeafToLeaf
            }
            _ => {
                self.forwarded_packets_count += 1;
                ETreeDecision::Permitted
            }
        }
    }
}
