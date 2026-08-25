//! BGP EVPN Preference-Based Designated Forwarder (DF) Election (RFC 8584).
//!
//! Implements the DF Election Extended Community (Type 0x06, Subtype 0x06),
//! Algorithm 0x02 (Preference-based DF election), Sticky Bit (S-bit), Don't Preempt
//! Bit (DP-bit), and deterministic highest-IP tie-breaking for multihomed Ethernet Segments.

use crate::evpn_synch::EthernetSegmentId;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;

/// EVPN DF Election Extended Community Type & Subtype (RFC 8584 Section 3).
pub const BGP_EXT_COMM_TYPE_EVPN: u8 = 0x06;
pub const BGP_EXT_COMM_SUBTYPE_DF_ELECTION: u8 = 0x06;

/// DF Election Algorithms (RFC 8584 Section 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DfElectionAlgorithm {
    DefaultModulo = 0x00,
    HighestRandomWeight = 0x01,
    PreferenceBased = 0x02,
}

/// EVPN DF Election Extended Community (RFC 8584).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvpnDfElectionExtCommunity {
    pub algorithm: DfElectionAlgorithm,
    pub dont_preempt: bool,
    pub sticky: bool,
    pub preference: u16,
}

impl EvpnDfElectionExtCommunity {
    pub fn new_preference(preference: u16, dont_preempt: bool, sticky: bool) -> Self {
        EvpnDfElectionExtCommunity {
            algorithm: DfElectionAlgorithm::PreferenceBased,
            dont_preempt,
            sticky,
            preference,
        }
    }

    /// Serializes the 8-octet BGP Extended Community.
    pub fn serialize(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0] = BGP_EXT_COMM_TYPE_EVPN;
        buf[1] = BGP_EXT_COMM_SUBTYPE_DF_ELECTION;
        buf[2] = self.algorithm as u8;

        let mut flags = 0u8;
        if self.dont_preempt {
            flags |= 0x01;
        }
        if self.sticky {
            flags |= 0x02;
        }
        buf[3] = flags;

        buf[4..6].copy_from_slice(&self.preference.to_be_bytes());
        buf[6] = 0x00; // Reserved
        buf[7] = 0x00; // Reserved
        buf
    }

    /// Parses the 8-octet BGP Extended Community.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data[0] != BGP_EXT_COMM_TYPE_EVPN || data[1] != BGP_EXT_COMM_SUBTYPE_DF_ELECTION {
            return None;
        }

        let algo = match data[2] {
            0x00 => DfElectionAlgorithm::DefaultModulo,
            0x01 => DfElectionAlgorithm::HighestRandomWeight,
            0x02 => DfElectionAlgorithm::PreferenceBased,
            _ => return None,
        };

        let dont_preempt = (data[3] & 0x01) != 0;
        let sticky = (data[3] & 0x02) != 0;
        let preference = u16::from_be_bytes([data[4], data[5]]);

        Some(EvpnDfElectionExtCommunity {
            algorithm: algo,
            dont_preempt,
            sticky,
            preference,
        })
    }
}

/// Candidate PE participating in Preference-based DF election on an ESI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePe {
    pub pe_ip: Ipv4Address,
    pub preference: u16,
    pub dont_preempt: bool,
    pub sticky: bool,
}

/// EVPN Preference-Based DF Election Protocol Engine.
#[derive(Debug, Clone, Default)]
pub struct EvpnPrefDfEngine {
    pub candidates: HashMap<EthernetSegmentId, Vec<CandidatePe>>,
    pub elected_df: HashMap<EthernetSegmentId, Ipv4Address>,
    pub elections_run_count: usize,
}

impl EvpnPrefDfEngine {
    pub fn new() -> Self {
        EvpnPrefDfEngine {
            candidates: HashMap::new(),
            elected_df: HashMap::new(),
            elections_run_count: 0,
        }
    }

    /// Adds or updates a candidate PE for an Ethernet Segment (ESI).
    pub fn add_or_update_candidate(&mut self, esi: EthernetSegmentId, candidate: CandidatePe) {
        let list = self.candidates.entry(esi).or_default();
        if let Some(pos) = list.iter().position(|c| c.pe_ip == candidate.pe_ip) {
            list[pos] = candidate;
        } else {
            list.push(candidate);
        }
    }

    /// Removes a candidate PE from an ESI upon link/node failure.
    pub fn remove_candidate(&mut self, esi: EthernetSegmentId, pe_ip: Ipv4Address) {
        if let Some(list) = self.candidates.get_mut(&esi) {
            list.retain(|c| c.pe_ip != pe_ip);
        }
        if self.elected_df.get(&esi) == Some(&pe_ip) {
            self.elected_df.remove(&esi);
        }
    }

    /// Performs Preference-based DF Election according to RFC 8584 Section 4.
    pub fn elect_df(&mut self, esi: EthernetSegmentId) -> Option<Ipv4Address> {
        let list = self.candidates.get(&esi)?;
        if list.is_empty() {
            self.elected_df.remove(&esi);
            return None;
        }

        self.elections_run_count += 1;

        // Check if an existing elected DF is still alive and has DP (Don't Preempt) or Sticky active
        if let Some(&current_df_ip) = self.elected_df.get(&esi) {
            if let Some(current_cand) = list.iter().find(|c| c.pe_ip == current_df_ip) {
                if current_cand.dont_preempt || current_cand.sticky {
                    return Some(current_df_ip);
                }
            }
        }

        // Elect candidate with:
        // 1. Highest Preference value
        // 2. Highest IPv4 numeric value (RFC 8584 tie-breaker)
        let mut best: Option<&CandidatePe> = None;
        for c in list {
            match best {
                None => best = Some(c),
                Some(b) => {
                    if c.preference > b.preference {
                        best = Some(c);
                    } else if c.preference == b.preference {
                        let c_ip_val = u32::from_be_bytes(c.pe_ip.0);
                        let b_ip_val = u32::from_be_bytes(b.pe_ip.0);
                        if c_ip_val > b_ip_val {
                            best = Some(c);
                        }
                    }
                }
            }
        }

        if let Some(winner) = best {
            self.elected_df.insert(esi, winner.pe_ip);
            Some(winner.pe_ip)
        } else {
            None
        }
    }
}
