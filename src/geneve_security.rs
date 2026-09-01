//! Geneve Micro-segmentation & Group-Based Policy (GBP / SGT) (RFC 8926).
//!
//! Provides zero-trust overlay security, endpoint classification, and micro-segmentation
//! tagging across Geneve virtual networks using Geneve TLV Option headers (Option Class 0x0108).
//!
//! Features:
//! - Security Group Tag (SGT) and Destination Group ID encapsulation in Geneve TLV options.
//! - Micro-segmentation Matrix Policy Engine:
//!   - Evaluates `(Src SGT, Dst SGT, IP Protocol, Destination Port)` against zero-trust rules.
//!   - Supports `Allow`, `Deny`, `LogOnly`, and `RateLimitBps` actions with telemetry hit counters.
//! - Seamless interoperation with Geneve tunnels (`src/geneve.rs`).

use std::fmt;

pub const GENEVE_OPT_CLASS_GBP: u16 = 0x0108;
pub const GENEVE_OPT_TYPE_GBP: u8 = 0x01;

pub const GBP_FLAG_APPLY_POLICY: u8 = 0x80;
pub const GBP_FLAG_REDIRECT: u8 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SecurityGroupTag {
    pub src_sgt: u16,
    pub dst_sgt: u16,
    pub flags: u8,
    pub tenant_id: u32,
}

