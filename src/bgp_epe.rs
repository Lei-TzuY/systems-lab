//! BGP Segment Routing Egress Peer Engineering (BGP-EPE / RFC 9086 & RFC 9087).
//!
//! Implements SR-TE Egress Peer Engineering with PeerNode-SID, PeerAdj-SID,
//! and PeerSet-SID for fine-grained inter-AS outbound traffic steering.

use crate::ipv4::Ipv4Address;

pub const BGP_EPE_PEER_NODE_SID: u8 = 1;
pub const BGP_EPE_PEER_ADJ_SID: u8 = 2;
pub const BGP_EPE_PEER_SET_SID: u8 = 3;

/// BGP Peering Segment Identifier (Peer-SID)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerSid {
    pub sid_type: u8,
    pub label: u32,
    pub peer_asn: u32,
    pub peer_ip: Ipv4Address,
    pub egress_interface_id: Option<u32>,
    pub weight: u8, // Load-balancing weight (1..100)
}

/// BGP-EPE Routing & Steering Database
#[derive(Debug, Clone, Default)]
pub struct BgpEpeDatabase {
    pub peering_sids: Vec<PeerSid>,
}

impl BgpEpeDatabase {
    pub fn new() -> Self {
        BgpEpeDatabase {
            peering_sids: Vec::new(),
        }
    }

    /// Registers a PeerNode-SID (steers traffic to a specific BGP peer node)
    pub fn add_peer_node_sid(&mut self, label: u32, peer_asn: u32, peer_ip: Ipv4Address) {
        self.peering_sids.push(PeerSid {
            sid_type: BGP_EPE_PEER_NODE_SID,
            label,
            peer_asn,
            peer_ip,
            egress_interface_id: None,
            weight: 100,
        });
    }

    /// Registers a PeerAdj-SID (steers traffic over a specific link to a peer)
    pub fn add_peer_adj_sid(&mut self, label: u32, peer_asn: u32, peer_ip: Ipv4Address, iface_id: u32) {
        self.peering_sids.push(PeerSid {
            sid_type: BGP_EPE_PEER_ADJ_SID,
            label,
            peer_asn,
            peer_ip,
            egress_interface_id: Some(iface_id),
            weight: 100,
        });
    }

    /// Registers a PeerSet-SID entry (part of an ECMP / weighted group across peers)
    pub fn add_peer_set_member(&mut self, label: u32, peer_asn: u32, peer_ip: Ipv4Address, iface_id: Option<u32>, weight: u8) {
        self.peering_sids.push(PeerSid {
            sid_type: BGP_EPE_PEER_SET_SID,
            label,
            peer_asn,
            peer_ip,
            egress_interface_id: iface_id,
            weight,
        });
    }

    /// Resolves an incoming EPE Label to its candidate egress paths
    pub fn resolve_egress_path(&self, label: u32) -> Vec<&PeerSid> {
        self.peering_sids.iter().filter(|s| s.label == label).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgp_epe_peering_sids_and_resolution() {
        let mut epe_db = BgpEpeDatabase::new();
        let peer_ip1 = Ipv4Address::new(198, 51, 100, 1);
        let peer_ip2 = Ipv4Address::new(198, 51, 100, 2);

        // 1. PeerNode-SID
        epe_db.add_peer_node_sid(1001, 65001, peer_ip1);

        // 2. PeerAdj-SID
        epe_db.add_peer_adj_sid(1002, 65001, peer_ip1, 3);

        // 3. PeerSet-SID (ECMP set 1003 across peer1 and peer2)
        epe_db.add_peer_set_member(1003, 65001, peer_ip1, Some(1), 50);
        epe_db.add_peer_set_member(1003, 65002, peer_ip2, Some(2), 50);

        // Lookup PeerNode-SID
        let node_paths = epe_db.resolve_egress_path(1001);
        assert_eq!(node_paths.len(), 1);
        assert_eq!(node_paths[0].sid_type, BGP_EPE_PEER_NODE_SID);
        assert_eq!(node_paths[0].peer_ip, peer_ip1);

        // Lookup PeerAdj-SID
        let adj_paths = epe_db.resolve_egress_path(1002);
        assert_eq!(adj_paths.len(), 1);
        assert_eq!(adj_paths[0].egress_interface_id, Some(3));

        // Lookup PeerSet-SID
        let set_paths = epe_db.resolve_egress_path(1003);
        assert_eq!(set_paths.len(), 2);
        assert_eq!(set_paths[0].weight, 50);
        assert_eq!(set_paths[1].weight, 50);
    }
}
