//! PTP Telecom Grandmaster (T-GM) Clock Class & GNSS Holdover Aging (ITU-T G.8275.1 / G.8272 / IEEE 1588-2019).
//!
//! Models Telecom Grandmaster clock quality state transitions, GNSS signal loss,
//! oscillator drift accumulation (Rubidium / OCXO / TCXO), and automated Clock Class
//! degradation (Class 6 -> 7 -> 14 -> 15 -> 165 -> 248).

use crate::ptp_telecom::TelecomBmcaAttributes;

pub const PTP_CLOCK_CLASS_PRTC_LOCKED: u8 = 6;
pub const PTP_CLOCK_CLASS_HOLDOVER_IN_SPEC: u8 = 7;
pub const PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_1: u8 = 14;
pub const PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_2: u8 = 15;
pub const PTP_CLOCK_CLASS_HOLDOVER_OUT_OF_SPEC: u8 = 165;
pub const PTP_CLOCK_CLASS_FREERUN: u8 = 248;

pub const PTP_ACCURACY_LE_25NS: u8 = 0x20;
pub const PTP_ACCURACY_LE_100NS: u8 = 0x21;
pub const PTP_ACCURACY_LE_250NS: u8 = 0x22;
pub const PTP_ACCURACY_LE_1US: u8 = 0x23;
pub const PTP_ACCURACY_LE_2_5US: u8 = 0x24;
pub const PTP_ACCURACY_LE_10US: u8 = 0x25;
pub const PTP_ACCURACY_UNKNOWN: u8 = 0xFE;

/// Oscillator hardware tier supporting the Grandmaster holdover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GmOscillatorType {
    /// Atomic Rubidium standard (high stability, ~1.5 ns / hour drift)
    Rubidium,
    /// High-grade Stratum 3E Oven-Controlled Crystal Oscillator (~15 ns / hour drift)
    Stratum3eOcxo,
    /// Standard OCXO (~100 ns / hour drift)
    StandardOcxo,
    /// Temperature-Compensated Crystal Oscillator (~1000 ns / hour drift)
    Tcxo,
}

impl GmOscillatorType {
    /// Nominal drift rate in nanoseconds per hour.
    pub fn drift_ns_per_hour(&self) -> f64 {
        match self {
            GmOscillatorType::Rubidium => 1.5,
            GmOscillatorType::Stratum3eOcxo => 15.0,
            GmOscillatorType::StandardOcxo => 100.0,
            GmOscillatorType::Tcxo => 1000.0,
        }
    }
}

/// Operational synchronization state of the Telecom Grandmaster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmSyncState {
    /// GNSS locked and traceable to PRTC / UTC (<100ns)
    LockedPrtc,
    /// GNSS lost, operating in in-spec holdover (<250ns)
    HoldoverInSpec,
    /// Holdover degrading: Category 1 (250ns .. 1.5us)
    HoldoverDegradedCat1,
    /// Holdover degrading: Category 2 (1.5us .. 5.0us)
    HoldoverDegradedCat2,
    /// Holdover out-of-specification (>5us)
    HoldoverOutOfSpec,
    /// Freerunning uncalibrated local oscillator
    Freerun,
}

/// Dynamic Telecom Grandmaster Quality Engine.
#[derive(Debug, Clone)]
pub struct TelecomGrandmasterEngine {
    pub clock_id: [u8; 8],
    pub oscillator: GmOscillatorType,
    pub state: GmSyncState,
    pub holdover_seconds: f64,
    pub estimated_phase_error_ns: f64,
    pub local_priority: u8,
}

impl TelecomGrandmasterEngine {
    pub fn new(clock_id: [u8; 8], oscillator: GmOscillatorType) -> Self {
        Self {
            clock_id,
            oscillator,
            state: GmSyncState::LockedPrtc,
            holdover_seconds: 0.0,
            estimated_phase_error_ns: 0.0,
            local_priority: 100,
        }
    }

    /// Triggers GNSS signal loss and enters Holdover.
    pub fn notify_gnss_loss(&mut self) {
        if self.state == GmSyncState::LockedPrtc {
            self.state = GmSyncState::HoldoverInSpec;
            self.holdover_seconds = 0.0;
            self.estimated_phase_error_ns = 25.0; // Baseline initial error
        }
    }

    /// Triggers GNSS signal re-acquisition and restores PRTC locked state.
    pub fn notify_gnss_locked(&mut self) {
        self.state = GmSyncState::LockedPrtc;
        self.holdover_seconds = 0.0;
        self.estimated_phase_error_ns = 15.0;
    }

