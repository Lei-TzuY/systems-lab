//! 3GPP Release 18 (5G-Advanced) Multi-Port Phase Tracking Reference Signal (PT-RS)
//! & mmWave / FR2-2 Phase Noise Compensation Engine.
//!
//! Standards Reference:
//! - 3GPP TS 38.211 §5.1.6.3: Downlink Phase Tracking Reference Signals (PT-RS)
//! - 3GPP TS 38.211 §6.4.1.2: Uplink Phase Tracking Reference Signals (CP-OFDM & DFT-s-OFDM)
//! - 3GPP TS 38.214 §5.1.6.3 / §6.2.3.1: PT-RS time/frequency density threshold tables
//! - 3GPP TS 38.101-2: FR2 (24.25-52.6 GHz) & FR2-2 (52.6-71 GHz) phase noise profiles
//!
//! This module implements:
//! 1. Zero-dependency 64-bit complex number arithmetic (`Complex64`).
//! 2. Dynamic time density ($L_{PT-RS} \in \{1, 2, 4\}$) based on scheduled MCS (TS 38.214 Table 5.1.6.3-1).
//! 3. Dynamic frequency density ($K_{PT-RS} \in \{2, 4\}$) based on scheduled PRB allocation (TS 38.214 Table 5.1.6.3-2).
//! 4. 3GPP Gold sequence QPSK generator for CP-OFDM and low-PAPR sequence generator for DFT-s-OFDM.
//! 5. Subcarrier coordinate allocation and DMRS port association.
//! 6. Minimum-Variance Least-Squares Common Phase Error (CPE) estimator with phase unwrapping servo.
//! 7. Full OFDM symbol phase derotation, residual ICI estimation, and EVM enhancement evaluator.

use std::f64::consts::PI;
use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered during PT-RS configuration and phase tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtrsError {
    InvalidMcs(u8),
    InvalidPrbCount(u32),
    InvalidAntennaPort(u16),
    InvalidCellId(u16),
    InvalidThresholdConfiguration(String),
    NoPtrsPresent,
    EstimationFailure(String),
}

impl fmt::Display for PtrsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PtrsError::InvalidMcs(mcs) => write!(f, "Invalid MCS index: {mcs} (must be 0..=31)"),
            PtrsError::InvalidPrbCount(n) => write!(f, "Invalid PRB count: {n} (must be >= 1)"),
            PtrsError::InvalidAntennaPort(p) => {
                write!(f, "Invalid DMRS/PT-RS antenna port: {p}")
            }
            PtrsError::InvalidCellId(id) => write!(f, "Invalid Physical Cell ID: {id} (0..=1007)"),
            PtrsError::InvalidThresholdConfiguration(msg) => {
                write!(f, "Invalid threshold config: {msg}")
            }
            PtrsError::NoPtrsPresent => {
                write!(f, "PT-RS is disabled for current MCS / PRB allocation")
            }
            PtrsError::EstimationFailure(msg) => write!(f, "CPE estimation failure: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// High-Precision Complex Number Math (Zero External Dependencies)
// ---------------------------------------------------------------------------

/// Standard 64-bit complex number representation for baseband signal processing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex64 {
    pub re: f64,
    pub im: f64,
}

impl Complex64 {
    pub const ZERO: Complex64 = Complex64 { re: 0.0, im: 0.0 };
    pub const ONE: Complex64 = Complex64 { re: 1.0, im: 0.0 };
    pub const I: Complex64 = Complex64 { re: 0.0, im: 1.0 };

    pub fn new(re: f64, im: f64) -> Self {
        Complex64 { re, im }
    }

    /// Construct complex exponential $e^{j \theta} = \cos(\theta) + j \sin(\theta)$.
    pub fn from_polar(r: f64, theta: f64) -> Self {
        Complex64 {
            re: r * theta.cos(),
            im: r * theta.sin(),
        }
    }

    /// Complex conjugate $z^* = a - j b$.
    pub fn conj(&self) -> Self {
        Complex64 {
            re: self.re,
            im: -self.im,
        }
    }

