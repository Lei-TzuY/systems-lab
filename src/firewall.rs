//! Stateful Packet Filter and Firewall Engine (iptables/nftables style).
//!
//! Provides rule matching across INPUT, OUTPUT, and FORWARD chains with CIDR subnet matching,
//! protocol filtering, port range evaluation, and ACCEPT / DROP / REJECT verdicts.

use crate::ipv4::{Ipv4Address, Ipv4Packet, IP_PROTO_TCP, IP_PROTO_UDP};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallChain {
    Input,
    Output,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallAction {
    Accept,
    Drop,
    Reject,
}

impl fmt::Display for FirewallAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FirewallAction::Accept => write!(f, "ACCEPT"),
            FirewallAction::Drop => write!(f, "DROP"),
            FirewallAction::Reject => write!(f, "REJECT"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpCidr {
    pub ip: Ipv4Address,
    pub prefix_len: u8,
}

impl IpCidr {
    pub fn new(ip: Ipv4Address, prefix_len: u8) -> Self {
        IpCidr { ip, prefix_len }
    }

    pub fn matches(&self, target: Ipv4Address) -> bool {
        if self.prefix_len == 0 {
            return true;
        }
        let mask = if self.prefix_len >= 32 {
            !0u32
        } else {
            !0u32 << (32 - self.prefix_len)
        };
        (self.ip.to_u32() & mask) == (target.to_u32() & mask)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FirewallRule {
    pub description: String,
    pub src_cidr: Option<IpCidr>,
    pub dst_cidr: Option<IpCidr>,
    pub protocol: Option<u8>,
    pub src_port_range: Option<(u16, u16)>,
    pub dst_port_range: Option<(u16, u16)>,
    pub action: FirewallAction,
}

impl Default for FirewallAction {
    fn default() -> Self {
        FirewallAction::Accept
    }
}

impl FirewallRule {
    pub fn matches(&self, packet: &Ipv4Packet<'_>) -> bool {
        // 1. Source IP check
        if let Some(ref src_cidr) = self.src_cidr {
            if !src_cidr.matches(packet.header.src_ip) {
                return false;
            }
        }

        // 2. Destination IP check
        if let Some(ref dst_cidr) = self.dst_cidr {
            if !dst_cidr.matches(packet.header.dst_ip) {
                return false;
            }
        }

        // 3. Protocol check
        if let Some(proto) = self.protocol {
            if packet.header.protocol.to_u8() != proto {
                return false;
            }
        }

        // 4. Port ranges (TCP/UDP)
        if self.src_port_range.is_some() || self.dst_port_range.is_some() {
            let proto = packet.header.protocol.to_u8();
            if proto == IP_PROTO_TCP || proto == IP_PROTO_UDP {
                if packet.payload.len() < 4 {
                    return false;
                }
                let src_port = u16::from_be_bytes([packet.payload[0], packet.payload[1]]);
                let dst_port = u16::from_be_bytes([packet.payload[2], packet.payload[3]]);

                if let Some((min_sp, max_sp)) = self.src_port_range {
                    if src_port < min_sp || src_port > max_sp {
                        return false;
                    }
                }

                if let Some((min_dp, max_dp)) = self.dst_port_range {
                    if dst_port < min_dp || dst_port > max_dp {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct Firewall {
    pub input_rules: Vec<FirewallRule>,
    pub output_rules: Vec<FirewallRule>,
    pub forward_rules: Vec<FirewallRule>,
    pub default_input_policy: FirewallAction,
    pub default_output_policy: FirewallAction,
    pub default_forward_policy: FirewallAction,
}

impl Default for Firewall {
    fn default() -> Self {
        Self::new()
    }
}

impl Firewall {
    pub fn new() -> Self {
        Firewall {
            input_rules: Vec::new(),
            output_rules: Vec::new(),
            forward_rules: Vec::new(),
            default_input_policy: FirewallAction::Accept,
            default_output_policy: FirewallAction::Accept,
            default_forward_policy: FirewallAction::Accept,
        }
    }

    pub fn add_rule(&mut self, chain: FirewallChain, rule: FirewallRule) {
        match chain {
            FirewallChain::Input => self.input_rules.push(rule),
            FirewallChain::Output => self.output_rules.push(rule),
            FirewallChain::Forward => self.forward_rules.push(rule),
        }
    }

    pub fn flush_chain(&mut self, chain: FirewallChain) {
        match chain {
            FirewallChain::Input => self.input_rules.clear(),
            FirewallChain::Output => self.output_rules.clear(),
            FirewallChain::Forward => self.forward_rules.clear(),
        }
    }

    /// Evaluates an incoming or outgoing IPv4 packet against the specified chain rules.
    pub fn evaluate(&self, chain: FirewallChain, packet: &Ipv4Packet<'_>) -> FirewallAction {
        let (rules, default_policy) = match chain {
            FirewallChain::Input => (&self.input_rules, self.default_input_policy),
            FirewallChain::Output => (&self.output_rules, self.default_output_policy),
            FirewallChain::Forward => (&self.forward_rules, self.default_forward_policy),
        };

        for rule in rules {
            if rule.matches(packet) {
                return rule.action;
            }
        }

        default_policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipv4::IP_PROTO_ICMP;

    #[test]
    fn test_firewall_rule_matching_and_drop() {
        let mut fw = Firewall::new();

        // Block all ICMP from 10.0.0.0/8
        fw.add_rule(
            FirewallChain::Input,
            FirewallRule {
                description: "Block 10.0.0.0/8 Ping".to_string(),
                src_cidr: Some(IpCidr::new(Ipv4Address::new(10, 0, 0, 0), 8)),
                protocol: Some(IP_PROTO_ICMP),
                action: FirewallAction::Drop,
                ..Default::default()
            },
        );

        // Allow all TCP to port 80
        fw.add_rule(
            FirewallChain::Input,
            FirewallRule {
                description: "Allow HTTP".to_string(),
                protocol: Some(IP_PROTO_TCP),
                dst_port_range: Some((80, 80)),
                action: FirewallAction::Accept,
                ..Default::default()
            },
        );

        // 1. Packet from 10.1.2.3 ICMP -> should DROP
        let raw_icmp = Ipv4Packet::serialize(
            Ipv4Address::new(10, 1, 2, 3),
            Ipv4Address::new(192, 168, 1, 1),
            IP_PROTO_ICMP,
            1,
            64,
            &[8, 0, 0, 0],
        );
        let pkt_icmp = Ipv4Packet::parse(&raw_icmp, false).unwrap();
        assert_eq!(fw.evaluate(FirewallChain::Input, &pkt_icmp), FirewallAction::Drop);

        // 2. Packet from 192.168.1.100 ICMP -> should ACCEPT (default policy)
        let raw_icmp2 = Ipv4Packet::serialize(
            Ipv4Address::new(192, 168, 1, 100),
            Ipv4Address::new(192, 168, 1, 1),
            IP_PROTO_ICMP,
            2,
            64,
            &[8, 0, 0, 0],
        );
        let pkt_icmp2 = Ipv4Packet::parse(&raw_icmp2, false).unwrap();
        assert_eq!(fw.evaluate(FirewallChain::Input, &pkt_icmp2), FirewallAction::Accept);
    }
}
