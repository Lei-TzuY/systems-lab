//! 3GPP Release 18 (5G-Advanced) Multi-Panel Uplink Transmit Switching (UL-Tx-Switching)
//! & Sounding Reference Signal (SRS) Resource Allocation Engine.
//!
//! Standards Reference:
//! - 3GPP TS 38.211 §6.4.1.4: Sounding Reference Signal (SRS) physical structure
//! - 3GPP TS 38.214 §6.2.1: UE Sounding Procedure and SRS resource sets
//! - 3GPP TS 38.214 §6.2.1.2: UE Sounding Procedure for Uplink Transmit Switching
//! - 3GPP TS 38.306 §4.2.7: Radio Access Capability for UL Tx Switching
//!
//! This module implements:
//! 1. UE antenna capability profiles: 1T2R, 1T4R, 2T4R, and Dual-Band Carrier Aggregation (1T+1T <-> 2T+0T).
//! 2. SRS physical transmission comb configurations (Comb-2, Comb-4, Comb-8) and cyclic shifts.
//! 3. Deterministic frequency hopping tree across Bandwidth Parts (BWPs).
//! 4. Hardware switching guard period enforcement ($T_{switch} \in \{14, 28, 35\}\ \mu\text{s}$).
//! 5. Sounding-to-PUSCH/PUCCH collision arbitration and symbol puncture management.
//! 6. Reciprocity-based channel matrix reconstruction and SVD dominant beamformer derivation.
//! 7. Multi-carrier UL Tx switching telemetry and reciprocity gain evaluation.

use std::fmt;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered in UL Transmit Switching and SRS management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UlTxSwitchingError {
    InvalidAntennaCount(u8),
    InvalidCombNumber(u8),
    InvalidCyclicShift(u8),
    InvalidPortCount(u8),
    InvalidResourceSetUsage(String),
    GuardIntervalViolation {
        slot: u32,
        symbol: u8,
        reason: String,
    },
    SwitchingConflict {
        slot: u32,
        channel: String,
    },
    CalibrationFailure(String),
}

impl fmt::Display for UlTxSwitchingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UlTxSwitchingError::InvalidAntennaCount(n) => {
                write!(f, "Invalid antenna count: {n} (must be 1, 2, or 4)")
            }
            UlTxSwitchingError::InvalidCombNumber(c) => {
                write!(f, "Invalid transmission comb: {c} (must be 2, 4, or 8)")
            }
            UlTxSwitchingError::InvalidCyclicShift(cs) => {
                write!(f, "Invalid cyclic shift index: {cs}")
            }
            UlTxSwitchingError::InvalidPortCount(p) => {
                write!(f, "Invalid SRS port count: {p} (must be 1, 2, or 4)")
            }
            UlTxSwitchingError::InvalidResourceSetUsage(u) => {
                write!(f, "Invalid resource set usage: {u}")
            }
            UlTxSwitchingError::GuardIntervalViolation {
                slot,
                symbol,
                reason,
            } => {
                write!(
                    f,
                    "Guard interval violation at slot {slot}, symbol {symbol}: {reason}"
                )
            }
            UlTxSwitchingError::SwitchingConflict { slot, channel } => {
                write!(
                    f,
                    "Switching conflict at slot {slot} with higher-priority channel: {channel}"
                )
            }
            UlTxSwitchingError::CalibrationFailure(msg) => {
                write!(f, "Reciprocity calibration failure: {msg}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UE Antenna & Switching Capabilities (TS 38.306 §4.2.7)
// ---------------------------------------------------------------------------

/// UE Uplink Transmit Switching capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UlTxSwitchingCapability {
    /// 1 Tx chain switching between 2 Rx antennas (1T2R)
    OneTxTwoRx,
    /// 1 Tx chain switching among 4 Rx antennas (1T4R)
    OneTxFourRx,
    /// 2 Tx chains switching between 2 pairs of antennas (2T4R)
    TwoTxFourRx,
    /// Dual-band CA Tx switching: 1 Tx Band A + 1 Tx Band B <-> 2 Tx Band A (or Band B)
    DualBandCarrierAggregation,
}

impl UlTxSwitchingCapability {
    pub fn total_rx_antennas(&self) -> usize {
        match self {
            UlTxSwitchingCapability::OneTxTwoRx => 2,
            UlTxSwitchingCapability::OneTxFourRx => 4,
            UlTxSwitchingCapability::TwoTxFourRx => 4,
            UlTxSwitchingCapability::DualBandCarrierAggregation => 4,
        }
    }

    pub fn simultaneous_tx_chains(&self) -> usize {
        match self {
            UlTxSwitchingCapability::OneTxTwoRx => 1,
            UlTxSwitchingCapability::OneTxFourRx => 1,
            UlTxSwitchingCapability::TwoTxFourRx => 2,
            UlTxSwitchingCapability::DualBandCarrierAggregation => 2,
        }
    }

    pub fn required_srs_resources(&self) -> usize {
        match self {
            UlTxSwitchingCapability::OneTxTwoRx => 2,
            UlTxSwitchingCapability::OneTxFourRx => 4,
            UlTxSwitchingCapability::TwoTxFourRx => 2,
            UlTxSwitchingCapability::DualBandCarrierAggregation => 2,
        }
    }
}

/// Switching hardware transition guard period (TS 38.214 §6.2.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchingPeriodUs {
    Guard14Us = 14,
    Guard28Us = 28,
    Guard35Us = 35,
}

