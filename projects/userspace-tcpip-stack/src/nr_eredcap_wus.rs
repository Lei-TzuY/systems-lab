//! 3GPP Release 18 (5G-Advanced) e-RedCap & Low-Power Wake-Up Signal (LP-WUS) Engine.
//!
//! Standards Reference:
//! - 3GPP TS 38.300 §16.14: "Reduced Capability (RedCap) UEs and further Reduced Capability (e-RedCap) UEs"
//! - 3GPP TS 38.211 §7.4: Low-Power Wake-Up Signal (LP-WUS) sequence generation
//! - 3GPP TS 38.213 §10 / §11: WUS monitoring occasions, timing offsets, and DCI 2_6 / Wake-up indications
//! - 3GPP TS 38.304 / TS 38.331: Extended DRX (eDRX) with Hyper-SFN (H-SFN 0..1023), Paging Time Window (PTW), and stationary relaxed RRM.
//!
//! This module implements the end-to-end e-RedCap IoT subsystem:
//! 1. e-RedCap 5 MHz and 10 MHz PRB bandwidth and peak rate limits (15 kHz & 30 kHz SCS).
//! 2. Low-Power Wake-Up Signal (LP-WUS) sequence generation with On-Off Keying (OOK) / FSK modulation.
//! 3. Low-Power Wake-Up Receiver (LP-WUR) energy correlation detector with adaptive thresholding ($< 500\ \mu\text{W}$).
//! 4. Extended DRX (eDRX) cycle state machine with Hyper-SFN (H-SFN) and Paging Time Window (PTW).
//! 5. Stationary and low-mobility relaxed RRM measurement evaluation ($8\times - 32\times$ relaxation).
//! 6. Small Data Transmission (SDT) in RRC_INACTIVE state (Configured Grant and RACH-based).
//! 7. Battery lifetime and energy efficiency analytical models (> 90% power reduction over legacy DRX).

use std::collections::VecDeque;
use std::fmt;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered in e-RedCap and LP-WUS operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ERedCapError {
    InvalidBandwidth(u32),
    InvalidPagingGroupId(u16),
    InvalidHyperSfn(u16),
    InvalidSfn(u16),
    InvalidSequenceLength(usize),
    InvalidEdrxCycle(u32),
    BufferOverflow { capacity: usize },
    DetectionError(String),
}

impl fmt::Display for ERedCapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ERedCapError::InvalidBandwidth(bw) => {
                write!(
                    f,
                    "Invalid e-RedCap bandwidth: {bw} MHz (supported: 5, 10 MHz)"
                )
            }
            ERedCapError::InvalidPagingGroupId(pg) => {
                write!(f, "Invalid Paging Group ID: {pg} (must be 0..=255)")
            }
            ERedCapError::InvalidHyperSfn(h) => {
                write!(f, "Invalid Hyper-SFN: {h} (must be 0..=1023)")
            }
            ERedCapError::InvalidSfn(s) => {
                write!(f, "Invalid SFN: {s} (must be 0..=1023)")
            }
            ERedCapError::InvalidSequenceLength(len) => {
                write!(f, "Invalid LP-WUS sequence length: {len}")
            }
            ERedCapError::InvalidEdrxCycle(c) => {
                write!(
                    f,
                    "Invalid eDRX cycle: {c} H-SFNs (must be power of 2: 1..=1024)"
                )
            }
            ERedCapError::BufferOverflow { capacity } => {
                write!(
                    f,
                    "Small data transmission buffer overflow (capacity: {capacity})"
                )
            }
            ERedCapError::DetectionError(msg) => write!(f, "LP-WUR detection error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// e-RedCap Bandwidth & RF Configuration (TS 38.300 §16.14)
// ---------------------------------------------------------------------------

/// Supported channel bandwidths for e-RedCap UEs in FR1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ERedCapBandwidth {
    /// 5 MHz channel bandwidth (25 PRBs at 15 kHz SCS, 12 PRBs at 30 kHz SCS).
    Bw5Mhz,
    /// 10 MHz channel bandwidth (52 PRBs at 15 kHz SCS, 24 PRBs at 30 kHz SCS).
    Bw10Mhz,
}

impl ERedCapBandwidth {
    pub fn bandwidth_mhz(&self) -> u32 {
        match self {
            ERedCapBandwidth::Bw5Mhz => 5,
            ERedCapBandwidth::Bw10Mhz => 10,
        }
    }

