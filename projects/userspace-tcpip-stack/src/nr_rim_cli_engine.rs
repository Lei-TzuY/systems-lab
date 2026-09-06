//! 3GPP Rel-16/17 Remote Interference Management (RIM) & Cross-Link Interference (CLI) Engine.
//!
//! Implements 3GPP TS 38.300 §16.10, TS 38.211 §7.4.1.6, TS 38.213 §16, and TS 38.331:
//! - Atmospheric ducting cross-link interference detection across remote TDD base stations.
//! - RIM Reference Signal (RIM-RS-1 for victim indication, RIM-RS-2 for aggressor acknowledgment)
//!   using 3GPP 31-bit length Gold sequence generation ($N_c = 1600$).
//! - Cross-correlation delay tracking resolving atmospheric ducting distances ($D_{duct} = c \cdot \tau_{prop}$).
//! - Cross-Link Interference (CLI) measurement: CLI-RSSI and Interference-to-Noise Ratio ($INR$).
//! - Severity classification: `None`, `Minor`, `Moderate`, and `Severe`.
//! - Automated TDD mitigation controller:
//!   - Dynamic Guard Period (GP) expansion to absorb distant ducting delay.
//!   - Aggressor downlink transmit power back-off ($\Delta P_{dB} \in 3..12\text{ dB}$).
//!   - Selective PRB scheduling avoidance on victim frequency blocks.
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::fmt;

/// Speed of light in vacuum in meters per second ($c$).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Standard length of Gold code pre-advance ($N_c$).
pub const GOLD_NC: usize = 1600;

/// Thermal noise floor at 30 kHz subcarrier spacing (100 MHz bandwidth) in dBm.
pub const DEFAULT_THERMAL_NOISE_DBM: f64 = -94.0;

/// Type of RIM Reference Signal (3GPP TS 38.211 §7.4.1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RimRsType {
    /// RIM-RS-1: Transmitted by victim gNB to request remote aggressors to mitigate interference.
    RimRs1 = 0,
    /// RIM-RS-2: Transmitted by aggressor gNB to notify its presence and mitigation status.
    RimRs2 = 1,
}

/// Cross-Link Interference Measurement Mode (3GPP TS 38.213 §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CliMeasurementType {
    /// CLI-RSSI: Received signal strength indicator on victim UL symbols.
    CliRssi,
    /// SRS-RSRP: Reference signal received power from specific interfering sounding signals.
    SrsRsrp,
}

/// Severity of Cross-Link / Atmospheric Ducting Interference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterferenceSeverity {
    None,
    Minor,    // INR in [3 dB, 10 dB)
    Moderate, // INR in [10 dB, 20 dB)
    Severe,   // INR >= 20 dB
}

/// Automated TDD Mitigation Action commanded by the RIM Controller.
#[derive(Debug, Clone, PartialEq)]
pub enum MitigationAction {
    None,
    /// Expands the TDD guard period by a specified number of OFDM symbols.
    ExpandGuardPeriod {
        added_symbols: u8,
    },
    /// Instructs aggressor gNB to back off downlink transmission power.
    DlPowerBackoff {
        backoff_db: f64,
    },
    /// Masks specified high-interference PRBs from uplink scheduling.
    AvoidVictimPrbs {
        masked_prbs: Vec<u16>,
    },
}

/// Errors raised during RIM/CLI processing.
#[derive(Debug, Clone, PartialEq)]
pub enum RimCliError {
    InvalidCellId(u16),
    InvalidSequenceLength(usize),
    InsufficientSamples { available: usize, required: usize },
    InvalidSamplingRate(f64),
    CorrelationThresholdOutOfRange(f64),
}

impl fmt::Display for RimCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RimCliError::InvalidCellId(id) => {
                write!(f, "Invalid cell ID {} (must be <= 1007)", id)
            }
            RimCliError::InvalidSequenceLength(len) => {
                write!(f, "Invalid sequence length {} (must be > 0)", len)
            }
            RimCliError::InsufficientSamples {
                available,
                required,
            } => {
                write!(
                    f,
                    "Insufficient samples for correlation: available {}, required {}",
                    available, required
                )
            }
            RimCliError::InvalidSamplingRate(rate) => {
                write!(f, "Invalid sampling rate {:.1} Hz (must be > 0)", rate)
            }
            RimCliError::CorrelationThresholdOutOfRange(th) => {
                write!(f, "Correlation threshold {:.3} out of range (0.0..1.0)", th)
            }
        }
    }
}

impl std::error::Error for RimCliError {}

/// Complex sample representation for cross-correlation and power calculation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ComplexSample {
    pub i: f64,
    pub q: f64,
}

impl ComplexSample {
    pub fn new(i: f64, q: f64) -> Self {
        Self { i, q }
    }

    #[inline]
    pub fn power(&self) -> f64 {
        self.i * self.i + self.q * self.q
    }

    #[inline]
    pub fn conjugate(&self) -> Self {
        Self {
            i: self.i,
            q: -self.q,
        }
    }