impl SwitchingPeriodUs {
    pub fn duration_us(&self) -> u32 {
        *self as u32
    }

    /// Calculate required guard symbols given numerology subcarrier spacing (kHz).
    pub fn required_guard_symbols(&self, scs_khz: u32) -> u8 {
        // Approximate symbol duration including cyclic prefix
        let symbol_duration_us = 1000.0 / (14.0 * (scs_khz as f64 / 15.0));
        let num_symbols = ((self.duration_us() as f64) / symbol_duration_us).ceil() as u8;
        num_symbols.max(1)
    }
}

// ---------------------------------------------------------------------------
// SRS Physical Structure (TS 38.211 §6.4.1.4)
// ---------------------------------------------------------------------------

/// SRS Transmission Comb ($K_{TC} \in \{2, 4, 8\}$).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrsCombStructure {
    Comb2 { offset: u8 },
    Comb4 { offset: u8 },
    Comb8 { offset: u8 },
}

impl SrsCombStructure {
    pub fn new(comb_num: u8, offset: u8) -> Result<Self, UlTxSwitchingError> {
        match comb_num {
            2 => {
                if offset < 2 {
                    Ok(SrsCombStructure::Comb2 { offset })
                } else {
                    Err(UlTxSwitchingError::InvalidCombNumber(comb_num))
                }
            }
            4 => {
                if offset < 4 {
                    Ok(SrsCombStructure::Comb4 { offset })
                } else {
                    Err(UlTxSwitchingError::InvalidCombNumber(comb_num))
                }
            }
            8 => {
                if offset < 8 {
                    Ok(SrsCombStructure::Comb8 { offset })
                } else {
                    Err(UlTxSwitchingError::InvalidCombNumber(comb_num))
                }
            }
            other => Err(UlTxSwitchingError::InvalidCombNumber(other)),
        }
    }

    pub fn k_tc(&self) -> usize {
        match self {
            SrsCombStructure::Comb2 { .. } => 2,
            SrsCombStructure::Comb4 { .. } => 4,
            SrsCombStructure::Comb8 { .. } => 8,
        }
    }

    pub fn offset(&self) -> usize {
        match self {
            SrsCombStructure::Comb2 { offset }
            | SrsCombStructure::Comb4 { offset }
            | SrsCombStructure::Comb8 { offset } => *offset as usize,
        }
    }