    /// Advances operational clock time by `seconds` and re-evaluates ClockClass & Accuracy.
    pub fn advance_time(&mut self, elapsed_seconds: f64) {
        if self.state == GmSyncState::LockedPrtc || self.state == GmSyncState::Freerun {
            return;
        }

        self.holdover_seconds += elapsed_seconds;
        let hours = elapsed_seconds / 3600.0;
        let drift = hours * self.oscillator.drift_ns_per_hour();
        self.estimated_phase_error_ns += drift;

        // Determine state based on accumulated estimated phase error
        if self.estimated_phase_error_ns <= 250.0 {
            self.state = GmSyncState::HoldoverInSpec;
        } else if self.estimated_phase_error_ns <= 1500.0 {
            self.state = GmSyncState::HoldoverDegradedCat1;
        } else if self.estimated_phase_error_ns <= 5000.0 {
            self.state = GmSyncState::HoldoverDegradedCat2;
        } else {
            self.state = GmSyncState::HoldoverOutOfSpec;
        }
    }

    /// Generates current Telecom BMCA dataset attributes for Announce message generation.
    pub fn get_bmca_attributes(&self) -> TelecomBmcaAttributes {
        let (clock_class, clock_accuracy, variance) = match self.state {
            GmSyncState::LockedPrtc => (PTP_CLOCK_CLASS_PRTC_LOCKED, PTP_ACCURACY_LE_25NS, 0x4000),
            GmSyncState::HoldoverInSpec => {
                let acc = if self.estimated_phase_error_ns <= 100.0 {
                    PTP_ACCURACY_LE_100NS
                } else {
                    PTP_ACCURACY_LE_250NS
                };
                (PTP_CLOCK_CLASS_HOLDOVER_IN_SPEC, acc, 0x5000)
            }
            GmSyncState::HoldoverDegradedCat1 => (
                PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_1,
                PTP_ACCURACY_LE_1US,
                0x7000,
            ),
            GmSyncState::HoldoverDegradedCat2 => (
                PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_2,
                PTP_ACCURACY_LE_2_5US,
                0x9000,
            ),
            GmSyncState::HoldoverOutOfSpec => (
                PTP_CLOCK_CLASS_HOLDOVER_OUT_OF_SPEC,
                PTP_ACCURACY_LE_10US,
                0xC000,
            ),
            GmSyncState::Freerun => (PTP_CLOCK_CLASS_FREERUN, PTP_ACCURACY_UNKNOWN, 0xFFFF),
        };

        TelecomBmcaAttributes {
            clock_class,
            clock_accuracy,
            offset_scaled_log_variance: variance,
            priority1: 128,
            priority2: 128,
            local_priority: self.local_priority,
            clock_identity: self.clock_id,
            steps_removed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telecom_grandmaster_gnss_holdover_degradation() {
        let clock_id = [0x00, 0x11, 0x22, 0xFF, 0xFE, 0x33, 0x44, 0x55];
        let mut gm = TelecomGrandmasterEngine::new(clock_id, GmOscillatorType::StandardOcxo);

        // 1. Initial state: Locked PRTC
        assert_eq!(gm.state, GmSyncState::LockedPrtc);
        let attr1 = gm.get_bmca_attributes();
        assert_eq!(attr1.clock_class, PTP_CLOCK_CLASS_PRTC_LOCKED);
        assert_eq!(attr1.clock_accuracy, PTP_ACCURACY_LE_25NS);

        // 2. GNSS Loss -> Holdover In-Spec
        gm.notify_gnss_loss();
        assert_eq!(gm.state, GmSyncState::HoldoverInSpec);
        let attr2 = gm.get_bmca_attributes();
        assert_eq!(attr2.clock_class, PTP_CLOCK_CLASS_HOLDOVER_IN_SPEC);

        // 3. Advance 5 hours (Standard OCXO drifts 100 ns/hr -> +500 ns)
        gm.advance_time(5.0 * 3600.0);
        assert_eq!(gm.state, GmSyncState::HoldoverDegradedCat1);
        let attr3 = gm.get_bmca_attributes();
        assert_eq!(attr3.clock_class, PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_1);
        assert_eq!(attr3.clock_accuracy, PTP_ACCURACY_LE_1US);

        // 4. Advance 20 more hours (+2000 ns -> total > 2500 ns)
        gm.advance_time(20.0 * 3600.0);
        assert_eq!(gm.state, GmSyncState::HoldoverDegradedCat2);
        let attr4 = gm.get_bmca_attributes();
        assert_eq!(attr4.clock_class, PTP_CLOCK_CLASS_HOLDOVER_CATEGORY_2);

        // 5. Advance 50 more hours (+5000 ns -> total > 5000 ns)
        gm.advance_time(50.0 * 3600.0);
        assert_eq!(gm.state, GmSyncState::HoldoverOutOfSpec);
        let attr5 = gm.get_bmca_attributes();
        assert_eq!(attr5.clock_class, PTP_CLOCK_CLASS_HOLDOVER_OUT_OF_SPEC);

        // 6. GNSS re-acquisition -> Restores PRTC
        gm.notify_gnss_locked();
        assert_eq!(gm.state, GmSyncState::LockedPrtc);
        let attr6 = gm.get_bmca_attributes();
        assert_eq!(attr6.clock_class, PTP_CLOCK_CLASS_PRTC_LOCKED);
    }
}
