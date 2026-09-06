//! O-RAN WG4 Open Fronthaul Shared Cell (Single Cell ID Multi-RU) Engine.
//!
//! Implements O-RAN.WG4.CUS.0-v07.00 Section 11:
//! - Multi-RU topology sharing a single Physical Cell ID (PCI) to eliminate intra-cell handovers.
//! - Downlink distribution: O-DU multicasting U-Plane IQ data with per-RU transmission delay
//!   offsets ($D_i$) and amplitude scalings ($g_i$) ensuring coherent RF arrival at the UE.
//! - Uplink aggregation & diversity combining:
//!   - Maximum Ratio Combining (MRC): weights IQ samples by channel amplitude and aligns phase.
//!   - Equal Gain Combining (EGC): co-phases IQ samples with equal weighting.
//!   - Selection Combining (SC): selects the branch with maximum instantaneous SNR.
//! - Fronthaul arrival skew absorption window to align asynchronous multi-RU packet arrivals.
//! - Array gain ($10 \log_{10}(M)$ dB) and effective SNR improvement telemetry.
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::collections::HashMap;
use std::fmt;

/// Maximum number of O-RUs supported in a single Shared Cell cluster.
pub const MAX_RUS_PER_SHARED_CELL: usize = 16;

/// Default fronthaul arrival skew tolerance in nanoseconds (5000 ns = 5 µs).
pub const DEFAULT_SKEW_TOLERANCE_NS: u64 = 5_000;

/// Number of subcarriers per standard Physical Resource Block.
pub const SUBCARRIERS_PER_PRB: usize = 12;

/// Diversity combining scheme for uplink reception across multiple O-RUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombiningMode {
    /// Maximum Ratio Combining: optimal SNR gain proportional to sum of branch SNRs.
    MaximumRatioCombining,
    /// Equal Gain Combining: co-phases branches without requiring instantaneous noise estimation.
    EqualGainCombining,
    /// Selection Combining: chooses the single branch with highest instantaneous SNR/RSSI.
    SelectionCombining,
}

/// Errors raised during Shared Cell processing.
#[derive(Debug, Clone, PartialEq)]
pub enum SharedCellError {
    InvalidRuCount(usize),
    RuNotFound(u16),
    DuplicateRuId(u16),
    SkewToleranceExceeded { arrival_skew_ns: u64, max_ns: u64 },
    InsufficientBranches { available: usize, required: usize },
    InvalidPrbSize(usize),
}

impl fmt::Display for SharedCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SharedCellError::InvalidRuCount(n) => {
                write!(
                    f,
                    "Invalid RU count {} (must be 2..={})",
                    n, MAX_RUS_PER_SHARED_CELL
                )
            }
            SharedCellError::RuNotFound(id) => {
                write!(f, "O-RU ID 0x{:04X} not registered in Shared Cell", id)
            }
            SharedCellError::DuplicateRuId(id) => {
                write!(
                    f,
                    "Duplicate O-RU ID 0x{:04X} in Shared Cell configuration",
                    id
                )
            }
            SharedCellError::SkewToleranceExceeded {
                arrival_skew_ns,
                max_ns,
            } => {
                write!(
                    f,
                    "Fronthaul packet arrival skew {} ns exceeds tolerance {} ns",
                    arrival_skew_ns, max_ns
                )
            }
            SharedCellError::InsufficientBranches {
                available,
                required,
            } => {
                write!(
                    f,
                    "Insufficient O-RU branches for combining: available {}, required {}",
                    available, required
                )
            }
            SharedCellError::InvalidPrbSize(sz) => {
                write!(
                    f,
                    "Invalid PRB sample count {} (must be {})",
                    sz, SUBCARRIERS_PER_PRB
                )
            }
        }
    }
}

impl std::error::Error for SharedCellError {}

/// Complex IQ sample (16-bit signed integer representation common in eCPRI / O-RAN).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ComplexIq {
    pub i: f32,
    pub q: f32,
}

impl ComplexIq {
    pub fn new(i: f32, q: f32) -> Self {
        Self { i, q }
    }