    /// Maximum orthogonal cyclic shifts for this comb.
    pub fn max_cyclic_shifts(&self) -> usize {
        match self {
            SrsCombStructure::Comb2 { .. } => 8,
            SrsCombStructure::Comb4 { .. } => 12,
            SrsCombStructure::Comb8 { .. } => 6,
        }
    }

    /// Subcarriers allocated for SRS within 1 PRB (12 subcarriers).
    pub fn subcarriers_per_prb(&self) -> usize {
        12 / self.k_tc()
    }

    /// Generates the subcarrier indices within 1 PRB for this comb.
    pub fn subcarrier_indices_in_prb(&self) -> Vec<usize> {
        let step = self.k_tc();
        let off = self.offset();
        (off..12).step_by(step).collect()
    }
}

/// SRS Resource Usage Type (TS 38.214 §6.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrsResourceUsage {
    BeamManagement,
    Codebook,
    NonCodebook,
    AntennaSwitching,
    Positioning,
}

/// Time domain periodicity of SRS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrsTimeDomainBehavior {
    Aperiodic,
    SemiPersistent {
        periodicity_slots: u32,
        offset_slots: u32,
    },
    Periodic {
        periodicity_slots: u32,
        offset_slots: u32,
    },
}

/// Individual SRS Resource definition.
#[derive(Debug, Clone)]
pub struct SrsResource {
    pub resource_id: u8,
    pub num_ports: u8,
    pub comb: SrsCombStructure,
    pub cyclic_shift: u8,
    pub start_symbol: u8, // 0..13 (typically last 1 to 4 symbols of slot)
    pub num_symbols: u8,  // 1, 2, or 4 consecutive symbols
    pub bwp_start_prb: u32,
    pub num_prbs: u32,
    pub antenna_port_mapping: Vec<usize>, // Antenna indices sounding this resource
}

impl SrsResource {
    pub fn new(
        resource_id: u8,
        num_ports: u8,
        comb: SrsCombStructure,
        cyclic_shift: u8,
        start_symbol: u8,
        num_symbols: u8,
        bwp_start_prb: u32,
        num_prbs: u32,
        antenna_port_mapping: Vec<usize>,
    ) -> Result<Self, UlTxSwitchingError> {
        if num_ports != 1 && num_ports != 2 && num_ports != 4 {
            return Err(UlTxSwitchingError::InvalidPortCount(num_ports));
        }
        if (cyclic_shift as usize) >= comb.max_cyclic_shifts() {
            return Err(UlTxSwitchingError::InvalidCyclicShift(cyclic_shift));
        }
        if start_symbol + num_symbols > 14 {
            return Err(UlTxSwitchingError::GuardIntervalViolation {
                slot: 0,
                symbol: start_symbol,
                reason: "SRS extends beyond slot boundary".into(),
            });
        }

        Ok(SrsResource {
            resource_id,
            num_ports,
            comb,
            cyclic_shift,
            start_symbol,
            num_symbols,
            bwp_start_prb,
            num_prbs,
            antenna_port_mapping,
        })
    }
}

/// Group of SRS resources configured for a specific usage.
#[derive(Debug, Clone)]
pub struct SrsResourceSet {
    pub set_id: u8,
    pub usage: SrsResourceUsage,
    pub time_behavior: SrsTimeDomainBehavior,
    pub resources: Vec<SrsResource>,
}

// ---------------------------------------------------------------------------
// Frequency Hopping Engine (TS 38.211 §6.4.1.4.3)
// ---------------------------------------------------------------------------

/// Tree-based SRS Frequency Hopping Calculator.
pub struct SrsFrequencyHopper;