    /// Number of Physical Resource Blocks (PRBs) allocated for given Subcarrier Spacing (SCS).
    pub fn allocated_prbs(&self, scs_khz: u32) -> u32 {
        match (self, scs_khz) {
            (ERedCapBandwidth::Bw5Mhz, 15) => 25,
            (ERedCapBandwidth::Bw5Mhz, 30) => 12,
            (ERedCapBandwidth::Bw10Mhz, 15) => 52,
            (ERedCapBandwidth::Bw10Mhz, 30) => 24,
            _ => 25, // default fallback
        }
    }

    /// Theoretical maximum DL peak bitrate in bits per second (3GPP TS 38.306 §4.1.2).
    pub fn theoretical_peak_dl_bps(&self, scs_khz: u32, rx_antennas: u8) -> u64 {
        let prbs = self.allocated_prbs(scs_khz) as f64;
        let slots_per_sec = match scs_khz {
            15 => 1000.0,
            30 => 2000.0,
            60 => 4000.0,
            _ => 2000.0,
        };
        let symbols_per_sec = prbs * 12.0 * 14.0 * slots_per_sec;
        let qm = 6.0; // 64-QAM
        let r_max = 0.925;
        let oh = 0.14; // DL overhead factor in FR1
        let layers = (rx_antennas as f64).clamp(1.0, 2.0);
        (symbols_per_sec * qm * r_max * (1.0 - oh) * layers) as u64
    }
}

/// Antenna configuration of the e-RedCap device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntennaConfiguration {
    /// Single transmitter, single receiver (1T1R) - lowest cost.
    OneTxOneRx,
    /// Single transmitter, dual receiver (1T2R) - diversity reception.
    OneTxTwoRx,
}

/// Power consumption state of the e-RedCap device.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerProfile {
    /// Deep sleep power in microwatts (main baseband OFF, LP-WUR active).
    pub deep_sleep_power_uw: f64,
    /// Low-power wake-up receiver active power in microwatts.
    pub lpwur_active_power_uw: f64,
    /// Main NR baseband active power in milliwatts (PDCCH/PDSCH reception).
    pub main_rx_power_mw: f64,
    /// Uplink transmission power in milliwatts (+20 dBm EIRP).
    pub main_tx_power_mw: f64,
}

impl Default for PowerProfile {
    fn default() -> Self {
        PowerProfile {
            deep_sleep_power_uw: 15.0,    // 15 µW deep sleep
            lpwur_active_power_uw: 350.0, // 350 µW LP-WUR active listening
            main_rx_power_mw: 95.0,       // 95 mW full 5G NR baseband RX
            main_tx_power_mw: 320.0,      // 320 mW full 5G NR baseband TX
        }
    }
}

// ---------------------------------------------------------------------------
// Low-Power Wake-Up Signal (LP-WUS) Modulation & Detection (TS 38.211 §7.4)
// ---------------------------------------------------------------------------

/// Modulation scheme for the Low-Power Wake-Up Signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpWusModulation {
    /// On-Off Keying (OOK) - ultra-low complexity envelope detector.
    OnOfKeying,
    /// 2-ary Frequency Shift Keying (2-FSK).
    FrequencyShiftKeying,
}

/// Low-Power Wake-Up Signal (LP-WUS) Sequence Specification.
#[derive(Debug, Clone, PartialEq)]
pub struct LpWusSequence {
    pub paging_group_id: u16,
    pub cell_id: u16,
    pub modulation: LpWusModulation,
    pub sequence_length: usize,
    /// Advance offset before Paging Occasion in milliseconds (typically 10 to 40 ms).
    pub advance_offset_ms: u32,
    pub chip_values: Vec<f64>,
}