impl SecurityGroupTag {
    pub fn new(src_sgt: u16, dst_sgt: u16, tenant_id: u32) -> Self {
        SecurityGroupTag {
            src_sgt,
            dst_sgt,
            flags: GBP_FLAG_APPLY_POLICY,
            tenant_id,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(&self.src_sgt.to_be_bytes());
        buf.extend_from_slice(&self.dst_sgt.to_be_bytes());
        buf.push(self.flags);
        let t_bytes = self.tenant_id.to_be_bytes();
        buf.push(t_bytes[1]);
        buf.push(t_bytes[2]);
        buf.push(t_bytes[3]);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 8 {
            return Err("GBP option payload too short (expected 8 bytes)");
        }
        let src_sgt = u16::from_be_bytes([data[0], data[1]]);
        let dst_sgt = u16::from_be_bytes([data[2], data[3]]);
        let flags = data[4];
        let tenant_id = u32::from_be_bytes([0, data[5], data[6], data[7]]);

        Ok(SecurityGroupTag {
            src_sgt,
            dst_sgt,
            flags,
            tenant_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicrosegAction {
    Allow,
    Deny,
    LogOnly,
    RateLimitBps(u32),
}

impl fmt::Display for MicrosegAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MicrosegAction::Allow => write!(f, "ALLOW"),
            MicrosegAction::Deny => write!(f, "DENY"),
            MicrosegAction::LogOnly => write!(f, "LOG-ONLY"),
            MicrosegAction::RateLimitBps(r) => write!(f, "RATE-LIMIT ({} bps)", r),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicrosegDecision {
    Permit,
    Drop,
    RateLimit(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrosegRule {
    pub id: u32,
    pub src_sgt: Option<u16>,
    pub dst_sgt: Option<u16>,
    pub protocol: Option<u8>,
    pub dst_port: Option<u16>,
    pub action: MicrosegAction,
    pub hit_count: u64,
}

impl MicrosegRule {
    pub fn new(
        id: u32,
        src_sgt: Option<u16>,
        dst_sgt: Option<u16>,
        protocol: Option<u8>,
        dst_port: Option<u16>,
        action: MicrosegAction,
    ) -> Self {
        MicrosegRule {
            id,
            src_sgt,
            dst_sgt,
            protocol,
            dst_port,
            action,
            hit_count: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenevePolicyEngine {
    pub rules: Vec<MicrosegRule>,
    pub default_action: MicrosegAction,
    pub total_evaluated: u64,
    pub total_permitted: u64,
    pub total_dropped: u64,
}

impl Default for GenevePolicyEngine {
    fn default() -> Self {
        GenevePolicyEngine {
            rules: Vec::new(),
            default_action: MicrosegAction::Deny, // Zero-trust default deny
            total_evaluated: 0,
            total_permitted: 0,
            total_dropped: 0,
        }
    }
}

impl GenevePolicyEngine {
    pub fn new(default_action: MicrosegAction) -> Self {
        GenevePolicyEngine {
            rules: Vec::new(),
            default_action,
            total_evaluated: 0,
            total_permitted: 0,
            total_dropped: 0,
        }
    }

    pub fn add_rule(&mut self, rule: MicrosegRule) {
        self.rules.push(rule);
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }

    pub fn evaluate(
        &mut self,
        src_sgt: u16,
        dst_sgt: u16,
        protocol: u8,
        dst_port: Option<u16>,
    ) -> MicrosegDecision {
        self.total_evaluated += 1;

        for rule in &mut self.rules {
            if let Some(req_src) = rule.src_sgt
                && req_src != src_sgt
            {
                continue;
            }

            if let Some(req_dst) = rule.dst_sgt
                && req_dst != dst_sgt
            {
                continue;
            }

            if let Some(req_proto) = rule.protocol
                && req_proto != protocol
            {
                continue;
            }

            if let Some(req_port) = rule.dst_port {
                if dst_port != Some(req_port) {
                    continue;
                }
            }

            rule.hit_count += 1;

            return match &rule.action {
                MicrosegAction::Allow | MicrosegAction::LogOnly => {
                    self.total_permitted += 1;
                    MicrosegDecision::Permit
                }
                MicrosegAction::Deny => {
                    self.total_dropped += 1;
                    MicrosegDecision::Drop
                }
                MicrosegAction::RateLimitBps(bps) => {
                    self.total_permitted += 1;
                    MicrosegDecision::RateLimit(*bps)
                }
            };
        }

        // Apply default policy
        match &self.default_action {
            MicrosegAction::Allow | MicrosegAction::LogOnly => {
                self.total_permitted += 1;
                MicrosegDecision::Permit
            }
            MicrosegAction::Deny => {
                self.total_dropped += 1;
                MicrosegDecision::Drop
            }
            MicrosegAction::RateLimitBps(bps) => {
                self.total_permitted += 1;
                MicrosegDecision::RateLimit(*bps)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geneve_sgt_codec_roundtrip() {
        let sgt = SecurityGroupTag::new(100, 200, 12345);
        let ser = sgt.serialize();
        assert_eq!(ser.len(), 8);

        let parsed = SecurityGroupTag::parse(&ser).unwrap();
        assert_eq!(parsed.src_sgt, 100);
        assert_eq!(parsed.dst_sgt, 200);
        assert_eq!(parsed.tenant_id, 12345);
        assert_eq!(parsed.flags, GBP_FLAG_APPLY_POLICY);
    }

    #[test]
    fn test_geneve_microseg_matrix_evaluation() {
        let mut engine = GenevePolicyEngine::new(MicrosegAction::Deny);

        // Web Tier (SGT 10) to DB Tier (SGT 20) on MySQL (port 3306) -> Allow
        engine.add_rule(MicrosegRule::new(
            1,
            Some(10),
            Some(20),
            Some(6), // TCP
            Some(3306),
            MicrosegAction::Allow,
        ));

        // Permit Web to DB on port 3306
        let dec1 = engine.evaluate(10, 20, 6, Some(3306));
        assert_eq!(dec1, MicrosegDecision::Permit);
        assert_eq!(engine.rules[0].hit_count, 1);

        // Deny Web to DB on SSH (port 22) due to default zero-trust deny
        let dec2 = engine.evaluate(10, 20, 6, Some(22));
        assert_eq!(dec2, MicrosegDecision::Drop);

        // Deny External (SGT 99) to DB (SGT 20) on port 3306
        let dec3 = engine.evaluate(99, 20, 6, Some(3306));
        assert_eq!(dec3, MicrosegDecision::Drop);

        assert_eq!(engine.total_evaluated, 3);
        assert_eq!(engine.total_permitted, 1);
        assert_eq!(engine.total_dropped, 2);
    }
}