impl SrsFrequencyHopper {
    /// Calculate current subband PRB start index for hopping level $b$ at transmission counter $n_{SRS}$.
    pub fn calculate_hopping_prb(
        b_hop: u8,
        b_srs: u8,
        n_srs: u32,
        num_subbands: u32,
        subband_size_prbs: u32,
    ) -> u32 {
        if b_hop >= b_srs || num_subbands == 0 {
            // Frequency hopping disabled
            0
        } else {
            let subband_idx = (n_srs) % num_subbands;
            subband_idx * subband_size_prbs
        }
    }
}

// ---------------------------------------------------------------------------
// Reciprocity Channel Reconstruction & SVD Beamformer
// ---------------------------------------------------------------------------

/// Complex number for MIMO channel operations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReciprocityComplex {
    pub re: f64,
    pub im: f64,
}

impl ReciprocityComplex {
    pub fn new(re: f64, im: f64) -> Self {
        ReciprocityComplex { re, im }
    }

    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }
}

/// Reconstructed multi-antenna downlink reciprocal channel.
#[derive(Debug, Clone)]
pub struct ReciprocalChannelProfile {
    pub num_antennas: usize,
    /// Channel coefficients for each antenna path
    pub channel_vector: Vec<ReciprocityComplex>,
    /// Optimal beamforming weights derived from SVD/Maximum Ratio Transmission (MRT)
    pub beamforming_weights: Vec<ReciprocityComplex>,
    /// Theoretical array gain (dB)
    pub array_gain_db: f64,
}

// ---------------------------------------------------------------------------
// Uplink Transmit Switching Engine
// ---------------------------------------------------------------------------

/// Telemetry metrics for UL Tx switching and sounding operations.
#[derive(Debug, Clone, PartialEq)]
pub struct UlTxSwitchingMetrics {
    pub total_srs_transmitted: u64,
    pub total_switch_events: u64,
    pub guard_symbols_punctured: u64,
    pub pusch_sounding_conflicts_resolved: u64,
    pub average_reciprocity_gain_db: f64,
}

/// 3GPP Release 18 Multi-Panel Uplink Transmit Switching & SRS Engine.
pub struct UlTxSwitchingEngine {
    pub capability: UlTxSwitchingCapability,
    pub switching_period: SwitchingPeriodUs,
    pub scs_khz: u32,
    pub resource_sets: Vec<SrsResourceSet>,
    /// Current active antenna port set (e.g. [0] or [0, 1])
    pub current_active_antennas: Vec<usize>,
    pub metrics: UlTxSwitchingMetrics,
    reciprocity_gain_sum: f64,
}

impl UlTxSwitchingEngine {
    pub fn new(
        capability: UlTxSwitchingCapability,
        switching_period: SwitchingPeriodUs,
        scs_khz: u32,
    ) -> Self {
        let initial_antennas = match capability {
            UlTxSwitchingCapability::OneTxTwoRx | UlTxSwitchingCapability::OneTxFourRx => vec![0],
            UlTxSwitchingCapability::TwoTxFourRx => vec![0, 1],
            UlTxSwitchingCapability::DualBandCarrierAggregation => vec![0, 1],
        };

        UlTxSwitchingEngine {
            capability,
            switching_period,
            scs_khz,
            resource_sets: Vec::new(),
            current_active_antennas: initial_antennas,
            metrics: UlTxSwitchingMetrics {
                total_srs_transmitted: 0,
                total_switch_events: 0,
                guard_symbols_punctured: 0,
                pusch_sounding_conflicts_resolved: 0,
                average_reciprocity_gain_db: 0.0,
            },
            reciprocity_gain_sum: 0.0,
        }
    }

    /// Add an SRS resource set.
    pub fn add_resource_set(&mut self, set: SrsResourceSet) {
        self.resource_sets.push(set);
    }

    /// Perform antenna switching to target antenna set.
    /// Returns true if an RF switch occurred (requiring guard interval).
    pub fn switch_antenna_path(&mut self, target_antennas: Vec<usize>) -> bool {
        if self.current_active_antennas != target_antennas {
            self.current_active_antennas = target_antennas;
            self.metrics.total_switch_events += 1;
            true
        } else {
            false
        }
    }

