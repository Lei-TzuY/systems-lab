//! 5G N4 / PFCP Protocol (Packet Forwarding Control Protocol - 3GPP TS 29.244 / UDP 8805).
//!
//! Implements SMF <-> UPF control plane interface for PDR (Packet Detection Rule) and
//! FAR (Forwarding Action Rule) programming, Session Establishment, and Association Setup.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

pub const PFCP_UDP_PORT: u16 = 8805;

pub const PFCP_MSG_HEARTBEAT_REQUEST: u8 = 1;
pub const PFCP_MSG_HEARTBEAT_RESPONSE: u8 = 2;
pub const PFCP_MSG_ASSOCIATION_SETUP_REQUEST: u8 = 5;
pub const PFCP_MSG_ASSOCIATION_SETUP_RESPONSE: u8 = 6;
pub const PFCP_MSG_SESSION_ESTABLISHMENT_REQUEST: u8 = 50;
pub const PFCP_MSG_SESSION_ESTABLISHMENT_RESPONSE: u8 = 51;

pub const PFCP_SRC_INTERFACE_ACCESS: u8 = 0;
pub const PFCP_SRC_INTERFACE_CORE: u8 = 1;
pub const PFCP_APPLY_ACTION_FORWARD: u8 = 0x02;
pub const PFCP_APPLY_ACTION_DROP: u8 = 0x01;

/// Packet Detection Rule (PDR)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketDetectionRule {
    pub pdr_id: u16,
    pub precedence: u32,
    pub source_interface: u8,
    pub teid: Option<u32>,
    pub ue_ip: Option<Ipv4Address>,
}

/// Forwarding Action Rule (FAR)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardingActionRule {
    pub far_id: u16,
    pub apply_action: u8,
    pub destination_interface: u8,
    pub outer_header_creation: Option<(u32, Ipv4Address)>, // (GTP-U TEID, Peer IP)
}

/// PFCP PDU Session Entry on UPF
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PfcpSession {
    pub cp_seid: u64,
    pub up_seid: u64,
    pub pdrs: Vec<PacketDetectionRule>,
    pub fars: Vec<ForwardingActionRule>,
}

/// 5G N4 PFCP Node (UPF / SMF Control Plane Simulator)
#[derive(Debug, Clone, Default)]
pub struct PfcpNode {
    pub node_id: String,
    pub is_associated: bool,
    pub sessions: HashMap<u64, PfcpSession>, // Keyed by UP F-SEID
    pub session_counter: u64,
}

impl PfcpNode {
    pub fn new(node_id: &str) -> Self {
        PfcpNode {
            node_id: node_id.to_string(),
            is_associated: false,
            sessions: HashMap::new(),
            session_counter: 100,
        }
    }

    /// Handles PFCP Association Setup Request
    pub fn handle_association_setup(&mut self, peer_node_id: &str) -> bool {
        self.is_associated = true;
        !peer_node_id.is_empty()
    }

    /// Establishes a new PFCP Session on the UPF
    pub fn establish_session(
        &mut self,
        cp_seid: u64,
        pdrs: Vec<PacketDetectionRule>,
        fars: Vec<ForwardingActionRule>,
    ) -> u64 {
        self.session_counter += 1;
        let up_seid = self.session_counter;

        let session = PfcpSession {
            cp_seid,
            up_seid,
            pdrs,
            fars,
        };

        self.sessions.insert(up_seid, session);
        up_seid
    }

    /// Matches an incoming GTP-U packet against installed PDRs and returns the Forwarding Action
    pub fn match_and_forward(&self, up_seid: u64, teid: u32) -> Option<&ForwardingActionRule> {
        let session = self.sessions.get(&up_seid)?;
        let matched_pdr = session.pdrs.iter().find(|p| p.teid == Some(teid))?;
        session.fars.iter().find(|f| f.far_id == matched_pdr.pdr_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pfcp_constants_and_association() {
        assert_eq!(PFCP_UDP_PORT, 8805);
        assert_eq!(PFCP_MSG_SESSION_ESTABLISHMENT_REQUEST, 50);

        let mut upf = PfcpNode::new("upf-edge-01.5g.local");
        assert!(!upf.is_associated);
        assert!(upf.handle_association_setup("smf-control-01.5g.local"));
        assert!(upf.is_associated);
    }

    #[test]
    fn test_pfcp_session_establishment_and_forwarding_match() {
        let mut upf = PfcpNode::new("upf-core-01");
        upf.handle_association_setup("smf-01");

        let pdr_uplink = PacketDetectionRule {
            pdr_id: 1,
            precedence: 100,
            source_interface: PFCP_SRC_INTERFACE_ACCESS,
            teid: Some(0x10001),
            ue_ip: Some(Ipv4Address::new(10, 45, 0, 10)),
        };

        let far_uplink = ForwardingActionRule {
            far_id: 1,
            apply_action: PFCP_APPLY_ACTION_FORWARD,
            destination_interface: PFCP_SRC_INTERFACE_CORE,
            outer_header_creation: None, // Decapsulate and forward to Data Network (DN)
        };

        let cp_seid = 0xAAAA_BBBB;
        let up_seid = upf.establish_session(cp_seid, vec![pdr_uplink], vec![far_uplink]);

        assert_eq!(upf.sessions.len(), 1);
        let action = upf.match_and_forward(up_seid, 0x10001).unwrap();
        assert_eq!(action.apply_action, PFCP_APPLY_ACTION_FORWARD);
        assert_eq!(action.destination_interface, PFCP_SRC_INTERFACE_CORE);
    }
}