impl LpWusSequence {
    /// Generate a deterministic pseudo-random binary sequence for given Paging Group ID and Cell ID.
    pub fn generate(
        paging_group_id: u16,
        cell_id: u16,
        modulation: LpWusModulation,
        sequence_length: usize,
        advance_offset_ms: u32,
    ) -> Result<Self, ERedCapError> {
        if paging_group_id > 255 {
            return Err(ERedCapError::InvalidPagingGroupId(paging_group_id));
        }
        if sequence_length == 0 || sequence_length > 512 {
            return Err(ERedCapError::InvalidSequenceLength(sequence_length));
        }

        // Gold sequence initialization seed: c_init = (PG_ID + 1) * (Cell_ID + 1) * 31
        let mut lfsr: u32 = ((paging_group_id as u32 + 1) * (cell_id as u32 + 1) * 31) | 1;
        let mut chips = Vec::with_capacity(sequence_length);

        for _ in 0..sequence_length {
            let bit = (lfsr & 1) as f64;
            // Linear feedback: x^31 + x^3 + 1
            let new_bit = ((lfsr >> 0) ^ (lfsr >> 28)) & 1;
            lfsr = (lfsr >> 1) | (new_bit << 30);

            match modulation {
                LpWusModulation::OnOfKeying => {
                    // OOK: 1 -> +1.0, 0 -> 0.0
                    chips.push(bit);
                }
                LpWusModulation::FrequencyShiftKeying => {
                    // 2-FSK: 1 -> +1.0, 0 -> -1.0
                    chips.push(if bit > 0.5 { 1.0 } else { -1.0 });
                }
            }
        }

        Ok(LpWusSequence {
            paging_group_id,
            cell_id,
            modulation,
            sequence_length,
            advance_offset_ms,
            chip_values: chips,
        })
    }
}

/// Outcome of Low-Power Wake-Up Receiver (LP-WUR) detection.
#[derive(Debug, Clone, PartialEq)]
pub enum LpWurDecision {
    /// Valid LP-WUS detected matching Paging Group ID: Wake up main NR baseband!
    WakeUpMainBaseband {
        paging_group_id: u16,
        correlation_score: f64,
        threshold: f64,
        snr_est_db: f64,
    },
    /// No LP-WUS or unmatched: Remain in ultra-low power sleep.
    RemainAsleep {
        max_correlation: f64,
        threshold: f64,
    },
}

/// Low-Power Wake-Up Receiver (LP-WUR) Energy Correlation Detector.
#[derive(Debug, Clone)]
pub struct LpWurDetector {
    pub target_sequence: LpWusSequence,
    /// Detection threshold factor (multiplied with noise standard deviation).
    pub threshold_factor: f64,
    pub total_detections: u64,
    pub total_sleep_cycles: u64,
}

impl LpWurDetector {
    pub fn new(target_sequence: LpWusSequence, threshold_factor: f64) -> Self {
        let factor = if threshold_factor <= 0.0 {
            0.65
        } else {
            threshold_factor
        };
        LpWurDetector {
            target_sequence,
            threshold_factor: factor,
            total_detections: 0,
            total_sleep_cycles: 0,
        }
    }