    /// Evaluate transmission of an SRS resource in a given slot and symbol.
    /// `has_pusch_prior_symbol`: Whether preceding symbol has PUSCH transmission.
    /// `is_pusch_critical_urllc`: If true, PUSCH cannot be punctured for periodic SRS.
    pub fn schedule_srs_transmission(
        &mut self,
        _slot: u32,
        resource_id: u8,
        has_pusch_prior_symbol: bool,
        is_pusch_critical_urllc: bool,
    ) -> Result<bool, UlTxSwitchingError> {
        let resource = self
            .resource_sets
            .iter()
            .flat_map(|s| &s.resources)
            .find(|r| r.resource_id == resource_id)
            .cloned()
            .ok_or_else(|| {
                UlTxSwitchingError::InvalidResourceSetUsage("Resource not found".into())
            })?;

        // Check if an antenna switch is required
        let needs_switch = self.current_active_antennas != resource.antenna_port_mapping;

        if needs_switch {
            let guard_symbols = self.switching_period.required_guard_symbols(self.scs_khz);

            if has_pusch_prior_symbol {
                if is_pusch_critical_urllc {
                    // Critical URLLC PUSCH takes precedence: cancel sounding
                    self.metrics.pusch_sounding_conflicts_resolved += 1;
                    return Ok(false);
                } else {
                    // Puncture prior symbol PUSCH to accommodate RF switch guard
                    self.metrics.guard_symbols_punctured += guard_symbols as u64;
                    self.metrics.pusch_sounding_conflicts_resolved += 1;
                }
            }

            self.switch_antenna_path(resource.antenna_port_mapping);
        }

        self.metrics.total_srs_transmitted += 1;
        Ok(true)
    }

    /// Reconstruct full DL channel matrix from sounded SRS channel vectors across all antennas.
    /// Derives optimal SVD / Maximum Ratio Transmission (MRT) beamforming weights.
    pub fn reconstruct_reciprocal_channel(
        &mut self,
        sounded_channel_samples: &[(usize, ReciprocityComplex)], // (antenna_idx, h)
    ) -> Result<ReciprocalChannelProfile, UlTxSwitchingError> {
        let num_rx = self.capability.total_rx_antennas();
        if sounded_channel_samples.len() < num_rx {
            return Err(UlTxSwitchingError::CalibrationFailure(format!(
                "Incomplete antenna sounding: got {} paths, expected {}",
                sounded_channel_samples.len(),
                num_rx
            )));
        }

        let mut channel_vec = vec![ReciprocityComplex::new(0.0, 0.0); num_rx];
        for &(ant_idx, h) in sounded_channel_samples {
            if ant_idx < num_rx {
                channel_vec[ant_idx] = h;
            }
        }

        // Derive MRT beamforming weights: w = h* / ||h||
        let mut norm_sq = 0.0;
        for h in &channel_vec {
            norm_sq += h.norm_sq();
        }

        let norm = norm_sq.sqrt().max(1e-12);
        let mut beamforming_weights = Vec::with_capacity(num_rx);
        for h in &channel_vec {
            // Conjugate normalized
            beamforming_weights.push(ReciprocityComplex::new(h.re / norm, -h.im / norm));
        }

        // Theoretical array gain: 10 * log10(num_rx)
        let array_gain_db = 10.0 * (num_rx as f64).log10();

        self.reciprocity_gain_sum += array_gain_db;
        let count = (self.metrics.total_switch_events + 1) as f64;
        self.metrics.average_reciprocity_gain_db = self.reciprocity_gain_sum / count;

        Ok(ReciprocalChannelProfile {
            num_antennas: num_rx,
            channel_vector: channel_vec,
            beamforming_weights,
            array_gain_db,
        })
    }
}
