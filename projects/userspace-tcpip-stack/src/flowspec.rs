//! BGP Flowspec (RFC 5575 / RFC 8955).
//!
//! Automated DDoS mitigation, dynamic traffic filtering, and traffic-action policy distribution via BGP (AFI 1 / SAFI 133).

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const BGP_SAFI_FLOWSPEC: u8 = 133;

pub const FLOWSPEC_TYPE_DST_PREFIX: u8 = 1;
pub const FLOWSPEC_TYPE_SRC_PREFIX: u8 = 2;
pub const FLOWSPEC_TYPE_IP_PROTO: u8 = 3;
pub const FLOWSPEC_TYPE_PORT: u8 = 4;
pub const FLOWSPEC_TYPE_DST_PORT: u8 = 5;
pub const FLOWSPEC_TYPE_SRC_PORT: u8 = 6;
pub const FLOWSPEC_TYPE_TCP_FLAGS: u8 = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowspecAction {
    Drop,
    RateLimitBps(u32),
    RedirectIp(Ipv4Address),
    MarkDscp(u8),
}

impl fmt::Display for FlowspecAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowspecAction::Drop => write!(f, "DROP (Rate=0 bps)"),
            FlowspecAction::RateLimitBps(r) => write!(f, "RATE-LIMIT ({} bps)", r),
            FlowspecAction::RedirectIp(ip) => write!(f, "REDIRECT ({})", ip),
            FlowspecAction::MarkDscp(dscp) => write!(f, "MARK-DSCP ({})", dscp),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowspecMatch {
    pub dst_prefix: Option<(Ipv4Address, u8)>,
    pub src_prefix: Option<(Ipv4Address, u8)>,
    pub ip_protocol: Option<u8>,
    pub dst_port: Option<u16>,
    pub src_port: Option<u16>,
    pub tcp_flags: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowspecRule {
    pub id: u32,
    pub match_fields: FlowspecMatch,
    pub action: FlowspecAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowspecDecision {
    Pass,
    Drop,
    RateLimit(u32),
    Redirect(Ipv4Address),
    Mark(u8),
}

#[derive(Debug, Clone, Default)]
pub struct FlowspecEngine {
    pub rules: Vec<FlowspecRule>,
}

impl FlowspecEngine {
    pub fn new() -> Self {
        FlowspecEngine { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: FlowspecRule) {
        self.rules.push(rule);
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }

    pub fn evaluate(
        &self,
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        protocol: u8,
        src_port: Option<u16>,
        dst_port: Option<u16>,
        tcp_flags: Option<u8>,
    ) -> FlowspecDecision {
        for rule in &self.rules {
            let m = &rule.match_fields;

            if let Some((d_ip, d_mask)) = m.dst_prefix
                && !matches_cidr(dst_ip, d_ip, d_mask)
            {
                continue;
            }

            if let Some((s_ip, s_mask)) = m.src_prefix
                && !matches_cidr(src_ip, s_ip, s_mask)
            {
                continue;
            }

            if let Some(proto) = m.ip_protocol
                && proto != protocol
            {
                continue;
            }

            if let Some(dp) = m.dst_port
                && dst_port != Some(dp)
            {
                continue;
            }

            if let Some(sp) = m.src_port
                && src_port != Some(sp)
            {
                continue;
            }

            if let Some(flags) = m.tcp_flags
                && tcp_flags != Some(flags)
            {
                continue;
            }

            // Match succeeded, return action decision
            return match &rule.action {
                FlowspecAction::Drop => FlowspecDecision::Drop,
                FlowspecAction::RateLimitBps(bps) => FlowspecDecision::RateLimit(*bps),
                FlowspecAction::RedirectIp(ip) => FlowspecDecision::Redirect(*ip),
                FlowspecAction::MarkDscp(d) => FlowspecDecision::Mark(*d),
            };
        }

        FlowspecDecision::Pass
    }

    pub fn serialize_rule(&self, rule: &FlowspecRule) -> Vec<u8> {
        let mut buf = Vec::new();

        // Destination Prefix (Type 1)
        if let Some((dst, mask)) = rule.match_fields.dst_prefix {
            buf.push(FLOWSPEC_TYPE_DST_PREFIX);
            buf.push(mask);
            let bytes_to_write = (mask as usize).div_ceil(8);
            buf.extend_from_slice(&dst.0[..bytes_to_write]);
        }

        // Source Prefix (Type 2)
        if let Some((src, mask)) = rule.match_fields.src_prefix {
            buf.push(FLOWSPEC_TYPE_SRC_PREFIX);
            buf.push(mask);
            let bytes_to_write = (mask as usize).div_ceil(8);
            buf.extend_from_slice(&src.0[..bytes_to_write]);
        }

        // IP Protocol (Type 3)
        if let Some(proto) = rule.match_fields.ip_protocol {
            buf.push(FLOWSPEC_TYPE_IP_PROTO);
            buf.push(0x81); // End-of-list + EQ (==)
            buf.push(proto);
        }

        // Destination Port (Type 5)
        if let Some(dp) = rule.match_fields.dst_port {
            buf.push(FLOWSPEC_TYPE_DST_PORT);
            buf.push(0x91); // End-of-list + 2-byte len + EQ
            buf.extend_from_slice(&dp.to_be_bytes());
        }

        buf
    }
}

fn matches_cidr(ip: Ipv4Address, subnet: Ipv4Address, mask_len: u8) -> bool {
    if mask_len == 0 {
        return true;
    }
    if mask_len > 32 {
        return false;
    }
    let mask = !((1u64 << (32 - mask_len)) - 1) as u32;
    (ip.to_u32() & mask) == (subnet.to_u32() & mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flowspec_ddos_mitigation_matching() {
        let mut engine = FlowspecEngine::new();

        // Rule 1: Drop DNS Amplification Attack targeting 192.168.1.100 (UDP Port 53)
        engine.add_rule(FlowspecRule {
            id: 1,
            match_fields: FlowspecMatch {
                dst_prefix: Some((Ipv4Address::new(192, 168, 1, 100), 32)),
                src_prefix: None,
                ip_protocol: Some(17), // UDP
                dst_port: None,
                src_port: Some(53),
                tcp_flags: None,
            },
            action: FlowspecAction::Drop,
        });

        // Rule 2: Rate-limit HTTP SYN flood targeting web farm 192.168.1.0/24
        engine.add_rule(FlowspecRule {
            id: 2,
            match_fields: FlowspecMatch {
                dst_prefix: Some((Ipv4Address::new(192, 168, 1, 0), 24)),
                src_prefix: None,
                ip_protocol: Some(6), // TCP
                dst_port: Some(80),
                src_port: None,
                tcp_flags: Some(0x02), // SYN
            },
            action: FlowspecAction::RateLimitBps(1_000_000),
        });

        // Test Attack 1 (DNS Amp) -> Should DROP
        let d1 = engine.evaluate(
            Ipv4Address::new(8, 8, 8, 8),
            Ipv4Address::new(192, 168, 1, 100),
            17,
            Some(53),
            Some(49152),
            None,
        );
        assert_eq!(d1, FlowspecDecision::Drop);

        // Test Attack 2 (SYN Flood) -> Should Rate-Limit
        let d2 = engine.evaluate(
            Ipv4Address::new(203, 0, 113, 50),
            Ipv4Address::new(192, 168, 1, 10),
            6,
            Some(34567),
            Some(80),
            Some(0x02),
        );
        assert_eq!(d2, FlowspecDecision::RateLimit(1_000_000));

        // Test Normal Traffic -> Should PASS
        let d3 = engine.evaluate(
            Ipv4Address::new(192, 168, 1, 50),
            Ipv4Address::new(192, 168, 1, 10),
            6,
            Some(50000),
            Some(443),
            Some(0x18),
        );
        assert_eq!(d3, FlowspecDecision::Pass);
    }
}
