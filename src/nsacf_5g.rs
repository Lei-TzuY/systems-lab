//! 3GPP TS 29.536 / TS 23.501 Section 5.15.11 5G Network Slice Admission Control Function (NSACF) Engine.
//!
//! Implements 3GPP Release 17 Network Slice Admission Control (NSAC):
//! - Nnsacf_NSAC Service (TS 29.536 Section 5.2):
//!   - AMF slice registration admission control (`update_ue_admission`)
//!   - SMF PDU session slice admission control (`update_pdu_session_admission`)
//!   - Hard quota enforcement on Maximum Number of Registered UEs per S-NSSAI
//!   - Hard quota enforcement on Maximum Number of PDU Sessions per S-NSSAI
//!   - Idempotent tracking of subscriber SUPIs and active PDU session IDs
//!   - Capacity utilization monitoring with threshold crossing alerts (e.g. 80%, 90%)

use std::collections::{HashMap, HashSet};

use crate::nssaaf_5g::Snssai;

// ---------------------------------------------------------------------------
// 5G NSACF Enums & Data Structures (TS 29.536 Section 6)
// ---------------------------------------------------------------------------

/// Update Action for NSAC counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsacUpdateAction {
    Increase,
    Decrease,
}

/// Result of a slice admission inquiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsacAdmissionResult {
    /// Request admitted within slice SLA capacity.
    Admitted,
    /// Request refused because maximum slice quota is exhausted.
    RefusedExceededQuota,
    /// The requested S-NSSAI is not subject to NSAC control.
    SliceNotSubjectToNsac,
}

/// Capacity and Utilization Status of a slice.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceUtilizationStatus {
    pub snssai: Snssai,
    pub current_ues: u32,
    pub max_ues: Option<u32>,
    pub ue_utilization_pct: f64,
    pub current_pdu_sessions: u32,
    pub max_pdu_sessions: Option<u32>,
    pub pdu_utilization_pct: f64,
    pub alert_threshold_breached: bool,
}

/// Internal Slice NSAC Profile.
#[derive(Debug, Clone)]
pub struct SliceNsacProfile {
    pub snssai: Snssai,
    pub max_registered_ues: Option<u32>,
    pub registered_supis: HashSet<String>,
    pub max_pdu_sessions: Option<u32>,
    pub active_pdu_sessions: HashSet<(String, u8)>, // (supi, pdu_session_id)
    pub threshold_alert_pct: u8,
}

/// NSACF Error Types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NsacfError {
    SliceNotFound,
    QuotaAlreadyZero,
    SessionNotFound,
}

// ---------------------------------------------------------------------------
// Top-Level NSACF Engine
// ---------------------------------------------------------------------------

/// 5G Network Slice Admission Control Function (NSACF).
pub struct NsacfEngine {
    pub nsacf_id: String,
    /// Controlled S-NSSAIs: Snssai -> SliceNsacProfile
    pub controlled_slices: HashMap<Snssai, SliceNsacProfile>,
}

impl NsacfEngine {
    /// Create a new NSACF engine instance.
    pub fn new(nsacf_id: &str) -> Self {
        NsacfEngine {
            nsacf_id: nsacf_id.to_string(),
            controlled_slices: HashMap::new(),
        }
    }

    /// Configure admission control quotas for a slice.
    pub fn configure_slice_quota(
        &mut self,
        snssai: Snssai,
        max_ues: Option<u32>,
        max_pdu_sessions: Option<u32>,
        threshold_alert_pct: u8,
    ) {
        let profile = SliceNsacProfile {
            snssai,
            max_registered_ues: max_ues,
            registered_supis: HashSet::new(),
            max_pdu_sessions,
            active_pdu_sessions: HashSet::new(),
            threshold_alert_pct,
        };
        self.controlled_slices.insert(snssai, profile);
    }

    // -----------------------------------------------------------------------
    // Nnsacf_NSAC Service Operations (TS 29.536 Section 5.2)
    // -----------------------------------------------------------------------

    /// Evaluate and update UE slice registration admission (called by AMF).
    pub fn update_ue_admission(
        &mut self,
        snssai: Snssai,
        supi: &str,
        action: NsacUpdateAction,
    ) -> NsacAdmissionResult {
        let profile = match self.controlled_slices.get_mut(&snssai) {
            Some(p) => p,
            None => return NsacAdmissionResult::SliceNotSubjectToNsac,
        };

        match action {
            NsacUpdateAction::Increase => {
                // If UE is already counted on this slice, admit idempotently
                if profile.registered_supis.contains(supi) {
                    return NsacAdmissionResult::Admitted;
                }

                if let Some(max_ues) = profile.max_registered_ues {
                    if profile.registered_supis.len() as u32 >= max_ues {
                        return NsacAdmissionResult::RefusedExceededQuota;
                    }
                }

                profile.registered_supis.insert(supi.to_string());
                NsacAdmissionResult::Admitted
            }
            NsacUpdateAction::Decrease => {
                profile.registered_supis.remove(supi);
                NsacAdmissionResult::Admitted
            }
        }
    }

    /// Evaluate and update PDU session slice admission (called by SMF).
    pub fn update_pdu_session_admission(
        &mut self,
        snssai: Snssai,
        supi: &str,
        pdu_session_id: u8,
        action: NsacUpdateAction,
    ) -> NsacAdmissionResult {
        let profile = match self.controlled_slices.get_mut(&snssai) {
            Some(p) => p,
            None => return NsacAdmissionResult::SliceNotSubjectToNsac,
        };

        let session_key = (supi.to_string(), pdu_session_id);

        match action {
            NsacUpdateAction::Increase => {
                // If PDU session is already counted, admit idempotently
                if profile.active_pdu_sessions.contains(&session_key) {
                    return NsacAdmissionResult::Admitted;
                }

                if let Some(max_sessions) = profile.max_pdu_sessions {
                    if profile.active_pdu_sessions.len() as u32 >= max_sessions {
                        return NsacAdmissionResult::RefusedExceededQuota;
                    }
                }

                profile.active_pdu_sessions.insert(session_key);
                NsacAdmissionResult::Admitted
            }
            NsacUpdateAction::Decrease => {
                profile.active_pdu_sessions.remove(&session_key);
                NsacAdmissionResult::Admitted
            }
        }
    }

    /// Query capacity and utilization status of a slice.
    pub fn get_slice_utilization(&self, snssai: Snssai) -> Option<SliceUtilizationStatus> {
        let profile = self.controlled_slices.get(&snssai)?;

        let current_ues = profile.registered_supis.len() as u32;
        let ue_util_pct = match profile.max_registered_ues {
            Some(max) if max > 0 => (current_ues as f64 / max as f64) * 100.0,
            _ => 0.0,
        };

        let current_pdu = profile.active_pdu_sessions.len() as u32;
        let pdu_util_pct = match profile.max_pdu_sessions {
            Some(max) if max > 0 => (current_pdu as f64 / max as f64) * 100.0,
            _ => 0.0,
        };

        let threshold = profile.threshold_alert_pct as f64;
        let alert = ue_util_pct >= threshold || pdu_util_pct >= threshold;

        Some(SliceUtilizationStatus {
            snssai,
            current_ues,
            max_ues: profile.max_registered_ues,
            ue_utilization_pct: ue_util_pct,
            current_pdu_sessions: current_pdu,
            max_pdu_sessions: profile.max_pdu_sessions,
            pdu_utilization_pct: pdu_util_pct,
            alert_threshold_breached: alert,
        })
    }
}