    /// Computes instantaneous signal power: $I^2 + Q^2$.
    #[inline]
    pub fn power(&self) -> f32 {
        self.i * self.i + self.q * self.q
    }

    /// Computes phase angle in radians: $\text{atan2}(Q, I)$.
    #[inline]
    pub fn phase(&self) -> f32 {
        self.q.atan2(self.i)
    }

    /// Rotates sample by a phase angle in radians: $z \cdot e^{j\theta}$.
    #[inline]
    pub fn rotate(&self, theta: f32) -> Self {
        let (sin_t, cos_t) = theta.sin_cos();
        Self {
            i: self.i * cos_t - self.q * sin_t,
            q: self.i * sin_t + self.q * cos_t,
        }
    }

    /// Scales sample by a scalar amplitude factor.
    #[inline]
    pub fn scale(&self, factor: f32) -> Self {
        Self {
            i: self.i * factor,
            q: self.q * factor,
        }
    }
}

/// Profile and calibration offsets for an individual O-RU member in the Shared Cell cluster.
#[derive(Debug, Clone, PartialEq)]
pub struct RuMemberProfile {
    pub ru_id: u16,
    /// Calibrated transmission delay offset in nanoseconds ($D_i$) to align air-interface emission.
    pub dl_delay_offset_ns: i64,
    /// Downlink amplitude scaling factor ($0.0 \dots 1.0$).
    pub dl_power_scaling: f32,
    /// Estimated uplink channel SNR in dB.
    pub ul_estimated_snr_db: f32,
    /// Estimated uplink channel phase offset in radians for co-phasing combining.
    pub ul_phase_offset_rad: f32,
}

impl RuMemberProfile {
    pub fn new(
        ru_id: u16,
        dl_delay_offset_ns: i64,
        dl_power_scaling: f32,
        ul_estimated_snr_db: f32,
        ul_phase_offset_rad: f32,
    ) -> Self {
        Self {
            ru_id,
            dl_delay_offset_ns,
            dl_power_scaling: dl_power_scaling.clamp(0.0, 1.0),
            ul_estimated_snr_db,
            ul_phase_offset_rad,
        }
    }

    /// Linear SNR scale ($10^{\text{SNR}_{dB} / 10}$).
    #[inline]
    pub fn linear_snr(&self) -> f32 {
        10.0f32.powf(self.ul_estimated_snr_db / 10.0)
    }
}

/// Uplink Resource Block transmission received from a specific O-RU branch.
#[derive(Debug, Clone, PartialEq)]
pub struct RuPrbPacket {
    pub ru_id: u16,
    pub subframe: u8,
    pub slot: u8,
    pub symbol: u8,
    pub prb_idx: u16,
    pub arrival_timestamp_ns: u64,
    pub samples: [ComplexIq; SUBCARRIERS_PER_PRB],
}

/// Downlink Distributed Packet generated for transmission by an individual O-RU.
#[derive(Debug, Clone, PartialEq)]
pub struct RuDlDistributedPacket {
    pub ru_id: u16,
    pub subframe: u8,
    pub slot: u8,
    pub symbol: u8,
    pub prb_idx: u16,
    pub target_transmit_timestamp_ns: u64,
    pub samples: [ComplexIq; SUBCARRIERS_PER_PRB],
}

/// Operational Telemetry for the Shared Cell Cluster.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SharedCellMetrics {
    pub total_dl_packets_distributed: u64,
    pub total_ul_packets_aggregated: u64,
    pub combined_prb_blocks: u64,
    pub dropped_packets_skew_violation: u64,
    pub theoretical_array_gain_db: f32,
    pub average_post_combining_snr_db: f32,
}

/// O-RAN WG4 Shared Cell Management Engine.
#[derive(Debug, Clone)]
pub struct SharedCellEngine {
    pub cell_id: u16,
    pub combining_mode: CombiningMode,
    pub skew_tolerance_ns: u64,
    members: HashMap<u16, RuMemberProfile>,
    metrics: SharedCellMetrics,
}

impl SharedCellEngine {
    pub fn new(cell_id: u16, combining_mode: CombiningMode, skew_tolerance_ns: u64) -> Self {
        Self {
            cell_id,
            combining_mode,
            skew_tolerance_ns,
            members: HashMap::new(),
            metrics: SharedCellMetrics::default(),
        }
    }

