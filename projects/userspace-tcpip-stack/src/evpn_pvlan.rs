//! EVPN Layer 2 Private VLAN (PVLAN) & Port Isolation Engine (RFC 7432 / RFC 5517).
//!
//! In multi-tenant datacenters and Zero-Trust networks, Private VLANs (PVLAN) partition
//! a broadcast domain (Primary VLAN) into isolated micro-segments (Secondary VLANs):
//!
//! 1. **Promiscuous (P-Port)**: Can talk to ALL ports (Promiscuous, Isolated, Community).
//!    Usually the default gateway or core switch uplink.
//! 2. **Isolated (I-Port)**: Can ONLY talk to Promiscuous ports. Cannot talk to other Isolated
//!    or Community ports (Layer 2 micro-segmentation).
//! 3. **Community (C-Port)**: Can talk to Promiscuous ports and ports within the SAME Community group.
//!
//! This module implements:
//! * PVLAN port type classification (Promiscuous, Isolated, Community).
//! * Layer 2 inter-port forwarding decision matrix.
//! * Interface-to-PVLAN role mapping and security audit logging.

use std::collections::HashMap;

/// Private VLAN Port Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PvlanPortType {
    Promiscuous,
    Isolated,
    Community(u32),
}

/// EVPN Private VLAN (PVLAN) Port Isolation Engine.
#[derive(Debug, Clone)]
pub struct EvpnPvlanEngine {
    pub primary_vni: u32,
    pub port_roles: HashMap<String, PvlanPortType>,
    pub total_allowed_frames: u64,
    pub total_blocked_frames: u64,
}

impl EvpnPvlanEngine {
    pub fn new(primary_vni: u32) -> Self {
        EvpnPvlanEngine {
            primary_vni,
            port_roles: HashMap::new(),
            total_allowed_frames: 0,
            total_blocked_frames: 0,
        }
    }

    pub fn register_port(&mut self, iface: &str, role: PvlanPortType) {
        self.port_roles.insert(iface.to_string(), role);
    }

    /// Evaluates whether a frame from `ingress_port` is permitted to egress on `egress_port`.
    pub fn can_forward(&mut self, ingress_port: &str, egress_port: &str) -> bool {
        if ingress_port == egress_port {
            return false;
        }

        let in_role = match self.port_roles.get(ingress_port) {
            Some(r) => *r,
            None => return false,
        };

        let out_role = match self.port_roles.get(egress_port) {
            Some(r) => *r,
            None => return false,
        };

        let allowed = match (in_role, out_role) {
            // Promiscuous can communicate with any port
            (PvlanPortType::Promiscuous, _) | (_, PvlanPortType::Promiscuous) => true,

            // Isolated cannot communicate with other Isolated or Community
            (PvlanPortType::Isolated, PvlanPortType::Isolated) => false,
            (PvlanPortType::Isolated, PvlanPortType::Community(_)) => false,
            (PvlanPortType::Community(_), PvlanPortType::Isolated) => false,

            // Community can only communicate within the same community ID
            (PvlanPortType::Community(c1), PvlanPortType::Community(c2)) => c1 == c2,
        };

        if allowed {
            self.total_allowed_frames += 1;
        } else {
            self.total_blocked_frames += 1;
        }

        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evpn_pvlan_forwarding_matrix() {
        let mut pvlan = EvpnPvlanEngine::new(100);

        pvlan.register_port("gw_port", PvlanPortType::Promiscuous);
        pvlan.register_port("vm_iso_1", PvlanPortType::Isolated);
        pvlan.register_port("vm_iso_2", PvlanPortType::Isolated);
        pvlan.register_port("vm_comm_a1", PvlanPortType::Community(10));
        pvlan.register_port("vm_comm_a2", PvlanPortType::Community(10));
        pvlan.register_port("vm_comm_b1", PvlanPortType::Community(20));

        // 1. Isolated -> Gateway: Allowed
        assert!(pvlan.can_forward("vm_iso_1", "gw_port"));

        // 2. Gateway -> Isolated: Allowed
        assert!(pvlan.can_forward("gw_port", "vm_iso_1"));

        // 3. Isolated -> Isolated: Blocked!
        assert!(!pvlan.can_forward("vm_iso_1", "vm_iso_2"));

        // 4. Isolated -> Community: Blocked!
        assert!(!pvlan.can_forward("vm_iso_1", "vm_comm_a1"));

        // 5. Community 10 -> Community 10: Allowed!
        assert!(pvlan.can_forward("vm_comm_a1", "vm_comm_a2"));

        // 6. Community 10 -> Community 20: Blocked!
        assert!(!pvlan.can_forward("vm_comm_a1", "vm_comm_b1"));
    }
}
