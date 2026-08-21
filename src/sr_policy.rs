//! Segment Routing Policy Architecture (SR Policy - RFC 9256 / BGP SR-TE RFC 9012).
//!
//! Provides traffic steering via (Color, Endpoint) tuples, candidate path preferences,
//! and weighted segment list forwarding.

use crate::ipv6::Ipv6Address;
use std::collections::HashMap;

pub const BGP_EXT_COMMUNITY_COLOR: u16 = 0x030B;
pub const SR_POLICY_TUNNEL_TYPE: u16 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrProtocolOrigin {
    Cli,
    Pcep,
    BgpSrTe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrSegmentList {
    pub weight: u32,
    pub segments: Vec<Ipv6Address>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrCandidatePath {
    pub preference: u32,
    pub protocol_origin: SrProtocolOrigin,
    pub segment_lists: Vec<SrSegmentList>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrPolicy {
    pub color: u32,
    pub endpoint: Ipv6Address,
    pub name: String,
    pub candidate_paths: Vec<SrCandidatePath>,
}

impl SrPolicy {
    pub fn new(color: u32, endpoint: Ipv6Address, name: &str) -> Self {
        SrPolicy {
            color,
            endpoint,
            name: name.to_string(),
            candidate_paths: Vec::new(),
        }
    }

    pub fn add_candidate_path(&mut self, path: SrCandidatePath) {
        self.candidate_paths.push(path);
        // Sort descending by preference
        self.candidate_paths.sort_by(|a, b| b.preference.cmp(&a.preference));
    }

    /// Selects active candidate path (highest preference)
    pub fn best_candidate_path(&self) -> Option<&SrCandidatePath> {
        self.candidate_paths.first()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SrPolicyDatabase {
    pub policies: HashMap<(u32, Ipv6Address), SrPolicy>,
}

impl SrPolicyDatabase {
    pub fn new() -> Self {
        SrPolicyDatabase {
            policies: HashMap::new(),
        }
    }

    pub fn insert_policy(&mut self, policy: SrPolicy) {
        self.policies.insert((policy.color, policy.endpoint), policy);
    }

    /// Steers traffic matching (color, endpoint) to the active SRv6 Segment List
    pub fn steer_traffic(&self, color: u32, endpoint: Ipv6Address) -> Option<&SrSegmentList> {
        let policy = self.policies.get(&(color, endpoint))?;
        let active_path = policy.best_candidate_path()?;
        active_path.segment_lists.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sr_policy_candidate_path_preference_selection() {
        let mut db = SrPolicyDatabase::new();
        let endpoint = Ipv6Address::new([0x2001, 0x0db8, 0, 0, 0, 0, 0, 0x0002]);

        let mut policy = SrPolicy::new(100, endpoint, "SR-Policy-LowLatency");

        // Add backup path (Preference 100)
        policy.add_candidate_path(SrCandidatePath {
            preference: 100,
            protocol_origin: SrProtocolOrigin::Cli,
            segment_lists: vec![SrSegmentList {
                weight: 1,
                segments: vec![
                    Ipv6Address::new([0xfc00, 0, 0, 1, 0, 0, 0, 0x0001]),
                    Ipv6Address::new([0xfc00, 0, 0, 3, 0, 0, 0, 0x0001]),
                ],
            }],
        });

        // Add primary path (Preference 200)
        policy.add_candidate_path(SrCandidatePath {
            preference: 200,
            protocol_origin: SrProtocolOrigin::BgpSrTe,
            segment_lists: vec![SrSegmentList {
                weight: 1,
                segments: vec![
                    Ipv6Address::new([0xfc00, 0, 0, 2, 0, 0, 0, 0x0001]),
                ],
            }],
        });

        db.insert_policy(policy);

        let active_sl = db.steer_traffic(100, endpoint).unwrap();
        assert_eq!(active_sl.segments.len(), 1);
        assert_eq!(active_sl.segments[0], Ipv6Address::new([0xfc00, 0, 0, 2, 0, 0, 0, 0x0001]));
    }
}