    /// Correlate received signal samples with the stored LP-WUS template.
    pub fn detect(&mut self, received_signal: &[f64], noise_floor_sigma: f64) -> LpWurDecision {
        let template = &self.target_sequence.chip_values;
        let len = template.len().min(received_signal.len());

        if len == 0 {
            return LpWurDecision::RemainAsleep {
                max_correlation: 0.0,
                threshold: self.threshold_factor,
            };
        }

        // Calculate normalized cross-correlation: R = sum(x * y) / sqrt(sum(x^2) * sum(y^2))
        let mut dot_product = 0.0;
        let mut energy_template = 0.0;
        let mut energy_signal = 0.0;

        for i in 0..len {
            let x = template[i];
            let y = received_signal[i];
            dot_product += x * y;
            energy_template += x * x;
            energy_signal += y * y;
        }

        let denom = (energy_template * energy_signal).sqrt();
        let normalized_correlation = if denom > 1e-12 {
            (dot_product / denom).max(0.0)
        } else {
            0.0
        };

        // Adaptive detection threshold based on noise floor
        let adaptive_threshold =
            (self.threshold_factor * (1.0 - noise_floor_sigma.clamp(0.0, 0.5))).clamp(0.35, 0.95);

        if normalized_correlation >= adaptive_threshold {
            self.total_detections += 1;
            // Estimated SNR: SNR ~ 10 * log10(R^2 / (1 - R^2 + 1e-6))
            let r_sq = normalized_correlation * normalized_correlation;
            let snr_est = 10.0 * ((r_sq / (1.0 - r_sq + 1e-4)).log10());

            LpWurDecision::WakeUpMainBaseband {
                paging_group_id: self.target_sequence.paging_group_id,
                correlation_score: normalized_correlation,
                threshold: adaptive_threshold,
                snr_est_db: snr_est,
            }
        } else {
            self.total_sleep_cycles += 1;
            LpWurDecision::RemainAsleep {
                max_correlation: normalized_correlation,
                threshold: adaptive_threshold,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Extended DRX (eDRX) & Hyper-SFN (H-SFN) Cycle State Machine (TS 38.304 §7.1)
// ---------------------------------------------------------------------------

/// Timing in Hyper-SFN (H-SFN) and System Frame Number (SFN) domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HyperSfnTiming {
    /// Hyper-SFN index (0..1023). 1 H-SFN = 1024 SFNs = 10.24 seconds.
    pub h_sfn: u16,
    /// System Frame Number (0..1023). 1 SFN = 10 milliseconds.
    pub sfn: u16,
    /// Subframe index (0..9). 1 subframe = 1 millisecond.
    pub subframe: u8,
}

impl HyperSfnTiming {
    pub fn new(h_sfn: u16, sfn: u16, subframe: u8) -> Result<Self, ERedCapError> {
        if h_sfn > 1023 {
            return Err(ERedCapError::InvalidHyperSfn(h_sfn));
        }
        if sfn > 1023 {
            return Err(ERedCapError::InvalidSfn(sfn));
        }
        Ok(HyperSfnTiming {
            h_sfn,
            sfn,
            subframe: subframe.min(9),
        })
    }

    /// Advance timing by given milliseconds.
    pub fn advance_ms(&mut self, delta_ms: u64) {
        let total_subframes = (self.subframe as u64) + delta_ms;
        let new_subframe = (total_subframes % 10) as u8;
        let delta_frames = total_subframes / 10;

        let total_frames = (self.sfn as u64) + delta_frames;
        let new_sfn = (total_frames % 1024) as u16;
        let delta_hsfn = total_frames / 1024;

        let total_hsfn = (self.h_sfn as u64) + delta_hsfn;
        let new_hsfn = (total_hsfn % 1024) as u16;

        self.h_sfn = new_hsfn;
        self.sfn = new_sfn;
        self.subframe = new_subframe;
    }

    /// Total elapsed time in seconds.
    pub fn to_seconds(&self) -> f64 {
        (self.h_sfn as f64 * 10.24) + (self.sfn as f64 * 0.01) + (self.subframe as f64 * 0.001)
    }
}

/// Extended DRX (eDRX) Configuration (TS 38.304 §7.1).
#[derive(Debug, Clone, PartialEq)]
pub struct EDrxConfig {
    pub ue_id: u64,
    /// eDRX cycle in number of H-SFNs (e.g. 1, 2, 4, 8, 16, ..., 1024).
    pub edrx_cycle_hsfn: u16,
    /// Paging Time Window (PTW) length in seconds (typically 2.56 s to 40.96 s).
    pub ptw_length_seconds: f64,
    /// DRX cycle within PTW in milliseconds (e.g. 1280 ms or 2560 ms).
    pub ptw_drx_cycle_ms: u32,
}

impl EDrxConfig {
    pub fn new(
        ue_id: u64,
        edrx_cycle_hsfn: u16,
        ptw_length_seconds: f64,
        ptw_drx_cycle_ms: u32,
    ) -> Result<Self, ERedCapError> {
        if edrx_cycle_hsfn == 0 || edrx_cycle_hsfn > 1024 {
            return Err(ERedCapError::InvalidEdrxCycle(edrx_cycle_hsfn as u32));
        }

        Ok(EDrxConfig {
            ue_id,
            edrx_cycle_hsfn,
            ptw_length_seconds: ptw_length_seconds.max(1.28),
            ptw_drx_cycle_ms: ptw_drx_cycle_ms.max(320),
        })
    }

    /// Check if current H-SFN matches the eDRX paging formula:
    /// H-SFN mod T_eDRX,H = (UE_ID_H mod T_eDRX,H)
    pub fn is_paging_hsfn(&self, timing: &HyperSfnTiming) -> bool {
        let t_edrx = self.edrx_cycle_hsfn;
        let ue_id_h = (self.ue_id >> 10) as u16; // 10 MSBs of UE_ID
        (timing.h_sfn % t_edrx) == (ue_id_h % t_edrx)
    }

    /// Check whether the UE is currently inside its active Paging Time Window (PTW).
    pub fn is_inside_ptw(&self, timing: &HyperSfnTiming) -> bool {
        if !self.is_paging_hsfn(timing) {
            return false;
        }

        // Inside the matching H-SFN, PTW starts at SFN 0 up to ptw_length_seconds
        let elapsed_in_hsfn_s = (timing.sfn as f64 * 0.01) + (timing.subframe as f64 * 0.001);
        elapsed_in_hsfn_s <= self.ptw_length_seconds
    }
}

// ---------------------------------------------------------------------------
// Stationary & Low-Mobility Relaxed RRM Measurement (TS 38.304 §5.2.4.12)
// ---------------------------------------------------------------------------

/// Evaluator for stationary / low-mobility condition and relaxed RRM neighbor measurements.
#[derive(Debug, Clone)]
pub struct RelaxedRrmEvaluator {
    /// RSRP measurement history window.
    rsrp_history_dbm: VecDeque<f64>,
    pub max_history_samples: usize,
    /// Variance threshold in dB to qualify as stationary (typically 2.0 dB).
    pub stationary_threshold_db: f64,
    /// Minimum serving cell RSRP in dBm to permit relaxation (typically -105 dBm).
    pub min_serving_rsrp_dbm: f64,
    pub is_stationary: bool,
    pub is_relaxation_active: bool,
    /// Measurement period relaxation factor (e.g. 8x, 16x, 32x).
    pub relaxation_factor: u32,
}

impl RelaxedRrmEvaluator {
    pub fn new(
        stationary_threshold_db: f64,
        min_serving_rsrp_dbm: f64,
        relaxation_factor: u32,
    ) -> Self {
        RelaxedRrmEvaluator {
            rsrp_history_dbm: VecDeque::with_capacity(30),
            max_history_samples: 30,
            stationary_threshold_db: stationary_threshold_db.max(1.0),
            min_serving_rsrp_dbm,
            is_stationary: false,
            is_relaxation_active: false,
            relaxation_factor: relaxation_factor.clamp(2, 32),
        }
    }

    /// Record a serving cell RSRP sample and evaluate relaxed RRM eligibility.
    pub fn record_rsrp_sample(&mut self, rsrp_dbm: f64) -> bool {
        if self.rsrp_history_dbm.len() >= self.max_history_samples {
            self.rsrp_history_dbm.pop_front();
        }
        self.rsrp_history_dbm.push_back(rsrp_dbm);

        if self.rsrp_history_dbm.len() < 5 {
            // Need at least 5 samples to determine variance
            self.is_stationary = false;
            self.is_relaxation_active = false;
            return false;
        }

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        for &val in &self.rsrp_history_dbm {
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }

        let delta_db = max_val - min_val;
        let latest_rsrp = *self.rsrp_history_dbm.back().unwrap();

        // Stationary if variance <= threshold and latest RSRP is comfortably above cell edge
        self.is_stationary = delta_db <= self.stationary_threshold_db;
        self.is_relaxation_active =
            self.is_stationary && (latest_rsrp >= self.min_serving_rsrp_dbm);

        self.is_relaxation_active
    }

    /// Get current neighbor measurement period in seconds given baseline period.
    pub fn effective_measurement_period_s(&self, baseline_period_s: f64) -> f64 {
        if self.is_relaxation_active {
            baseline_period_s * (self.relaxation_factor as f64)
        } else {
            baseline_period_s
        }
    }
}

// ---------------------------------------------------------------------------
// Small Data Transmission (SDT) in RRC_INACTIVE
// ---------------------------------------------------------------------------

/// Mode of Small Data Transmission (SDT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdtMode {
    /// Configured Grant Small Data Transmission (CG-SDT).
    ConfiguredGrant,
    /// 2-step / 4-step RACH Small Data Transmission (RACH-SDT).
    RachBased,
}

/// Small Data Transmission Packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdtPacket {
    pub transaction_id: u32,
    pub mode: SdtMode,
    pub rrc_resume_cause: u8,
    pub payload: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Top-Level e-RedCap Engine
// ---------------------------------------------------------------------------

/// Telemetry metrics for the e-RedCap subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct ERedCapMetrics {
    pub total_wus_monitored: u64,
    pub total_wus_detections: u64,
    pub total_false_alarms: u64,
    pub total_deep_sleep_hours: f64,
    pub total_sdt_transmissions: u64,
    pub cumulative_energy_consumed_joules: f64,
    pub legacy_energy_consumed_joules: f64,
}

/// Top-Level 3GPP Release 18 e-RedCap & Low-Power Wake-Up Signal Engine.
pub struct ERedCapEngine {
    pub ue_id: u64,
    pub bandwidth: ERedCapBandwidth,
    pub antenna_mode: AntennaConfiguration,
    pub power_profile: PowerProfile,
    pub detector: LpWurDetector,
    pub edrx_config: EDrxConfig,
    pub timing: HyperSfnTiming,
    pub relaxed_rrm: RelaxedRrmEvaluator,
    pub sdt_queue: VecDeque<SdtPacket>,
    pub max_sdt_capacity: usize,
    pub metrics: ERedCapMetrics,
}

impl ERedCapEngine {
    pub fn new(
        ue_id: u64,
        bandwidth: ERedCapBandwidth,
        antenna_mode: AntennaConfiguration,
        target_wus: LpWusSequence,
        edrx_config: EDrxConfig,
    ) -> Self {
        ERedCapEngine {
            ue_id,
            bandwidth,
            antenna_mode,
            power_profile: PowerProfile::default(),
            detector: LpWurDetector::new(target_wus, 0.65),
            edrx_config,
            timing: HyperSfnTiming::new(0, 0, 0).unwrap(),
            relaxed_rrm: RelaxedRrmEvaluator::new(2.0, -105.0, 16),
            sdt_queue: VecDeque::with_capacity(16),
            max_sdt_capacity: 16,
            metrics: ERedCapMetrics {
                total_wus_monitored: 0,
                total_wus_detections: 0,
                total_false_alarms: 0,
                total_deep_sleep_hours: 0.0,
                total_sdt_transmissions: 0,
                cumulative_energy_consumed_joules: 0.0,
                legacy_energy_consumed_joules: 0.0,
            },
        }
    }

