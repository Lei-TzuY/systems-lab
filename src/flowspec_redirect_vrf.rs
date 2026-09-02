//! BGP Flowspec Redirect to VRF & DSCP Traffic Marking (RFC 5575 / RFC 8955 Section 7.2).
//!
//! Implements BGP Flowspec Extended Community Type 0x80 Subtype 0x08 (Redirect to VRF / Route Target),
//! Subtype 0x09 (Traffic Marking / DSCP Remarking), and automated DDoS traffic diversion
//! into dedicated scrubbing VRFs without dropping legitimate traffic.

use crate::ipv4::Ipv4Address;

/// Flowspec Traffic Action Extended Community Subtypes (RFC 8955).
/// Flowspec Traffic Action Extended Community Subtypes (RFC 8955).
pub const FLOWSPEC_ACTION_TRAFFIC_RATE: u8 = 0x06;
pub const FLOWSPEC_ACTION_TRAFFIC_ACTION: u8 = 0x07;
pub const FLOWSPEC_ACTION_REDIRECT_VRF: u8 = 0x08;
pub const FLOWSPEC_ACTION_TRAFFIC_MARKING: u8 = 0x09;

/// TCP Flag Constants (RFC 8955 Section 4.2.9).
pub const TCP_FLAG_FIN: u8 = 0x01;
pub const TCP_FLAG_SYN: u8 = 0x02;
pub const TCP_FLAG_RST: u8 = 0x04;
pub const TCP_FLAG_PSH: u8 = 0x08;
pub const TCP_FLAG_ACK: u8 = 0x10;
pub const TCP_FLAG_URG: u8 = 0x20;

/// Port Matching Condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortRangeMatch {
    Exact(u16),
    Range(u16, u16), // inclusive [min, max]
    LessThan(u16),
    GreaterThan(u16),
}

impl PortRangeMatch {
    pub fn matches(&self, port: u16) -> bool {
        match self {
            PortRangeMatch::Exact(p) => port == *p,
            PortRangeMatch::Range(min, max) => port >= *min && port <= *max,
            PortRangeMatch::LessThan(val) => port < *val,
            PortRangeMatch::GreaterThan(val) => port > *val,
        }
    }
}

/// TCP Flags Matching Condition with bitmask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlagsMatch {
    pub mask: u8,
    pub expected: u8,
}

impl TcpFlagsMatch {
    pub fn new(mask: u8, expected: u8) -> Self {
        TcpFlagsMatch { mask, expected }
    }

    pub fn matches(&self, actual_flags: u8) -> bool {
        (actual_flags & self.mask) == self.expected
    }
}

/// Packet Length Matching Condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketLengthMatch {
    pub min_bytes: u16,
    pub max_bytes: u16,
}

impl PacketLengthMatch {
    pub fn new(min_bytes: u16, max_bytes: u16) -> Self {
        PacketLengthMatch {
            min_bytes,
            max_bytes,
        }
    }

    pub fn matches(&self, len: u16) -> bool {
        len >= self.min_bytes && len <= self.max_bytes
    }
}

/// Flowspec Traffic Action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowspecVrfAction {
    Pass,
    Drop,
    RedirectVrf(String),         // VRF Name or Route Target
    RemarkDscp(u8),              // 6-bit DSCP value (0..63)
    RateLimitBytesPerSec(u64),   // RFC 8955 Section 7.1
    RedirectAndRemark { vrf: String, dscp: u8 },
    SampleAndMirror(String),     // Sampling / Mirroring target
}

/// Flowspec Filtering and Action Rule (Basic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowspecVrfRule {
    pub rule_id: u32,
    pub match_dst_ip: Option<Ipv4Address>,
    pub match_protocol: Option<u8>,
    pub match_dst_port: Option<u16>,
    pub action: FlowspecVrfAction,
}

/// Flowspec Advanced Rule supporting Port Ranges, TCP Flags, Length, and Source IP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowspecVrfAdvancedRule {
    pub rule_id: u32,
    pub match_src_ip: Option<Ipv4Address>,
    pub match_dst_ip: Option<Ipv4Address>,
    pub match_protocol: Option<u8>,
    pub match_src_port: Option<PortRangeMatch>,
    pub match_dst_port: Option<PortRangeMatch>,
    pub match_tcp_flags: Option<TcpFlagsMatch>,
    pub match_packet_len: Option<PacketLengthMatch>,
    pub action: FlowspecVrfAction,
}

impl FlowspecVrfAdvancedRule {
    pub fn new(rule_id: u32, action: FlowspecVrfAction) -> Self {
        FlowspecVrfAdvancedRule {
            rule_id,
            match_src_ip: None,
            match_dst_ip: None,
            match_protocol: None,
            match_src_port: None,
            match_dst_port: None,
            match_tcp_flags: None,
            match_packet_len: None,
            action,
        }
    }