    /// Registers a new O-RU member into the Shared Cell cluster.
    pub fn add_ru_member(&mut self, profile: RuMemberProfile) -> Result<(), SharedCellError> {
        if self.members.len() >= MAX_RUS_PER_SHARED_CELL {
            return Err(SharedCellError::InvalidRuCount(self.members.len() + 1));
        }
        if self.members.contains_key(&profile.ru_id) {
            return Err(SharedCellError::DuplicateRuId(profile.ru_id));
        }
        self.members.insert(profile.ru_id, profile);
        self.update_array_gain();
        Ok(())
    }

    /// Number of active O-RUs in the cluster.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Updates theoretical array gain: $10 \log_{10}(M)\text{ dB}$.
    fn update_array_gain(&mut self) {
        let m = self.members.len() as f32;
        if m > 0.0 {
            self.metrics.theoretical_array_gain_db = 10.0 * m.log10();
        } else {
            self.metrics.theoretical_array_gain_db = 0.0;
        }
    }

    /// Distributes a master downlink PRB across all registered O-RUs.
    /// Applies calibrated delay offsets and amplitude scalings per RU.
    pub fn distribute_downlink_prb(
        &mut self,
        subframe: u8,
        slot: u8,
        symbol: u8,
        prb_idx: u16,
        nominal_air_timestamp_ns: u64,
        master_samples: &[ComplexIq; SUBCARRIERS_PER_PRB],
    ) -> Result<Vec<RuDlDistributedPacket>, SharedCellError> {
        if self.members.is_empty() {
            return Err(SharedCellError::InsufficientBranches {
                available: 0,
                required: 1,
            });
        }

        let mut distributed = Vec::with_capacity(self.members.len());

        for profile in self.members.values() {
            // Apply delay offset: target transmit timestamp = nominal_air - delay_offset
            let target_ts = if profile.dl_delay_offset_ns >= 0 {
                nominal_air_timestamp_ns.saturating_sub(profile.dl_delay_offset_ns as u64)
            } else {
                nominal_air_timestamp_ns.saturating_add((-profile.dl_delay_offset_ns) as u64)
            };

            // Scale samples by amplitude scaling factor
            let mut scaled_samples = [ComplexIq::default(); SUBCARRIERS_PER_PRB];
            for (idx, sample) in master_samples.iter().enumerate() {
                scaled_samples[idx] = sample.scale(profile.dl_power_scaling);
            }

            distributed.push(RuDlDistributedPacket {
                ru_id: profile.ru_id,
                subframe,
                slot,
                symbol,
                prb_idx,
                target_transmit_timestamp_ns: target_ts,
                samples: scaled_samples,
            });
            self.metrics.total_dl_packets_distributed += 1;
        }

        Ok(distributed)
    }

    /// Aggregates and combines uplink PRB packets from multiple O-RUs using the configured diversity algorithm.
    pub fn aggregate_uplink_prb(
        &mut self,
        packets: &[RuPrbPacket],
    ) -> Result<[ComplexIq; SUBCARRIERS_PER_PRB], SharedCellError> {
        if packets.is_empty() {
            return Err(SharedCellError::InsufficientBranches {
                available: 0,
                required: 1,
            });
        }

        // 1. Skew validation across all incoming branches
        let mut min_ts = u64::MAX;
        let mut max_ts = 0u64;
        for pkt in packets {
            if pkt.arrival_timestamp_ns < min_ts {
                min_ts = pkt.arrival_timestamp_ns;
            }
            if pkt.arrival_timestamp_ns > max_ts {
                max_ts = pkt.arrival_timestamp_ns;
            }
        }
        let skew = max_ts.saturating_sub(min_ts);
        if skew > self.skew_tolerance_ns {
            self.metrics.dropped_packets_skew_violation += packets.len() as u64;
            return Err(SharedCellError::SkewToleranceExceeded {
                arrival_skew_ns: skew,
                max_ns: self.skew_tolerance_ns,
            });
        }

        self.metrics.total_ul_packets_aggregated += packets.len() as u64;
        self.metrics.combined_prb_blocks += 1;

        // 2. Perform diversity combining
        let combined = match self.combining_mode {
            CombiningMode::SelectionCombining => self.combine_selection(packets)?,
            CombiningMode::EqualGainCombining => self.combine_egc(packets)?,
            CombiningMode::MaximumRatioCombining => self.combine_mrc(packets)?,
        };

        Ok(combined)
    }

