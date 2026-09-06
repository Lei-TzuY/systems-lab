//! EVPN Layer 2 IGMP Snooping & Multicast Forwarding Pruning (RFC 9251 Section 5 & 6).
//!
//! Implements local IGMP membership report snooping on EVPN bridge ports, mapping (VNI, Group IP)
//! to active receiver ports, triggering Route Type 7/8 synchronization, and pruning
//! multicast traffic from ports without active subscribers to eliminate unnecessary BUM flooding.

use crate::ipv4::Ipv4Address;
use std::collections::{HashMap, HashSet};

/// Multicast Forwarding Action for an EVPN Bridge Port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MulticastForwardingAction {
    ForwardToPorts(Vec<u32>),
    PrunedNoReceivers,
}

/// EVPN IGMP Snooping and Multicast Pruning Engine (RFC 9251).
#[derive(Debug, Clone, Default)]
pub struct EvpnIgmpSnoopingEngine {
    pub group_memberships: HashMap<(u32, Ipv4Address), HashSet<u32>>, // (VNI, Group IP) -> Set of Port IDs
    pub join_events_count: usize,
    pub leave_events_count: usize,
    pub pruned_packets_count: usize,
    pub forwarded_packets_count: usize,
}

impl EvpnIgmpSnoopingEngine {
    pub fn new() -> Self {
        EvpnIgmpSnoopingEngine {
            group_memberships: HashMap::new(),
            join_events_count: 0,
            leave_events_count: 0,
            pruned_packets_count: 0,
            forwarded_packets_count: 0,
        }
    }

    /// Handles a local IGMP Join on an access bridge port within a VNI.
    pub fn process_igmp_join(&mut self, vni: u32, port_id: u32, group_ip: Ipv4Address) -> bool {
        self.join_events_count += 1;
        let ports = self.group_memberships.entry((vni, group_ip)).or_default();
        ports.insert(port_id)
    }

    /// Handles a local IGMP Leave on an access bridge port within a VNI.
    pub fn process_igmp_leave(&mut self, vni: u32, port_id: u32, group_ip: Ipv4Address) -> bool {
        self.leave_events_count += 1;
        if let Some(ports) = self.group_memberships.get_mut(&(vni, group_ip)) {
            let removed = ports.remove(&port_id);
            if ports.is_empty() {
                self.group_memberships.remove(&(vni, group_ip));
            }
            removed
        } else {
            false
        }
    }

    /// Evaluates an incoming multicast packet and returns the pruned target port list.
    pub fn evaluate_multicast_forwarding(
        &mut self,
        vni: u32,
        group_ip: Ipv4Address,
    ) -> MulticastForwardingAction {
        if let Some(ports) = self.group_memberships.get(&(vni, group_ip)) {
            if !ports.is_empty() {
                self.forwarded_packets_count += 1;
                let mut list: Vec<u32> = ports.iter().copied().collect();
                list.sort();
                return MulticastForwardingAction::ForwardToPorts(list);
            }
        }
        self.pruned_packets_count += 1;
        MulticastForwardingAction::PrunedNoReceivers
    }
}