    pub fn matches(
        &self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        protocol: u8,
        src_port: u16,
        dst_port: u16,
        tcp_flags: u8,
        packet_len: u16,
    ) -> bool {
        if let Some(sip) = self.match_src_ip {
            if sip != src_ip {
                return false;
            }
        }
        if let Some(dip) = self.match_dst_ip {
            if dip != dst_ip {
                return false;
            }
        }
        if let Some(proto) = self.match_protocol {
            if proto != protocol {
                return false;
            }
        }
        if let Some(ref sp) = self.match_src_port {
            if !sp.matches(src_port) {
                return false;
            }
        }
        if let Some(ref dp) = self.match_dst_port {
            if !dp.matches(dst_port) {
                return false;
            }
        }
        if let Some(ref tf) = self.match_tcp_flags {
            if !tf.matches(tcp_flags) {
                return false;
            }
        }
        if let Some(ref pl) = self.match_packet_len {
            if !pl.matches(packet_len) {
                return false;
            }
        }
        true
    }
}

/// Flowspec VRF Scrubbing & Traffic Marking Engine.
#[derive(Debug, Clone, Default)]
pub struct FlowspecVrfScrubbingEngine {
    pub rules: Vec<FlowspecVrfRule>,
    pub advanced_rules: Vec<FlowspecVrfAdvancedRule>,
    pub redirected_packets_count: usize,
    pub remarked_packets_count: usize,
    pub dropped_packets_count: usize,
    pub rate_limited_packets_count: usize,
    pub sampled_packets_count: usize,
    pub passed_packets_count: usize,
    pub total_bytes_diverted: u64,
}

impl FlowspecVrfScrubbingEngine {
    pub fn new() -> Self {
        FlowspecVrfScrubbingEngine {
            rules: Vec::new(),
            advanced_rules: Vec::new(),
            redirected_packets_count: 0,
            remarked_packets_count: 0,
            dropped_packets_count: 0,
            rate_limited_packets_count: 0,
            sampled_packets_count: 0,
            passed_packets_count: 0,
            total_bytes_diverted: 0,
        }
    }

    /// Adds a basic Flowspec filter rule.
    pub fn add_rule(&mut self, rule: FlowspecVrfRule) {
        self.rules.push(rule);
    }

    /// Adds an advanced Flowspec filter rule.
    pub fn add_advanced_rule(&mut self, rule: FlowspecVrfAdvancedRule) {
        self.advanced_rules.push(rule);
    }

    /// Evaluates an incoming packet against the basic Flowspec rule set (legacy API).
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
                FlowspecVrfAction::Drop => {
                    self.dropped_packets_count += 1;
                }
                FlowspecVrfAction::RateLimitBytesPerSec(_) => {
                    self.rate_limited_packets_count += 1;
                }
                FlowspecVrfAction::RedirectAndRemark { .. } => {
                    self.redirected_packets_count += 1;
                    self.remarked_packets_count += 1;
                }
                FlowspecVrfAction::SampleAndMirror(_) => {
                    self.sampled_packets_count += 1;
                }
                FlowspecVrfAction::Pass => {}
            }
            return r.action.clone();
        }

        self.passed_packets_count += 1;
        FlowspecVrfAction::Pass
    }

    /// Evaluates an incoming packet against both advanced and basic Flowspec rules with full 5-tuple, TCP flags, and packet length.
    pub fn evaluate_packet_advanced(
        &mut self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        protocol: u8,
        src_port: u16,
        dst_port: u16,
        tcp_flags: u8,
        packet_len: u16,
    ) -> FlowspecVrfAction {
        // 1. Evaluate advanced rules first (priority)
        for r in &self.advanced_rules {
            if r.matches(
                src_ip,
                dst_ip,
                protocol,
                src_port,
                dst_port,
                tcp_flags,
                packet_len,
            ) {
                match &r.action {
                    FlowspecVrfAction::RedirectVrf(_) => {
                        self.redirected_packets_count += 1;
                        self.total_bytes_diverted += packet_len as u64;
                    }
                    FlowspecVrfAction::RemarkDscp(_) => {
                        self.remarked_packets_count += 1;
                    }
                    FlowspecVrfAction::Drop => {
                        self.dropped_packets_count += 1;
                    }
                    FlowspecVrfAction::RateLimitBytesPerSec(_) => {
                        self.rate_limited_packets_count += 1;
                    }
                    FlowspecVrfAction::RedirectAndRemark { .. } => {
                        self.redirected_packets_count += 1;
                        self.remarked_packets_count += 1;
                        self.total_bytes_diverted += packet_len as u64;
                    }
                    FlowspecVrfAction::SampleAndMirror(_) => {
                        self.sampled_packets_count += 1;
                    }
                    FlowspecVrfAction::Pass => {}
                }
                return r.action.clone();
            }
        }

        // 2. Fall back to basic rules evaluation
        self.evaluate_packet(dst_ip, protocol, dst_port)
    }
}