    /// Advance device simulation clock and update energy consumption.
    pub fn step_time_ms(&mut self, delta_ms: u64) {
        self.timing.advance_ms(delta_ms);
        let delta_s = (delta_ms as f64) / 1000.0;
        self.metrics.total_deep_sleep_hours += delta_s / 3600.0;

        // Energy consumption model:
        // With LP-WUS: consumes deep_sleep_power (15 µW) + listening LP-WUR (350 µW) only during monitoring
        let power_w = (self.power_profile.deep_sleep_power_uw
            + self.power_profile.lpwur_active_power_uw)
            * 1e-6;
        self.metrics.cumulative_energy_consumed_joules += power_w * delta_s;

        // Legacy baseline without LP-WUR: full receiver wakes up periodically (95 mW)
        let legacy_power_w = 95.0 * 1e-3 * 0.08 + (15.0 * 1e-6 * 0.92); // 8% duty cycle wake-up
        self.metrics.legacy_energy_consumed_joules += legacy_power_w * delta_s;
    }

    /// Process a received RF signal during a Wake-Up Signal monitoring occasion.
    pub fn evaluate_wus_occasion(
        &mut self,
        received_signal: &[f64],
        noise_sigma: f64,
    ) -> LpWurDecision {
        self.metrics.total_wus_monitored += 1;
        let decision = self.detector.detect(received_signal, noise_sigma);

        match &decision {
            LpWurDecision::WakeUpMainBaseband { .. } => {
                self.metrics.total_wus_detections += 1;
                // Add energy cost of waking main baseband for 20 ms
                let wake_energy = (self.power_profile.main_rx_power_mw * 1e-3) * 0.020;
                self.metrics.cumulative_energy_consumed_joules += wake_energy;
            }
            LpWurDecision::RemainAsleep { .. } => {
                // Remained in micro-power sleep, energy already accounted in step_time
            }
        }

        decision
    }

    /// Enqueue and transmit a Small Data Transmission (SDT) packet in RRC_INACTIVE.
    pub fn transmit_sdt_packet(&mut self, packet: SdtPacket) -> Result<(), ERedCapError> {
        if self.sdt_queue.len() >= self.max_sdt_capacity {
            return Err(ERedCapError::BufferOverflow {
                capacity: self.max_sdt_capacity,
            });
        }

        // Add energy cost of SDT transmission (burst 10 ms at 320 mW)
        let tx_energy = (self.power_profile.main_tx_power_mw * 1e-3) * 0.010;
        self.metrics.cumulative_energy_consumed_joules += tx_energy;
        self.metrics.total_sdt_transmissions += 1;

        self.sdt_queue.push_back(packet);
        Ok(())
    }

    /// Calculate percentage power savings compared to legacy continuous paging DRX.
    pub fn energy_savings_percentage(&self) -> f64 {
        if self.metrics.legacy_energy_consumed_joules > 0.0 {
            let saved = self.metrics.legacy_energy_consumed_joules
                - self.metrics.cumulative_energy_consumed_joules;
            ((saved / self.metrics.legacy_energy_consumed_joules) * 100.0).clamp(0.0, 99.9)
        } else {
            0.0
        }
    }
}