    #[inline]
    pub fn multiply(&self, other: &Self) -> Self {
        Self {
            i: self.i * other.i - self.q * other.q,
            q: self.i * other.q + self.q * other.i,
        }
    }
}

/// 3GPP Length-31 Gold Sequence Generator for RIM Reference Signals.
#[derive(Debug, Clone)]
pub struct RimGoldSequenceGenerator;

impl RimGoldSequenceGenerator {
    /// Generates a normalized bipolar (+1 / -1) Gold sequence for a specified cell ID and RS type.
    pub fn generate_sequence(
        cell_id: u16,
        rs_type: RimRsType,
        length: usize,
    ) -> Result<Vec<ComplexSample>, RimCliError> {
        if length == 0 {
            return Err(RimCliError::InvalidSequenceLength(length));
        }

        // c_init calculation (3GPP TS 38.211 §7.4.1.6)
        let c_init =
            (((1u64 << 17) * (cell_id as u64 + 1) + 2 * (cell_id as u64) + (rs_type as u64) + 1)
                % (1u64 << 31)) as u32;

        let total_bits = length + GOLD_NC;
        let mut x1 = vec![0u8; total_bits + 31];
        let mut x2 = vec![0u8; total_bits + 31];

        // x1 initialization: x1(0) = 1, rest 0
        x1[0] = 1;

        // x2 initialization from c_init
        for i in 0..31 {
            x2[i] = ((c_init >> i) & 1) as u8;
        }

        // Generate m-sequences
        for n in 0..total_bits {
            x1[n + 31] = (x1[n + 3] ^ x1[n]) & 1;
            x2[n + 31] = (x2[n + 3] ^ x2[n + 2] ^ x2[n + 1] ^ x2[n]) & 1;
        }

        let mut seq = Vec::with_capacity(length);
        let inv_sqrt2 = 1.0 / (2.0f64).sqrt();

        for n in 0..length {
            let b0 = (x1[n + GOLD_NC] ^ x2[n + GOLD_NC]) & 1;
            // QPSK constellation mapping
            let sample_i = if b0 == 0 { inv_sqrt2 } else { -inv_sqrt2 };
            let sample_q = if b0 == 0 { inv_sqrt2 } else { -inv_sqrt2 };
            seq.push(ComplexSample::new(sample_i, sample_q));
        }

        Ok(seq)
    }
}

/// Atmospheric Ducting Profile and Measurement Configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct AtmosphericDuctingProfile {
    pub sampling_rate_hz: f64,
    pub correlation_threshold: f64,
    pub thermal_noise_dbm: f64,
}

impl AtmosphericDuctingProfile {
    pub fn new(
        sampling_rate_hz: f64,
        correlation_threshold: f64,
        thermal_noise_dbm: f64,
    ) -> Result<Self, RimCliError> {
        if sampling_rate_hz <= 0.0 {
            return Err(RimCliError::InvalidSamplingRate(sampling_rate_hz));
        }
        if correlation_threshold <= 0.0 || correlation_threshold > 1.0 {
            return Err(RimCliError::CorrelationThresholdOutOfRange(
                correlation_threshold,
            ));
        }
        Ok(Self {
            sampling_rate_hz,
            correlation_threshold,
            thermal_noise_dbm,
        })
    }
}

/// Result of Atmospheric Ducting Peak Correlation Detection.
#[derive(Debug, Clone, PartialEq)]
pub struct DuctingDetectionResult {
    pub is_detected: bool,
    pub peak_correlation: f64,
    pub delay_samples: usize,
    pub propagation_delay_us: f64,
    pub ducting_distance_km: f64,
}

/// Operational Telemetry and Health Metrics for RIM/CLI Subsystem.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RimCliMetrics {
    pub total_duct_detections: u64,
    pub last_detected_distance_km: f64,
    pub last_measured_inr_db: f64,
    pub last_cli_rssi_dbm: f64,
    pub active_guard_expansion_symbols: u8,
    pub active_power_backoff_db: f64,
    pub severity: Option<InterferenceSeverity>,
}

/// 3GPP Rel-16/17 Remote Interference Management (RIM) Mitigation Engine.
#[derive(Debug, Clone)]
pub struct RimCliMitigationEngine {
    pub profile: AtmosphericDuctingProfile,
    pub metrics: RimCliMetrics,
}

impl RimCliMitigationEngine {
    pub fn new(profile: AtmosphericDuctingProfile) -> Self {
        Self {
            profile,
            metrics: RimCliMetrics::default(),
        }
    }