    /// Squared Euclidean norm $|z|^2 = a^2 + b^2$.
    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Magnitude $|z| = \sqrt{a^2 + b^2}$.
    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Phase angle $\theta = \operatorname{atan2}(b, a) \in (-\pi, \pi]$.
    pub fn arg(&self) -> f64 {
        self.im.atan2(self.re)
    }

    /// Rotate by angle $\phi$: $z \cdot e^{j \phi}$.
    pub fn rotate(&self, phi: f64) -> Self {
        let rot = Complex64::from_polar(1.0, phi);
        *self * rot
    }
}

impl Add for Complex64 {
    type Output = Complex64;
    fn add(self, rhs: Complex64) -> Complex64 {
        Complex64 {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl Sub for Complex64 {
    type Output = Complex64;
    fn sub(self, rhs: Complex64) -> Complex64 {
        Complex64 {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl Mul for Complex64 {
    type Output = Complex64;
    fn mul(self, rhs: Complex64) -> Complex64 {
        Complex64 {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl Mul<f64> for Complex64 {
    type Output = Complex64;
    fn mul(self, rhs: f64) -> Complex64 {
        Complex64 {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl Div<f64> for Complex64 {
    type Output = Complex64;
    fn div(self, rhs: f64) -> Complex64 {
        Complex64 {
            re: self.re / rhs,
            im: self.im / rhs,
        }
    }
}

// ---------------------------------------------------------------------------
// PT-RS Densities & Thresholds (TS 38.214 §5.1.6.3 / §6.2.3.1)
// ---------------------------------------------------------------------------

/// PT-RS Time Density ($L_{PT-RS} \in \{1, 2, 4\}$ or Disabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PtrsTimeDensity {
    Disabled = 0,
    Every4Symbols = 4,
    Every2Symbols = 2,
    Every1Symbol = 1,
}

impl PtrsTimeDensity {
    pub fn step(&self) -> usize {
        *self as usize
    }

    pub fn is_enabled(&self) -> bool {
        *self != PtrsTimeDensity::Disabled
    }
}

/// PT-RS Frequency Density ($K_{PT-RS} \in \{2, 4\}$ or Disabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PtrsFrequencyDensity {
    Disabled = 0,
    Every4PRBs = 4,
    Every2PRBs = 2,
}

impl PtrsFrequencyDensity {
    pub fn step_prb(&self) -> usize {
        *self as usize
    }

    pub fn is_enabled(&self) -> bool {
        *self != PtrsFrequencyDensity::Disabled
    }
}

/// Frequency band configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtrsFrequencyBand {
    Fr1Sub6G,
    Fr2MmWave,   // 24.25 - 52.6 GHz
    Fr2_2SubTHz, // 52.6 - 71 GHz (3GPP Rel-17/18 higher numerology)
}

/// Waveform type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtrsWaveformType {
    CpOfdm,
    DftSOfdm,
}

/// Threshold table configuration for dynamic PT-RS density scaling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtrsThresholdConfig {
    /// MCS thresholds ptrsh-MCS1..4 (TS 38.214 Table 5.1.6.3-1)
    pub mcs_thresholds: [u8; 4],
    /// Scheduled bandwidth thresholds N_RB0, N_RB1 (TS 38.214 Table 5.1.6.3-2)
    pub n_rb_thresholds: [u32; 2],
}

impl Default for PtrsThresholdConfig {
    fn default() -> Self {
        // Standard 3GPP default values
        PtrsThresholdConfig {
            mcs_thresholds: [10, 16, 20, 29],
            n_rb_thresholds: [24, 48],
        }
    }
}

impl PtrsThresholdConfig {
    pub fn new(mcs_thresholds: [u8; 4], n_rb_thresholds: [u32; 2]) -> Result<Self, PtrsError> {
        if mcs_thresholds[0] >= mcs_thresholds[1]
            || mcs_thresholds[1] >= mcs_thresholds[2]
            || mcs_thresholds[2] >= mcs_thresholds[3]
        {
            return Err(PtrsError::InvalidThresholdConfiguration(
                "MCS thresholds must be strictly monotonic ascending".into(),
            ));
        }
        if n_rb_thresholds[0] >= n_rb_thresholds[1] {
            return Err(PtrsError::InvalidThresholdConfiguration(
                "N_RB thresholds must be strictly monotonic ascending".into(),
            ));
        }

        Ok(PtrsThresholdConfig {
            mcs_thresholds,
            n_rb_thresholds,
        })
    }

    /// Determine time density $L_{PT-RS}$ based on scheduled MCS.
    pub fn determine_time_density(&self, mcs: u8) -> PtrsTimeDensity {
        if mcs < self.mcs_thresholds[0] {
            PtrsTimeDensity::Disabled
        } else if mcs < self.mcs_thresholds[1] {
            PtrsTimeDensity::Every4Symbols
        } else if mcs < self.mcs_thresholds[2] {
            PtrsTimeDensity::Every2Symbols
        } else {
            PtrsTimeDensity::Every1Symbol
        }
    }

    /// Determine frequency density $K_{PT-RS}$ based on allocated PRBs $N_{RB}$.
    pub fn determine_frequency_density(&self, n_rb: u32) -> PtrsFrequencyDensity {
        if n_rb < self.n_rb_thresholds[0] {
            PtrsFrequencyDensity::Disabled
        } else if n_rb < self.n_rb_thresholds[1] {
            PtrsFrequencyDensity::Every4PRBs
        } else {
            PtrsFrequencyDensity::Every2PRBs
        }
    }
}

// ---------------------------------------------------------------------------
// 3GPP Pseudo-Random Gold Sequence Generator (TS 38.211 §5.2.1)
// ---------------------------------------------------------------------------

/// Standard 3GPP length-31 Gold sequence generator.
pub struct GoldSequenceGenerator {
    x1: u32,
    x2: u32,
}

impl GoldSequenceGenerator {
    pub const NC: usize = 1600;

    /// Initialize Gold sequence with standard seed c_init.
    pub fn new(c_init: u32) -> Self {
        let mut generator = GoldSequenceGenerator {
            x1: 1, // x1(0)=1, x1(n)=0 for n=1..30
            x2: c_init & 0x7FFF_FFFF,
        };

        // Advance through initial Nc=1600 steps
        for _ in 0..Self::NC {
            generator.step();
        }

        generator
    }

    /// Advance registers by one bit and return output bit c(n) = (x1 + x2) mod 2.
    pub fn step(&mut self) -> u32 {
        let bit1 = ((self.x1 >> 3) ^ self.x1) & 1;
        self.x1 = (self.x1 >> 1) | (bit1 << 30);

        let bit2 = ((self.x2 >> 3) ^ (self.x2 >> 2) ^ (self.x2 >> 1) ^ self.x2) & 1;
        self.x2 = (self.x2 >> 1) | (bit2 << 30);

        (bit1 ^ bit2) & 1
    }
}

// ---------------------------------------------------------------------------
// Downlink & Uplink CP-OFDM PT-RS Sequence & Resource Mapping (TS 38.211 §5.1.6.3)
// ---------------------------------------------------------------------------

/// PT-RS Resource Mapper and Sequence Generator.
pub struct PtrsResourceMapper;

impl PtrsResourceMapper {
    /// Calculate c_init seed for slot $n_s$ and symbol $l$ with Cell ID $N_{ID}$.
    pub fn calculate_c_init(slot_index: u32, symbol_index: u32, cell_id: u16) -> u32 {
        let n_id = cell_id as u64;
        let slot = slot_index as u64;
        let sym = symbol_index as u64;

        // c_init = (2^17 * (14 * n_s + l + 1) * (2 * N_ID + 1) + 2 * N_ID) mod 2^31
        let term1 = (1 << 17) * (14 * slot + sym + 1) * (2 * n_id + 1);
        let term2 = 2 * n_id;
        ((term1 + term2) & 0x7FFF_FFFF) as u32
    }

    /// Generate QPSK PT-RS reference symbol for sequence index $m$.
    pub fn generate_qpsk_symbol(gold: &mut GoldSequenceGenerator) -> Complex64 {
        let b0 = gold.step();
        let b1 = gold.step();

        let val_re = if b0 == 0 { 1.0 } else { -1.0 };
        let val_im = if b1 == 0 { 1.0 } else { -1.0 };

        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        Complex64::new(val_re * inv_sqrt2, val_im * inv_sqrt2)
    }

    /// Determine subcarrier offset $k_{offset} \in \{0, 2, 6, 8\}$ based on DMRS port (TS 38.211 Table 5.1.6.3-1).
    pub fn get_subcarrier_offset(dmrs_port: u16) -> usize {
        match dmrs_port {
            1000 => 0,
            1001 => 2,
            1002 => 6,
            1003 => 8,
            _ => (dmrs_port as usize) % 12,
        }
    }

    /// Map subcarrier indices $k$ (global subcarrier in scheduled bandwidth) containing PT-RS.
    /// Returns vector of global subcarrier indices.
    pub fn map_ptrs_subcarriers(
        start_prb: u32,
        num_prbs: u32,
        freq_density: PtrsFrequencyDensity,
        dmrs_port: u16,
        cell_id: u16,
    ) -> Vec<usize> {
        if !freq_density.is_enabled() {
            return Vec::new();
        }

        let step_prb = freq_density.step_prb();
        let k_offset = Self::get_subcarrier_offset(dmrs_port);
        let cell_shift = (cell_id as usize) % 6;
        let sc_in_prb = (k_offset + cell_shift) % 12;

        let mut subcarriers = Vec::new();
        for prb_idx in (0..num_prbs).step_by(step_prb) {
            let global_prb = start_prb + prb_idx;
            let global_sc = (global_prb as usize) * 12 + sc_in_prb;
            subcarriers.push(global_sc);
        }

        subcarriers
    }

    /// Check if a symbol contains PT-RS given DMRS symbol mask and time density.
    pub fn is_ptrs_symbol(
        sym: usize,
        dmrs_symbols: &[bool; 14],
        time_density: PtrsTimeDensity,
    ) -> bool {
        if !time_density.is_enabled() || dmrs_symbols[sym] {
            return false;
        }

        let step = time_density.step();
        // PT-RS is present starting from the first symbol after DMRS with interval `step`
        sym % step == 0
    }
}

// ---------------------------------------------------------------------------
// Uplink DFT-s-OFDM PT-RS Generation (TS 38.211 §6.4.1.2)
// ---------------------------------------------------------------------------

/// DFT-s-OFDM Uplink PT-RS configuration and group-chunk generator.
#[derive(Debug, Clone)]
pub struct DftSOfdmPtrsConfig {
    pub num_ptrs_groups: usize,   // N_group in {2, 4, 8}
    pub samples_per_group: usize, // M_group in {2, 4}
}

impl DftSOfdmPtrsConfig {
    pub fn new(num_ptrs_groups: usize, samples_per_group: usize) -> Self {
        DftSOfdmPtrsConfig {
            num_ptrs_groups: num_ptrs_groups.clamp(2, 8),
            samples_per_group: samples_per_group.clamp(2, 4),
        }
    }

    /// Generate low-PAPR sequence for transform precoded uplink PT-RS chunk.
    pub fn generate_chunk_sequence(&self, group_idx: usize, total_chunks: usize) -> Vec<Complex64> {
        let mut chunk = Vec::with_capacity(self.samples_per_group);
        for m in 0..self.samples_per_group {
            // Low-PAPR phase sequence: w(m) = e^{j * pi * m^2 / M}
            let phase = PI * (m as f64) * ((m + group_idx) as f64) / (total_chunks as f64);
            chunk.push(Complex64::from_polar(1.0, phase));
        }
        chunk
    }
}

// ---------------------------------------------------------------------------
// Common Phase Error (CPE) Estimator & Unwrapping Servo
// ---------------------------------------------------------------------------

/// Minimum-Variance Least-Squares Common Phase Error (CPE) Estimator.
#[derive(Debug, Clone)]
pub struct CommonPhaseErrorEstimator {
    /// Previous symbol estimated unwrapped phase (radians)
    prev_unwrapped_phase: f64,
    initialized: bool,
}

impl Default for CommonPhaseErrorEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl CommonPhaseErrorEstimator {
    pub fn new() -> Self {
        CommonPhaseErrorEstimator {
            prev_unwrapped_phase: 0.0,
            initialized: false,
        }
    }

    pub fn reset(&mut self) {
        self.prev_unwrapped_phase = 0.0;
        self.initialized = false;
    }

    /// Estimate Common Phase Error $\hat{\theta}(l)$ across all PT-RS subcarriers in symbol $l$:
    /// $\hat{P} = \sum_k Y(k, l) \cdot X^*(k, l)$
    /// $\hat{\theta}(l) = \operatorname{atan2}(\operatorname{Im}\{\hat{P}\}, \operatorname{Re}\{\hat{P}\})$
    pub fn estimate_cpe(
        &mut self,
        rx_symbols: &[Complex64],
        tx_reference: &[Complex64],
    ) -> Result<f64, PtrsError> {
        if rx_symbols.is_empty() || rx_symbols.len() != tx_reference.len() {
            return Err(PtrsError::EstimationFailure(
                "Mismatched or empty PT-RS symbol vectors".into(),
            ));
        }

        let mut sum_p = Complex64::ZERO;
        for (y, x) in rx_symbols.iter().zip(tx_reference.iter()) {
            sum_p = sum_p + (*y * x.conj());
        }

        if sum_p.norm_sq() < 1e-12 {
            return Err(PtrsError::EstimationFailure(
                "PT-RS signal energy below noise floor".into(),
            ));
        }

        let raw_phase = sum_p.arg();

        // Phase unwrap servo: track cumulative phase drift across symbols without 2*pi phase wrap
        let unwrapped_phase = if !self.initialized {
            self.initialized = true;
            raw_phase
        } else {
            let mut diff = raw_phase - (self.prev_unwrapped_phase % (2.0 * PI));
            while diff > PI {
                diff -= 2.0 * PI;
            }
            while diff < -PI {
                diff += 2.0 * PI;
            }
            self.prev_unwrapped_phase + diff
        };

        self.prev_unwrapped_phase = unwrapped_phase;
        Ok(unwrapped_phase)
    }

    pub fn current_unwrapped_phase(&self) -> f64 {
        self.prev_unwrapped_phase
    }
}

// ---------------------------------------------------------------------------
// Phase Derotator & Residual ICI Evaluator
// ---------------------------------------------------------------------------

/// Phase derotator applying $e^{-j \hat{\theta}(l)}$ across all data subcarriers in symbol $l$.
pub struct PhaseDerotator;

impl PhaseDerotator {
    /// Derotate frequency-domain symbol vector by phase angle theta.
    pub fn derotate_symbol(symbols: &mut [Complex64], estimated_phase: f64) {
        let derotator = Complex64::from_polar(1.0, -estimated_phase);
        for s in symbols.iter_mut() {
            *s = *s * derotator;
        }
    }

    /// Calculate Residual Inter-Carrier Interference (ICI) variance:
    /// $\sigma_{ICI}^2 = \frac{1}{M} \sum_{k} |Y(k) e^{-j \hat{\theta}} - X(k)|^2$.
    pub fn calculate_residual_ici(
        derotated_ptrs: &[Complex64],
        reference_ptrs: &[Complex64],
    ) -> f64 {
        if derotated_ptrs.is_empty() || derotated_ptrs.len() != reference_ptrs.len() {
            return 0.0;
        }

        let mut sum_sq = 0.0;
        for (y, x) in derotated_ptrs.iter().zip(reference_ptrs.iter()) {
            sum_sq += (*y - *x).norm_sq();
        }
        sum_sq / (derotated_ptrs.len() as f64)
    }

    /// Calculate Error Vector Magnitude (EVM) in percentage (%):
    /// $\text{EVM} = \sqrt{\frac{\sum |Y_k - X_{ref, k}|^2}{\sum |X_{ref, k}|^2}} \times 100\%$.
    pub fn calculate_evm_percent(
        symbols: &[Complex64],
        reference_constellation: &[Complex64],
    ) -> f64 {
        if symbols.is_empty() || symbols.len() != reference_constellation.len() {
            return 100.0;
        }

        let mut err_sq = 0.0;
        let mut ref_sq = 0.0;

        for (y, x) in symbols.iter().zip(reference_constellation.iter()) {
            err_sq += (*y - *x).norm_sq();
            ref_sq += x.norm_sq();
        }

        if ref_sq < 1e-12 {
            return 100.0;
        }

        (err_sq / ref_sq).sqrt() * 100.0
    }
}

// ---------------------------------------------------------------------------
// Top-Level PT-RS & Phase Noise Tracking Engine
// ---------------------------------------------------------------------------

/// Telemetry metrics for PT-RS phase tracking performance.
#[derive(Debug, Clone, PartialEq)]
pub struct PtrsMetrics {
    pub total_symbols_processed: u64,
    pub ptrs_symbols_tracked: u64,
    pub max_absolute_phase_drift_rad: f64,
    pub average_cpe_rad: f64,
    pub average_raw_evm_percent: f64,
    pub average_derotated_evm_percent: f64,
    pub average_residual_ici_variance: f64,
}

/// 3GPP Release 18 PT-RS and Phase Tracking Engine.
pub struct PtrsEngine {
    pub waveform: PtrsWaveformType,
    pub frequency_band: PtrsFrequencyBand,
    pub cell_id: u16,
    pub dmrs_port: u16,
    pub threshold_config: PtrsThresholdConfig,
    pub cpe_estimator: CommonPhaseErrorEstimator,
    pub metrics: PtrsMetrics,
    cpe_accumulator: f64,
    raw_evm_accumulator: f64,
    derotated_evm_accumulator: f64,
    ici_accumulator: f64,
}

impl PtrsEngine {
    pub fn new(
        waveform: PtrsWaveformType,
        frequency_band: PtrsFrequencyBand,
        cell_id: u16,
        dmrs_port: u16,
        threshold_config: Option<PtrsThresholdConfig>,
    ) -> Result<Self, PtrsError> {
        if cell_id > 1007 {
            return Err(PtrsError::InvalidCellId(cell_id));
        }

        Ok(PtrsEngine {
            waveform,
            frequency_band,
            cell_id,
            dmrs_port,
            threshold_config: threshold_config.unwrap_or_default(),
            cpe_estimator: CommonPhaseErrorEstimator::new(),
            metrics: PtrsMetrics {
                total_symbols_processed: 0,
                ptrs_symbols_tracked: 0,
                max_absolute_phase_drift_rad: 0.0,
                average_cpe_rad: 0.0,
                average_raw_evm_percent: 0.0,
                average_derotated_evm_percent: 0.0,
                average_residual_ici_variance: 0.0,
            },
            cpe_accumulator: 0.0,
            raw_evm_accumulator: 0.0,
            derotated_evm_accumulator: 0.0,
            ici_accumulator: 0.0,
        })
    }

    /// Process a received OFDM symbol: detect PT-RS presence, estimate CPE, derotate, and evaluate metrics.
    /// `slot`: Slot index (0..159)
    /// `symbol_idx`: OFDM symbol index (0..13)
    /// `mcs`: Scheduled MCS (0..31)
    /// `num_prbs`: Scheduled bandwidth in PRBs
    /// `dmrs_symbols`: 14-element boolean array indicating DMRS presence
    /// `rx_grid`: Received frequency-domain subcarriers for this symbol (12 * num_prbs)
    /// `tx_reference`: Transmitted reference symbols for EVM calculation
    pub fn process_symbol(
        &mut self,
        slot: u32,
        symbol_idx: usize,
        mcs: u8,
        num_prbs: u32,
        dmrs_symbols: &[bool; 14],
        rx_grid: &mut [Complex64],
        tx_reference: &[Complex64],
    ) -> Result<Option<f64>, PtrsError> {
        if mcs > 31 {
            return Err(PtrsError::InvalidMcs(mcs));
        }
        if num_prbs == 0 {
            return Err(PtrsError::InvalidPrbCount(num_prbs));
        }

        self.metrics.total_symbols_processed += 1;

        let time_density = self.threshold_config.determine_time_density(mcs);
        let freq_density = self.threshold_config.determine_frequency_density(num_prbs);

        let has_ptrs = PtrsResourceMapper::is_ptrs_symbol(symbol_idx, dmrs_symbols, time_density)
            && freq_density.is_enabled();

        if !has_ptrs {
            // If no PT-RS in this symbol, apply current tracking phase from previous symbol
            let current_phase = self.cpe_estimator.current_unwrapped_phase();
            PhaseDerotator::derotate_symbol(rx_grid, current_phase);
            return Ok(None);
        }

        // Map PT-RS subcarriers
        let ptrs_sc_indices = PtrsResourceMapper::map_ptrs_subcarriers(
            0,
            num_prbs,
            freq_density,
            self.dmrs_port,
            self.cell_id,
        );

        if ptrs_sc_indices.is_empty() {
            return Ok(None);
        }

        // Generate Gold sequence for PT-RS
        let c_init = PtrsResourceMapper::calculate_c_init(slot, symbol_idx as u32, self.cell_id);
        let mut gold = GoldSequenceGenerator::new(c_init);

        let mut tx_ptrs_samples = Vec::with_capacity(ptrs_sc_indices.len());
        let mut rx_ptrs_samples = Vec::with_capacity(ptrs_sc_indices.len());

        for &sc in ptrs_sc_indices.iter() {
            let ref_sym = PtrsResourceMapper::generate_qpsk_symbol(&mut gold);
            tx_ptrs_samples.push(ref_sym);
            if sc < rx_grid.len() {
                rx_ptrs_samples.push(rx_grid[sc]);
            }
        }

        // Calculate raw EVM before compensation
        let raw_evm = PhaseDerotator::calculate_evm_percent(rx_grid, tx_reference);

        // Estimate Common Phase Error (CPE)
        let est_phase = self
            .cpe_estimator
            .estimate_cpe(&rx_ptrs_samples, &tx_ptrs_samples)?;

        // Derotate full symbol grid
        PhaseDerotator::derotate_symbol(rx_grid, est_phase);

        // Calculate derotated EVM
        let derotated_evm = PhaseDerotator::calculate_evm_percent(rx_grid, tx_reference);

        // Extract derotated PT-RS samples to estimate residual ICI
        let mut derotated_ptrs = Vec::with_capacity(ptrs_sc_indices.len());
        for &sc in ptrs_sc_indices.iter() {
            if sc < rx_grid.len() {
                derotated_ptrs.push(rx_grid[sc]);
            }
        }
        let residual_ici =
            PhaseDerotator::calculate_residual_ici(&derotated_ptrs, &tx_ptrs_samples);

        // Update telemetry
        self.metrics.ptrs_symbols_tracked += 1;
        let abs_phase = est_phase.abs();
        if abs_phase > self.metrics.max_absolute_phase_drift_rad {
            self.metrics.max_absolute_phase_drift_rad = abs_phase;
        }

        self.cpe_accumulator += est_phase;
        self.raw_evm_accumulator += raw_evm;
        self.derotated_evm_accumulator += derotated_evm;
        self.ici_accumulator += residual_ici;

        let n = self.metrics.ptrs_symbols_tracked as f64;
        self.metrics.average_cpe_rad = self.cpe_accumulator / n;
        self.metrics.average_raw_evm_percent = self.raw_evm_accumulator / n;
        self.metrics.average_derotated_evm_percent = self.derotated_evm_accumulator / n;
        self.metrics.average_residual_ici_variance = self.ici_accumulator / n;

        Ok(Some(est_phase))
    }
}
