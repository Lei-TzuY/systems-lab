//! BGP Flowspec Redirect to VRF & DSCP Traffic Marking (RFC 5575 / RFC 8955 Section 7.2).
//!
//! Implements BGP Flowspec Extended Community Type 0x80 Subtype 0x08 (Redirect to VRF / Route Target),
//! Subtype 0x09 (Traffic Marking / DSCP Remarking), and automated DDoS traffic diversion
//! into dedicated scrubbing VRFs without dropping legitimate traffic.

use crate::ipv4::Ipv4Address;

/// Flowspec Traffic Action Extended Community Subtypes (RFC 8955).
pub const FLOWSPEC_ACTION_REDIRECT_VRF: u8 = 0x08;
pub const FLOWSPEC_ACTION_TRAFFIC_MARKING: u8 = 0x09;

/// Flowspec Traffic Action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowspecVrfAction {
    Pass,
    Drop,
    RedirectVrf(String), // VRF Name or Route Target
    RemarkDscp(u8),      // 6-bit DSCP value (0..63)
}

/// Flowspec Filtering and Action Rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowspecVrfRule {
    pub rule_id: u32,
    pub match_dst_ip: Option<Ipv4Address>,
    pub match_protocol: Option<u8>,
    pub match_dst_port: Option<u16>,
    pub action: FlowspecVrfAction,
}

/// Flowspec VRF Scrubbing & Traffic Marking Engine.
#[derive(Debug, Clone, Default)]
pub struct FlowspecVrfScrubbingEngine {
    pub rules: Vec<FlowspecVrfRule>,
    pub redirected_packets_count: usize,
    pub remarked_packets_count: usize,
    pub passed_packets_count: usize,
}

impl FlowspecVrfScrubbingEngine {
    pub fn new() -> Self {
        FlowspecVrfScrubbingEngine {
            rules: Vec::new(),
            redirected_packets_count: 0,
            remarked_packets_count: 0,
            passed_packets_count: 0,
        }
    }

    /// Adds a Flowspec filter rule.
    pub fn add_rule(&mut self, rule: FlowspecVrfRule) {
        self.rules.push(rule);
    }

    /// Evaluates an incoming packet against the Flowspec rule set.
    pub fn evaluate_packet(
        &mut self,
        dst_ip: Ipv4Address,
        protocol: u8,
        dst_port: u16,
    ) -> FlowspecVrfAction {
        for r in &self.rules {
            if let Some(dip) = r.match_dst_ip {
                if dip != dst_ip {
                    continue;
                }
            }
            if let Some(proto) = r.match_protocol {
                if proto != protocol {
                    continue;
                }
            }
            if let Some(port) = r.match_dst_port {
                if port != dst_port {
                    continue;
                }
            }

            match &r.action {
                FlowspecVrfAction::RedirectVrf(_) => {
                    self.redirected_packets_count += 1;
                }
                FlowspecVrfAction::RemarkDscp(_) => {
                    self.remarked_packets_count += 1;
                }
                _ => {}
            }
            return r.action.clone();
        }

        self.passed_packets_count += 1;
        FlowspecVrfAction::Pass
    }
}
