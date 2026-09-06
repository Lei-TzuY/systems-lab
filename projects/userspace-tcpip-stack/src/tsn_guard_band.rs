//! IEEE 802.1Qbu / 802.3br Frame Preemption & IEEE 802.1Qbv Dynamic Guard Band Engine.
//!
//! In Time-Sensitive Networking (TSN), a scheduled gate for Express Traffic Classes (eTC)
//! must open cleanly without collision from ongoing best-effort frames.
//!
//! Two strategies exist:
//! 1. **Static Guard Band (Qbv without Preemption)**: Gate closes to non-express frames
//!    for a full MTU transmission duration ($1500\text{ B} \times 8 / \text{portRate} \approx 12.3\ \mu\text{s}$ on 1 Gbps)
//!    before the scheduled window starts.
//! 2. **Frame Preemption (802.1Qbu / 802.3br with Preemption)**: Non-express frames can be fragmented
//!    down to a minimum fragment size of 64 bytes (or 124 bytes including mCRC and SMD).
//!    The Guard Band shrinks dramatically down to $124\text{ B} \times 8 / \text{portRate} \approx 1\ \mu\text{s}$,
//!    reclaiming up to $90\%$ of wasted transmission bandwidth.
//!
//! This module implements:
//! * Traffic class classification: Express (eTC) vs Preemptable (pTC).
//! * Dynamic Guard Band calculation with/without frame preemption enabled.
//! * MAC Merge Sublayer Hold/Release primitive state machine.
//! * Frame transmission admission logic.

/// Traffic Class Priority Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityType {
    Express,
    Preemptable,
}

/// MAC Merge Sublayer Hold/Release Primitive State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacMergeState {
    Release,
    Hold,
}

/// TSN Frame Preemption & Guard Band Engine.
#[derive(Debug, Clone)]
pub struct TsnPreemptionGuardBandEngine {
    pub port_rate_bps: u64,
    pub preemption_enabled: bool,
    /// Minimum non-final fragment size (64 bytes default per 802.3br).
    pub min_fragment_size_bytes: usize,
    /// Maximum SDU size on this port (default 1500 bytes).
    pub max_sdu_size_bytes: usize,
    pub merge_state: MacMergeState,
    pub total_preempted_frames: u64,
    pub total_guard_band_drops: u64,
}

impl TsnPreemptionGuardBandEngine {
    pub fn new(port_rate_bps: u64, preemption_enabled: bool) -> Self {
        TsnPreemptionGuardBandEngine {
            port_rate_bps: port_rate_bps.max(1),
            preemption_enabled,
            min_fragment_size_bytes: 64,
            max_sdu_size_bytes: 1500,
            merge_state: MacMergeState::Release,
            total_preempted_frames: 0,
            total_guard_band_drops: 0,
        }
    }

    /// Calculates required guard band duration in nanoseconds before express window starts.
    pub fn calculate_guard_band_duration_ns(&self) -> u64 {
        let vulnerable_bytes = if self.preemption_enabled {
            self.min_fragment_size_bytes as u64
        } else {
            self.max_sdu_size_bytes as u64
        };

        // duration_ns = (vulnerable_bytes * 8 * 1,000,000,000) / port_rate_bps
        (vulnerable_bytes * 8 * 1_000_000_000) / self.port_rate_bps
    }

    /// Signals the Hold / Release MAC merge sublayer primitive from the 802.1Qbv shaper.
    pub fn set_merge_primitive(&mut self, state: MacMergeState) {
        self.merge_state = state;
    }

    /// Evaluates if a frame of given size and priority can start transmission
    /// given time remaining before the scheduled express gate opens.
    pub fn can_transmit_frame(
        &mut self,
        prio_type: PriorityType,
        frame_bytes: usize,
        time_until_express_window_ns: u64,
    ) -> bool {
        if prio_type == PriorityType::Express {
            return true; // Express traffic always allowed in express window
        }

        // For preemptable traffic:
        if self.merge_state == MacMergeState::Hold && !self.preemption_enabled {
            self.total_guard_band_drops += 1;
            return false;
        }

        let guard_band_ns = self.calculate_guard_band_duration_ns();
        let frame_tx_time_ns = ((frame_bytes as u64) * 8 * 1_000_000_000) / self.port_rate_bps;

        if frame_tx_time_ns <= time_until_express_window_ns {
            // Full frame fits before express window!
            true
        } else if self.preemption_enabled && time_until_express_window_ns >= guard_band_ns {
            // Frame can be preempted (fragmented) because remaining time exceeds min fragment size
            self.total_preempted_frames += 1;
            true
        } else {
            // Frame cannot fit and cannot be fragmented safely in time
            self.total_guard_band_drops += 1;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsn_guard_band_with_and_without_preemption() {
        let port_1g = 1_000_000_000; // 1 Gbps

        // 1. Without Preemption: Guard band = 1500B -> 12,000 ns (12 us)
        let mut no_preempt = TsnPreemptionGuardBandEngine::new(port_1g, false);
        assert_eq!(no_preempt.calculate_guard_band_duration_ns(), 12_000);

        // Frame of 1500B takes 12us. If only 6us remaining -> rejected!
        assert!(!no_preempt.can_transmit_frame(PriorityType::Preemptable, 1500, 6_000));
        assert_eq!(no_preempt.total_guard_band_drops, 1);

        // 2. With Preemption: Guard band = 64B -> 512 ns (0.512 us)
        let mut with_preempt = TsnPreemptionGuardBandEngine::new(port_1g, true);
        assert_eq!(with_preempt.calculate_guard_band_duration_ns(), 512);

        // Frame of 1500B with 6us remaining -> can be preempted!
        assert!(with_preempt.can_transmit_frame(PriorityType::Preemptable, 1500, 6_000));
        assert_eq!(with_preempt.total_preempted_frames, 1);
    }
}