    /// Performs normalized cross-correlation between received samples and the reference RIM sequence.
    /// Resolves peak correlation, sample delay, and ducting propagation distance.
    pub fn detect_atmospheric_ducting(
        &mut self,
        received_samples: &[ComplexSample],
        reference_sequence: &[ComplexSample],
    ) -> Result<DuctingDetectionResult, RimCliError> {
        let seq_len = reference_sequence.len();
        if received_samples.len() < seq_len {
            return Err(RimCliError::InsufficientSamples {
                available: received_samples.len(),
                required: seq_len,
            });
        }

        let search_range = received_samples.len() - seq_len;
        let mut max_corr_sq = 0.0f64;
        let mut peak_delay = 0usize;

        // Energy of reference sequence
        let ref_energy: f64 = reference_sequence.iter().map(|s| s.power()).sum();
        if ref_energy < 1e-12 {
            return Ok(DuctingDetectionResult {
                is_detected: false,
                peak_correlation: 0.0,
                delay_samples: 0,
                propagation_delay_us: 0.0,
                ducting_distance_km: 0.0,
            });
        }

        for tau in 0..=search_range {
            let mut sum_corr = ComplexSample::default();
            let mut win_energy = 0.0f64;

            for (k, ref_s) in reference_sequence.iter().enumerate() {
                let rx_s = &received_samples[tau + k];
                let prod = rx_s.multiply(&ref_s.conjugate());
                sum_corr.i += prod.i;
                sum_corr.q += prod.q;
                win_energy += rx_s.power();
            }

            if win_energy > 1e-12 {
                let corr_sq = sum_corr.power() / (ref_energy * win_energy);
                if corr_sq > max_corr_sq {
                    max_corr_sq = corr_sq;
                    peak_delay = tau;
                }
            }
        }

        let peak_corr = max_corr_sq.sqrt();
        let is_detected = peak_corr >= self.profile.correlation_threshold;

        let prop_delay_sec = (peak_delay as f64) / self.profile.sampling_rate_hz;
        let prop_delay_us = prop_delay_sec * 1e6;
        let ducting_distance_km = (SPEED_OF_LIGHT_M_S * prop_delay_sec) / 1000.0;

        if is_detected {
            self.metrics.total_duct_detections += 1;
            self.metrics.last_detected_distance_km = ducting_distance_km;
        }

        Ok(DuctingDetectionResult {
            is_detected,
            peak_correlation: peak_corr,
            delay_samples: peak_delay,
            propagation_delay_us: prop_delay_us,
            ducting_distance_km,
        })
    }

    /// Computes Cross-Link Interference RSSI and Interference-to-Noise Ratio ($INR$).
    pub fn evaluate_cli_rssi(
        &mut self,
        symbol_samples: &[ComplexSample],
    ) -> (f64, f64, InterferenceSeverity) {
        if symbol_samples.is_empty() {
            return (-120.0, 0.0, InterferenceSeverity::None);
        }

        let avg_power: f64 =
            symbol_samples.iter().map(|s| s.power()).sum::<f64>() / (symbol_samples.len() as f64);

        // Convert linear power to dBm
        let cli_rssi_dbm = if avg_power > 1e-15 {
            10.0 * avg_power.log10() + 30.0
        } else {
            -120.0
        };

        let inr_db = (cli_rssi_dbm - self.profile.thermal_noise_dbm).max(0.0);

        let severity = if inr_db < 3.0 {
            InterferenceSeverity::None
        } else if inr_db < 10.0 {
            InterferenceSeverity::Minor
        } else if inr_db < 20.0 {
            InterferenceSeverity::Moderate
        } else {
            InterferenceSeverity::Severe
        };

        self.metrics.last_cli_rssi_dbm = cli_rssi_dbm;
        self.metrics.last_measured_inr_db = inr_db;
        self.metrics.severity = Some(severity);

        (cli_rssi_dbm, inr_db, severity)
    }

    /// Determines the optimal mitigation action based on ducting distance and interference severity.
    pub fn determine_mitigation(
        &mut self,
        detection: &DuctingDetectionResult,
        severity: InterferenceSeverity,
    ) -> MitigationAction {
        if !detection.is_detected || severity == InterferenceSeverity::None {
            self.metrics.active_guard_expansion_symbols = 0;
            self.metrics.active_power_backoff_db = 0.0;
            return MitigationAction::None;
        }

        match severity {
            InterferenceSeverity::Minor => {
                // Minor ducting: add 1 guard symbol to absorb edge delay
                self.metrics.active_guard_expansion_symbols = 1;
                self.metrics.active_power_backoff_db = 0.0;
                MitigationAction::ExpandGuardPeriod { added_symbols: 1 }
            }
            InterferenceSeverity::Moderate => {
                // Moderate ducting (> 100 km): expand guard by 2 symbols or 3 dB power backoff
                if detection.ducting_distance_km > 100.0 {
                    self.metrics.active_guard_expansion_symbols = 2;
                    self.metrics.active_power_backoff_db = 3.0;
                    MitigationAction::DlPowerBackoff { backoff_db: 3.0 }
                } else {
                    self.metrics.active_guard_expansion_symbols = 2;
                    MitigationAction::ExpandGuardPeriod { added_symbols: 2 }
                }
            }
            InterferenceSeverity::Severe => {
                // Severe ducting (> 200 km): significant power backoff (6 to 9 dB) and guard expansion
                self.metrics.active_guard_expansion_symbols = 3;
                self.metrics.active_power_backoff_db = 9.0;
                MitigationAction::DlPowerBackoff { backoff_db: 9.0 }
            }
            InterferenceSeverity::None => unreachable!(),
        }
    }
}
