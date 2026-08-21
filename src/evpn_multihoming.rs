//! EVPN Type 4 Ethernet Segment Route & Designated Forwarder (DF) Election (RFC 7432).
//!
//! Provides all-active multi-homing redundancy, split-horizon filtering, and deterministic
//! Designated Forwarder (DF) election for Broadcast, Unknown Unicast, and Multicast (BUM) traffic.

use crate::evpn::RouteDistinguisher;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const EVPN_ROUTE_TYPE_ETHERNET_SEGMENT: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnEthernetSegmentRoute {
    pub rd: RouteDistinguisher,
    pub esi: [u8; 10],
    pub originating_router_ip: Ipv4Address,
}

impl EvpnEthernetSegmentRoute {
    pub fn new(rd: RouteDistinguisher, esi: [u8; 10], originating_router_ip: Ipv4Address) -> Self {
        EvpnEthernetSegmentRoute {
            rd,
            esi,
            originating_router_ip,
        }
    }
}

/// DF Election Engine for multi-homed Ethernet Segments
#[derive(Debug, Clone)]
pub struct EvpnDfElectionEngine {
    pub local_router_ip: Ipv4Address,
    pub segment_members: HashMap<[u8; 10], Vec<Ipv4Address>>,
}

impl EvpnDfElectionEngine {
    pub fn new(local_router_ip: Ipv4Address) -> Self {
        EvpnDfElectionEngine {
            local_router_ip,
            segment_members: HashMap::new(),
        }
    }

    /// Registers a PE neighbor discovered via EVPN Type 4 ES Route
    pub fn add_segment_peer(&mut self, esi: [u8; 10], peer_ip: Ipv4Address) {
        let members = self.segment_members.entry(esi).or_default();
        if !members.contains(&peer_ip) {
            members.push(peer_ip);
            members.sort_by_key(|ip| ip.to_u32());
        }
    }

    /// RFC 7432 Section 8.5 DF Election Algorithm: DF_Index = (VLAN_ID) % (PE_Count)
    pub fn is_designated_forwarder(&self, esi: &[u8; 10], vlan_id: u16) -> bool {
        if let Some(members) = self.segment_members.get(esi) {
            if members.is_empty() {
                return true;
            }
            let df_index = (vlan_id as usize) % members.len();
            members.get(df_index) == Some(&self.local_router_ip)
        } else {
            true // Single homed / only local PE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_df_election_round_robin() {
        let local_pe = Ipv4Address::new(192, 168, 1, 10);
        let peer_pe = Ipv4Address::new(192, 168, 1, 20);
        let esi = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let mut engine = EvpnDfElectionEngine::new(local_pe);
        engine.add_segment_peer(esi, local_pe);
        engine.add_segment_peer(esi, peer_pe);

        // Sorted list: [192.168.1.10 (idx 0), 192.168.1.20 (idx 1)]
        // VLAN 100: 100 % 2 = 0 -> local_pe is DF
        assert!(engine.is_designated_forwarder(&esi, 100));

        // VLAN 101: 101 % 2 = 1 -> peer_pe is DF, local_pe is Non-DF
        assert!(!engine.is_designated_forwarder(&esi, 101));
    }
}
