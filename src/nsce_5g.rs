//! 3GPP TS 29.537 / TS 23.256 Release 17 5G Network Slice Capability Enablement (NSCE) Engine.
//!
//! Implements 5G NSCE Server for Vertical Industry Slice Automation:
//! - Nnsce_NSC Service (Network Slice Capability Service - TS 29.537 Section 5.2):
//!   - Discovery of slice capabilities (URLLC, TSN, Edge Breakout, Massive IoT)
//!   - Dynamic slice adaptation requests for vertical apps (V2X, Smart Grid, Factory Automation)
//! - Nnsce_NSAE Service (Network Slice Adaptation and Enablement Service - TS 29.537 Section 5.3):
//!   - Real-time Slice SLA compliance monitoring (latency, packet loss rate in PPM)
//!   - Automated SLA violation alerts and emergency bandwidth scaling

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 5G NSCE Enums & Data Structures (TS 29.537 Section 6 / TS 23.256)
// ---------------------------------------------------------------------------

/// Network Slice Capability Type (TS 23.256 Section 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceCapability {
    /// Ultra-Reliable Low Latency Communication (<5ms)
    UrllcUltraLowLatency,
    /// Time-Sensitive Networking (IEEE 802.1Qbv integration)
    TsnDeterministic,
    /// Edge Computing Local Breakout
    EdgeLocalBreakout,
    /// Massive IoT High-Density Connectivity
    MassiveIot,
    /// Extreme High-Throughput (10Gbps+ aggregate)
    HighThroughput,
}

/// Service Level Agreement (SLA) Contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceSlaContract {
    pub max_latency_ms: u32,
    pub max_packet_loss_rate_ppm: u32, // Parts per million (e.g. 10 ppm = 0.001%)
    pub guaranteed_throughput_mbps: u32,
}

/// Slice Adaptation Operational State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceAdaptationState {
    Nominal,
    AdaptedBoosted,
    SlaDegraded,
}

/// Network Slice Capability Profile (TS 29.537 Section 6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceCapabilityProfile {
    pub s_nssai: String, // e.g. "SST1-SD000001"
    pub dnn: String,     // e.g. "v2x.autonomous-traffic.net"
    pub capabilities: Vec<SliceCapability>,
    pub sla_contract: SliceSlaContract,
    pub allocated_throughput_mbps: u32,
    pub adaptation_state: SliceAdaptationState,
}

/// Result of Slice SLA Verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlaAssessmentResult {
    WithinContract,
    SlaViolationAlert {
        reason: &'static str,
        observed_value: u32,
        threshold_value: u32,
    },
}

/// NSCE Error Types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NsceError {
    SliceNotFound,
    CapabilityNotSupported,
    InvalidThroughputValue,
}

// ---------------------------------------------------------------------------
// Top-Level 5G NSCE Server Engine
// ---------------------------------------------------------------------------

/// 5G Network Slice Capability Enablement Server (NSCE).
pub struct NsceServerEngine {
    pub nsce_id: String,
    /// Active Slice Profiles: s_nssai -> SliceCapabilityProfile
    pub slice_profiles: HashMap<String, SliceCapabilityProfile>,
}

impl NsceServerEngine {
    /// Create a new 5G NSCE Server instance.
    pub fn new(nsce_id: &str) -> Self {
        NsceServerEngine {
            nsce_id: nsce_id.to_string(),
            slice_profiles: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Nnsce_NSC Service (TS 29.537 Section 5.2)
    // -----------------------------------------------------------------------

    /// Provision or register an operator Network Slice Profile.
    pub fn register_slice_profile(&mut self, profile: SliceCapabilityProfile) {
        self.slice_profiles.insert(profile.s_nssai.clone(), profile);
    }

    /// Discover capabilities supported by a network slice.
    pub fn discover_slice_capabilities(
        &self,
        s_nssai: &str,
    ) -> Result<Vec<SliceCapability>, NsceError> {
        let profile = self
            .slice_profiles
            .get(s_nssai)
            .ok_or(NsceError::SliceNotFound)?;
        Ok(profile.capabilities.clone())
    }

    /// Dynamically request slice adaptation (e.g. boosting bandwidth to absorb bursty traffic).
    pub fn request_slice_adaptation(
        &mut self,
        s_nssai: &str,
        additional_mbps: u32,
    ) -> Result<u32, NsceError> {
        if additional_mbps == 0 {
            return Err(NsceError::InvalidThroughputValue);
        }

        let profile = self
            .slice_profiles
            .get_mut(s_nssai)
            .ok_or(NsceError::SliceNotFound)?;

        profile.allocated_throughput_mbps += additional_mbps;
        profile.adaptation_state = SliceAdaptationState::AdaptedBoosted;

        Ok(profile.allocated_throughput_mbps)
    }

    /// Reset slice adaptation back to nominal contract state.
    pub fn reset_slice_adaptation(&mut self, s_nssai: &str) -> Result<(), NsceError> {
        let profile = self
            .slice_profiles
            .get_mut(s_nssai)
            .ok_or(NsceError::SliceNotFound)?;

        profile.allocated_throughput_mbps = profile.sla_contract.guaranteed_throughput_mbps;
        profile.adaptation_state = SliceAdaptationState::Nominal;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Nnsce_NSAE Service (TS 29.537 Section 5.3)
    // -----------------------------------------------------------------------

    /// Assess real-time telemetry against contracted Slice SLA.
    pub fn assess_slice_sla(
        &mut self,
        s_nssai: &str,
        observed_latency_ms: u32,
        observed_loss_ppm: u32,
    ) -> Result<SlaAssessmentResult, NsceError> {
        let profile = self
            .slice_profiles
            .get_mut(s_nssai)
            .ok_or(NsceError::SliceNotFound)?;

        // Check Latency Violation
        if observed_latency_ms > profile.sla_contract.max_latency_ms {
            profile.adaptation_state = SliceAdaptationState::SlaDegraded;
            return Ok(SlaAssessmentResult::SlaViolationAlert {
                reason: "Observed Latency Exceeded SLA Threshold",
                observed_value: observed_latency_ms,
                threshold_value: profile.sla_contract.max_latency_ms,
            });
        }

        // Check Packet Loss Violation
        if observed_loss_ppm > profile.sla_contract.max_packet_loss_rate_ppm {
            profile.adaptation_state = SliceAdaptationState::SlaDegraded;
            return Ok(SlaAssessmentResult::SlaViolationAlert {
                reason: "Observed Packet Loss Rate Exceeded SLA Threshold",
                observed_value: observed_loss_ppm,
                threshold_value: profile.sla_contract.max_packet_loss_rate_ppm,
            });
        }

        // Healthy
        if profile.adaptation_state == SliceAdaptationState::SlaDegraded {
            profile.adaptation_state = SliceAdaptationState::Nominal;
        }

        Ok(SlaAssessmentResult::WithinContract)
    }
}