    /// Selection Combining: picks the branch with highest estimated SNR.
    fn combine_selection(
        &self,
        packets: &[RuPrbPacket],
    ) -> Result<[ComplexIq; SUBCARRIERS_PER_PRB], SharedCellError> {
        let mut best_snr = f32::NEG_INFINITY;
        let mut best_pkt: Option<&RuPrbPacket> = None;

        for pkt in packets {
            let snr = self
                .members
                .get(&pkt.ru_id)
                .map(|p| p.ul_estimated_snr_db)
                .unwrap_or(0.0);
            if snr > best_snr {
                best_snr = snr;
                best_pkt = Some(pkt);
            }
        }

        match best_pkt {
            Some(pkt) => Ok(pkt.samples),
            None => Err(SharedCellError::InsufficientBranches {
                available: 0,
                required: 1,
            }),
        }
    }

    /// Equal Gain Combining: co-phases each branch sample and averages with equal weight.
    fn combine_egc(
        &self,
        packets: &[RuPrbPacket],
    ) -> Result<[ComplexIq; SUBCARRIERS_PER_PRB], SharedCellError> {
        let m = packets.len() as f32;
        let inv_sqrt_m = 1.0 / m.sqrt();
        let mut result = [ComplexIq::default(); SUBCARRIERS_PER_PRB];

        for sc in 0..SUBCARRIERS_PER_PRB {
            let mut sum_i = 0.0f32;
            let mut sum_q = 0.0f32;

            for pkt in packets {
                let phase_offset = self
                    .members
                    .get(&pkt.ru_id)
                    .map(|p| p.ul_phase_offset_rad)
                    .unwrap_or(0.0);

                // Co-phase: rotate by -phase_offset
                let co_phased = pkt.samples[sc].rotate(-phase_offset);
                sum_i += co_phased.i;
                sum_q += co_phased.q;
            }

            result[sc] = ComplexIq::new(sum_i * inv_sqrt_m, sum_q * inv_sqrt_m);
        }

        Ok(result)
    }

    /// Maximum Ratio Combining: weights each branch by $\sqrt{\text{SNR}}$ and co-phases.
    fn combine_mrc(
        &self,
        packets: &[RuPrbPacket],
    ) -> Result<[ComplexIq; SUBCARRIERS_PER_PRB], SharedCellError> {
        let mut weights = Vec::with_capacity(packets.len());
        let mut norm_factor = 0.0f32;

        for pkt in packets {
            let (linear_snr, phase) = match self.members.get(&pkt.ru_id) {
                Some(p) => (p.linear_snr(), p.ul_phase_offset_rad),
                None => (1.0, 0.0),
            };
            let weight = linear_snr.sqrt();
            weights.push((weight, phase));
            norm_factor += weight * weight;
        }

        let inv_norm = if norm_factor > 1e-6 {
            1.0 / norm_factor.sqrt()
        } else {
            1.0
        };

        let mut result = [ComplexIq::default(); SUBCARRIERS_PER_PRB];

        for sc in 0..SUBCARRIERS_PER_PRB {
            let mut sum_i = 0.0f32;
            let mut sum_q = 0.0f32;

            for (idx, pkt) in packets.iter().enumerate() {
                let (w, phase) = weights[idx];
                let co_phased = pkt.samples[sc].rotate(-phase);
                sum_i += co_phased.i * w;
                sum_q += co_phased.q * w;
            }

            result[sc] = ComplexIq::new(sum_i * inv_norm, sum_q * inv_norm);
        }

        Ok(result)
    }

    /// Returns current telemetry metrics.
    pub fn metrics(&self) -> &SharedCellMetrics {
        &self.metrics
    }
}
