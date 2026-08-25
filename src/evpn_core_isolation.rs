//! EVPN Layer 2 Core Isolation & Split-Horizon Filtering (RFC 7432 Section 8.4 / RFC 8365).
//!
//! In EVPN all-active multi-homing topologies:
//! 1. **Core Isolation Defense**: When a Leaf PE loses all spine/core underlay uplinks,
//!    it must immediately shut down / isolate its client-facing Attachment Circuits (ACs).
//!    This forces dual-homed Customer Edges (CE) to divert all traffic to the surviving Leaf PE,
//!    preventing complete blackholing.
//! 2. **Split-Horizon Filtering**: When a PE receives a BUM frame from a multi-homed CE
//!    and replicates it over the core to other PEs, the receiving PEs must identify
//!    the source Ethernet Segment Identifier (ESI) and suppress transmission on any local
//!    interface connected to the same ESI, eliminating loops.
//!
//! This module implements:
//! * Core uplink link-state tracker & automated AC port isolation state machine.
//! * Split-horizon ESI label / identifier filter.
//! * Auto-recovery when underlay core uplinks restore.

use std::collections::HashSet;

/// Core Uplink Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreIsolationState {
    Normal,
    CoreIsolated,
}

/// EVPN Core Isolation & Split-Horizon Engine.
#[derive(Debug, Clone)]
pub struct EvpnCoreIsolationEngine {
    pub local_leaf_id: u32,
    /// Set of active underlay spine/core uplinks (e.g. "eth1", "eth2")
    pub active_core_uplinks: HashSet<String>,
    pub client_attachment_circuits: HashSet<String>,
    pub state: CoreIsolationState,
    /// ESI of local multi-homed segments: Interface -> ESI ID
    pub interface_to_esi: std::collections::HashMap<String, u64>,
    pub total_core_isolation_events: u64,
    pub total_split_horizon_drops: u64,
}

impl EvpnCoreIsolationEngine {
    pub fn new(local_leaf_id: u32) -> Self {
        EvpnCoreIsolationEngine {
            local_leaf_id,
            active_core_uplinks: HashSet::new(),
            client_attachment_circuits: HashSet::new(),
            state: CoreIsolationState::Normal,
            interface_to_esi: std::collections::HashMap::new(),
            total_core_isolation_events: 0,
            total_split_horizon_drops: 0,
        }
    }

    pub fn add_core_uplink(&mut self, iface: &str) {
        self.active_core_uplinks.insert(iface.to_string());
        self.evaluate_core_state();
    }

    pub fn remove_core_uplink(&mut self, iface: &str) {
        self.active_core_uplinks.remove(iface);
        self.evaluate_core_state();
    }

    pub fn register_client_ac(&mut self, iface: &str, esi: Option<u64>) {
        self.client_attachment_circuits.insert(iface.to_string());
        if let Some(e) = esi {
            self.interface_to_esi.insert(iface.to_string(), e);
        }
    }

    fn evaluate_core_state(&mut self) {
        if self.active_core_uplinks.is_empty() {
            if self.state == CoreIsolationState::Normal {
                self.state = CoreIsolationState::CoreIsolated;
                self.total_core_isolation_events += 1;
            }
        } else {
            self.state = CoreIsolationState::Normal;
        }
    }

    /// Evaluates if an egress transmission to a client AC should be allowed or blocked
    /// based on Core Isolation and Split-Horizon filtering.
    pub fn should_forward_to_ac(&mut self, client_iface: &str, source_esi: Option<u64>) -> bool {
        // 1. If in Core Isolation, all local client ACs are blocked
        if self.state == CoreIsolationState::CoreIsolated {
            return false;
        }

        // 2. Split-Horizon filtering: if packet originated from the same ESI, DROP!
        if let Some(src_esi) = source_esi {
            if let Some(&local_esi) = self.interface_to_esi.get(client_iface) {
                if src_esi == local_esi && src_esi != 0 {
                    self.total_split_horizon_drops += 1;
                    return false; // Split-horizon loop suppression!
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_core_isolation_and_split_horizon() {
        let mut leaf = EvpnCoreIsolationEngine::new(1);

        leaf.add_core_uplink("spine1");
        leaf.add_core_uplink("spine2");
        leaf.register_client_ac("eth_ce1", Some(0x0011223344556677));
        leaf.register_client_ac("eth_ce2", Some(0x00AABBCCDDEEFF00));

        assert_eq!(leaf.state, CoreIsolationState::Normal);

        // Forward to CE1 when source ESI is from a different segment -> Allowed
        assert!(leaf.should_forward_to_ac("eth_ce1", Some(0x00AABBCCDDEEFF00)));

        // Forward to CE1 when source ESI is the SAME ESI (0x0011223344556677) -> Split-Horizon Drop!
        assert!(!leaf.should_forward_to_ac("eth_ce1", Some(0x0011223344556677)));
        assert_eq!(leaf.total_split_horizon_drops, 1);

        // Core Uplinks fail: spine1 down, spine2 down
        leaf.remove_core_uplink("spine1");
        assert_eq!(leaf.state, CoreIsolationState::Normal);
        leaf.remove_core_uplink("spine2");
        assert_eq!(leaf.state, CoreIsolationState::CoreIsolated);

        // Now in Core Isolation: all client AC transmissions are suppressed!
        assert!(!leaf.should_forward_to_ac("eth_ce2", None));
        assert_eq!(leaf.total_core_isolation_events, 1);

        // Core uplinks recover
        leaf.add_core_uplink("spine1");
        assert_eq!(leaf.state, CoreIsolationState::Normal);
        assert!(leaf.should_forward_to_ac("eth_ce2", None));
    }
}
