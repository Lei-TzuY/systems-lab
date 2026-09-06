//! 3GPP Release 18 (5G-Advanced) Subband Non-Overlapping Full Duplex (SBFD) Engine.
//!
//! Conforms to:
//! - 3GPP TR 38.858 Rel-18: Study on evolution of NR duplex operation (Subband full duplex).
//! - 3GPP TS 38.211 Rel-18: Physical channels and modulation - SBFD slot structure and subband grid.
//! - 3GPP TS 38.213 Rel-18: Physical layer procedures - SBFD slot format and scheduling.
//! - 3GPP TS 38.214 Rel-18: Physical layer procedures for data - SBFD link adaptation and MCS.
//! - 3GPP TS 38.331 Rel-18: Radio Resource Control (RRC) - SBFD configuration and assistance info.
//!
//! Features:
//! 1. SBFD slot structure with non-overlapping DL/UL subbands separated by guard bands.
//! 2. Multi-stage Self-Interference Cancellation (SIC): Spatial/Antenna, Analog RF, and Digital baseband.
//! 3. Residual Self-Interference (RSI) power and noise floor rise calculation ($\Delta N_0$).
//! 4. Cross-Link Interference (CLI) modeling: gNodeB-to-gNodeB LoS and UE-to-UE NLoS interference.
//! 5. Ultra-low latency SBFD scheduler: eliminates TDD slot alignment wait times for urgent URLLC/pose traffic.
//! 6. SBFD link adaptation and 3GPP MCS table mapping under RSI and CLI interference.
//! 7. Comprehensive telemetry tracking latency reduction, spectral efficiency gain, and SIC efficiency.
//!
//! Pure standard Rust with zero external dependencies.

use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Standard thermal noise density at room temperature in dBm/Hz (kT at 290 K).
pub const THERMAL_NOISE_DENSITY_DBM_HZ: f64 = -174.0;
pub const SBFD_THERMAL_NOISE_DENSITY_DBM_HZ: f64 = THERMAL_NOISE_DENSITY_DBM_HZ;

/// Default subcarrier spacing for Band n78 (30 kHz, numerology $\mu = 1$).
pub const DEFAULT_SCS_HZ: f64 = 30_000.0;
pub const SBFD_DEFAULT_SCS_HZ: f64 = DEFAULT_SCS_HZ;

/// Number of subcarriers per Physical Resource Block (PRB).
pub const SUBCARRIERS_PER_PRB: usize = 12;
pub const SBFD_SUBCARRIERS_PER_PRB: usize = SUBCARRIERS_PER_PRB;

/// Standard PRB bandwidth for 30 kHz SCS: $12 \times 30\text{ kHz} = 360\text{ kHz}$.
pub const PRB_BANDWIDTH_30KHZ_HZ: f64 = 360_000.0;
pub const SBFD_PRB_BANDWIDTH_30KHZ_HZ: f64 = PRB_BANDWIDTH_30KHZ_HZ;

/// Standard 100 MHz NR carrier PRB count for 30 kHz SCS.
pub const MAX_PRBS_100MHZ_30KHZ: u16 = 273;
pub const SBFD_MAX_PRBS_100MHZ_30KHZ: u16 = MAX_PRBS_100MHZ_30KHZ;

/// Default minimum guard band in PRBs between DL and UL subbands to prevent ACLR leakage.
pub const DEFAULT_MIN_GUARD_PRBS: u16 = 6;
pub const SBFD_DEFAULT_MIN_GUARD_PRBS: u16 = DEFAULT_MIN_GUARD_PRBS;

/// Default gNodeB transmit power in dBm (46 dBm = 40 Watts).
pub const DEFAULT_GNB_TX_POWER_DBM: f64 = 46.0;
pub const SBFD_DEFAULT_GNB_TX_POWER_DBM: f64 = DEFAULT_GNB_TX_POWER_DBM;

/// Default UE transmit power in dBm (23 dBm = 200 mW, Power Class 3).
pub const DEFAULT_UE_TX_POWER_DBM: f64 = 23.0;
pub const SBFD_DEFAULT_UE_TX_POWER_DBM: f64 = DEFAULT_UE_TX_POWER_DBM;

/// Maximum tolerable residual self-interference power in dBm before receiver saturation.
pub const MAX_TOLERABLE_RSI_DBM: f64 = -85.0;
pub const SBFD_MAX_TOLERABLE_RSI_DBM: f64 = MAX_TOLERABLE_RSI_DBM;

// ---------------------------------------------------------------------------
// SBFD Subband & Slot Structures
// ---------------------------------------------------------------------------

/// Subband operational direction within an SBFD slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SbfdSubbandType {
    /// Dedicated Downlink transmission subband.
    Downlink,
    /// Dedicated Uplink reception subband.
    Uplink,
    /// Guard band between DL and UL to protect against ACLR leakage.
    GuardBand,
    /// Flexible subband configured dynamically via DCI format 2_0.
    Flexible,
}

/// SBFD subband allocation definition.
#[derive(Debug, Clone, PartialEq)]
pub struct SbfdSubband {
    pub subband_id: u8,
    pub subband_type: SbfdSubbandType,
    pub start_prb: u16,
    pub num_prbs: u16,
    pub center_freq_hz: f64,
}

impl SbfdSubband {
    /// Create a new SBFD subband with PRB bounds.
    pub fn new(
        subband_id: u8,
        subband_type: SbfdSubbandType,
        start_prb: u16,
        num_prbs: u16,
        carrier_center_freq_hz: f64,
        scs_hz: f64,
        carrier_total_prbs: u16,
    ) -> Self {
        let prb_bw_hz = (SUBCARRIERS_PER_PRB as f64) * scs_hz;
        let carrier_bw_hz = (carrier_total_prbs as f64) * prb_bw_hz;
        let carrier_start_freq = carrier_center_freq_hz - (carrier_bw_hz / 2.0);

        let subband_start_freq = carrier_start_freq + ((start_prb as f64) * prb_bw_hz);
        let subband_bw = (num_prbs as f64) * prb_bw_hz;
        let center_freq_hz = subband_start_freq + (subband_bw / 2.0);

        Self {
            subband_id,
            subband_type,
            start_prb,
            num_prbs,
            center_freq_hz,
        }
    }

    /// Upper PRB index (inclusive).
    pub fn end_prb(&self) -> u16 {
        self.start_prb + self.num_prbs.saturating_sub(1)
    }

    /// Check if this subband overlaps with another subband.
    pub fn overlaps_with(&self, other: &SbfdSubband) -> bool {
        self.start_prb <= other.end_prb() && other.start_prb <= self.end_prb()
    }
}

/// Slot duplex mode classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SbfdSlotType {
    /// Standard legacy Downlink-only slot.
    LegacyDl,
    /// Standard legacy Uplink-only slot.
    LegacyUl,
    /// Standard flexible slot.
    Flexible,
    /// SBFD slot with simultaneous DL and UL on partitioned non-overlapping subbands.
    SbfdFullDuplex,
}

/// SBFD Slot configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct SbfdSlotConfig {
    pub slot_number: u32,
    pub slot_type: SbfdSlotType,
    pub total_carrier_prbs: u16,
    pub subbands: Vec<SbfdSubband>,
    pub min_guard_prbs: u16,
}

impl SbfdSlotConfig {
    /// Create a new SBFD slot configuration and validate integrity.
    pub fn new(
        slot_number: u32,
        slot_type: SbfdSlotType,
        total_carrier_prbs: u16,
        subbands: Vec<SbfdSubband>,
        min_guard_prbs: u16,
    ) -> Result<Self, SbfdError> {
        let config = Self {
            slot_number,
            slot_type,
            total_carrier_prbs,
            subbands,
            min_guard_prbs,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate PRB ranges, no overlapping active subbands, and guard band sufficiency.
    pub fn validate(&self) -> Result<(), SbfdError> {
        for (i, s1) in self.subbands.iter().enumerate() {
            if s1.end_prb() >= self.total_carrier_prbs {
                return Err(SbfdError::CarrierPrbOutOfBounds {
                    requested: s1.end_prb(),
                    max_prbs: self.total_carrier_prbs,
                });
            }

            for s2 in self.subbands.iter().skip(i + 1) {
                if s1.overlaps_with(s2) {
                    return Err(SbfdError::PrbOverlap {
                        subband_a: s1.subband_id,
                        subband_b: s2.subband_id,
                        prb: s1.start_prb.max(s2.start_prb),
                    });
                }
            }
        }

        // For SBFD full duplex slots, verify that a guard band separates DL and UL subbands
        if self.slot_type == SbfdSlotType::SbfdFullDuplex {
            let dl_subbands: Vec<&SbfdSubband> = self
                .subbands
                .iter()
                .filter(|s| s.subband_type == SbfdSubbandType::Downlink)
                .collect();
            let ul_subbands: Vec<&SbfdSubband> = self
                .subbands
                .iter()
                .filter(|s| s.subband_type == SbfdSubbandType::Uplink)
                .collect();

            for dl in &dl_subbands {
                for ul in &ul_subbands {
                    let separation = if dl.start_prb > ul.end_prb() {
                        dl.start_prb - ul.end_prb() - 1
                    } else if ul.start_prb > dl.end_prb() {
                        ul.start_prb - dl.end_prb() - 1
                    } else {
                        0
                    };

                    if separation < self.min_guard_prbs {
                        return Err(SbfdError::InsufficientGuardBand {
                            actual_prbs: separation,
                            required_prbs: self.min_guard_prbs,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Get total bandwidth in PRBs dedicated to DL in this slot.
    pub fn total_dl_prbs(&self) -> u16 {
        match self.slot_type {
            SbfdSlotType::LegacyDl => self.total_carrier_prbs,
            SbfdSlotType::LegacyUl => 0,
            SbfdSlotType::Flexible => 0,
            SbfdSlotType::SbfdFullDuplex => self
                .subbands
                .iter()
                .filter(|s| s.subband_type == SbfdSubbandType::Downlink)
                .map(|s| s.num_prbs)
                .sum(),
        }
    }

    /// Get total bandwidth in PRBs dedicated to UL in this slot.
    pub fn total_ul_prbs(&self) -> u16 {
        match self.slot_type {
            SbfdSlotType::LegacyUl => self.total_carrier_prbs,
            SbfdSlotType::LegacyDl => 0,
            SbfdSlotType::Flexible => 0,
            SbfdSlotType::SbfdFullDuplex => self
                .subbands
                .iter()
                .filter(|s| s.subband_type == SbfdSubbandType::Uplink)
                .map(|s| s.num_prbs)
                .sum(),
        }
    }
}

// ---------------------------------------------------------------------------
// Self-Interference Cancellation (SIC) Model
// ---------------------------------------------------------------------------

/// 3-Stage Self-Interference Cancellation (SIC) capability at gNodeB.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfInterferenceCancellationModel {
    /// Stage 1: Spatial & antenna isolation in dB (e.g. cross-polarization and physical separation).
    pub spatial_isolation_db: f64,
    /// Stage 2: Analog RF domain cancellation in dB (e.g. analog tapping & adaptive vector attenuator).
    pub analog_cancellation_db: f64,
    /// Stage 3: Digital baseband cancellation in dB (e.g. nonlinear Volterra/polynomial filter).
    pub digital_cancellation_db: f64,
}

impl Default for SelfInterferenceCancellationModel {
    fn default() -> Self {
        Self {
            spatial_isolation_db: 45.0,    // 45 dB spatial isolation
            analog_cancellation_db: 35.0,  // 35 dB analog cancellation
            digital_cancellation_db: 35.0, // 35 dB digital cancellation
        }
    }
}

impl SelfInterferenceCancellationModel {
    /// Create a customized SIC model with specified stage values.
    pub fn new(spatial_db: f64, analog_db: f64, digital_db: f64) -> Self {
        Self {
            spatial_isolation_db: spatial_db,
            analog_cancellation_db: analog_db,
            digital_cancellation_db: digital_db,
        }
    }

    /// Total cumulative self-interference cancellation in dB.
    pub fn total_cancellation_db(&self) -> f64 {
        self.spatial_isolation_db + self.analog_cancellation_db + self.digital_cancellation_db
    }

    /// Calculate Residual Self-Interference (RSI) power in dBm.
    ///
    /// $P_{RSI}\ (\text{dBm}) = P_{tx}\ (\text{dBm}) - \text{Total SIC}\ (\text{dB})$
    pub fn calculate_residual_self_interference_dbm(&self, tx_power_dbm: f64) -> f64 {
        tx_power_dbm - self.total_cancellation_db()
    }

    /// Calculate thermal noise power for a given bandwidth in PRBs at 30 kHz SCS.
    ///
    /// $N_0\ (\text{dBm}) = -174 + 10 \log_{10}(BW_{Hz}) + NF$
    pub fn calculate_thermal_noise_dbm(num_prbs: u16, noise_figure_db: f64) -> f64 {
        let bw_hz = (num_prbs as f64) * PRB_BANDWIDTH_30KHZ_HZ;
        THERMAL_NOISE_DENSITY_DBM_HZ + 10.0 * bw_hz.log10() + noise_figure_db
    }

    /// Calculate receiver noise floor elevation ($\Delta N_0$) in dB due to residual self-interference.
    ///
    /// $\Delta N_0\ (\text{dB}) = 10 \log_{10}\left(1 + 10^{(P_{RSI} - N_0)/10}\right)$
    pub fn calculate_noise_floor_rise_db(
        &self,
        tx_power_dbm: f64,
        num_prbs: u16,
        noise_figure_db: f64,
    ) -> f64 {
        let rsi_dbm = self.calculate_residual_self_interference_dbm(tx_power_dbm);
        let n0_dbm = Self::calculate_thermal_noise_dbm(num_prbs, noise_figure_db);

        let linear_ratio = 10.0f64.powf((rsi_dbm - n0_dbm) / 10.0);
        10.0 * (1.0 + linear_ratio).log10()
    }
}

// ---------------------------------------------------------------------------
// Cross-Link Interference (CLI) Model
// ---------------------------------------------------------------------------

/// Cross-Link Interference (CLI) between gNBs and UEs in SBFD environments.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossLinkInterferenceModel {
    pub carrier_freq_ghz: f64,
}

impl Default for CrossLinkInterferenceModel {
    fn default() -> Self {
        Self {
            carrier_freq_ghz: 3.5, // 3.5 GHz Band n78
        }
    }
}

impl CrossLinkInterferenceModel {
    pub fn new(carrier_freq_ghz: f64) -> Self {
        Self { carrier_freq_ghz }
    }

    /// Calculate gNodeB-to-gNodeB Line-of-Sight (LoS) pathloss in dB per 3GPP TR 38.901 UMi.
    ///
    /// $PL_{gNB-gNB} = 32.4 + 20 \log_{10}(f_{GHz}) + 20 \log_{10}(d_{m})$
    pub fn gnb_to_gnb_pathloss_db(&self, distance_m: f64) -> f64 {
        let dist = distance_m.max(1.0);
        32.4 + 20.0 * self.carrier_freq_ghz.log10() + 20.0 * dist.log10()
    }

    /// Calculate UE-to-UE Non-Line-of-Sight (NLoS) pathloss in dB per 3GPP TR 38.901 UMi.
    ///
    /// $PL_{UE-UE} = 35.3 + 22.4 \log_{10}(d_{m}) + 21.3 \log_{10}(f_{GHz})$
    pub fn ue_to_ue_pathloss_db(&self, distance_m: f64) -> f64 {
        let dist = distance_m.max(1.0);
        35.3 + 22.4 * dist.log10() + 21.3 * self.carrier_freq_ghz.log10()
    }

    /// Calculate received gNB-to-gNB CLI power in dBm.
    pub fn calculate_gnb_cli_power_dbm(&self, aggressor_tx_power_dbm: f64, distance_m: f64) -> f64 {
        let pl = self.gnb_to_gnb_pathloss_db(distance_m);
        aggressor_tx_power_dbm - pl
    }

    /// Calculate received UE-to-UE CLI power in dBm.
    pub fn calculate_ue_cli_power_dbm(
        &self,
        aggressor_ue_tx_power_dbm: f64,
        distance_m: f64,
    ) -> f64 {
        let pl = self.ue_to_ue_pathloss_db(distance_m);
        aggressor_ue_tx_power_dbm - pl
    }
}

// ---------------------------------------------------------------------------
// Link Adaptation & MCS Mapping
// ---------------------------------------------------------------------------

/// 3GPP TS 38.214 Modulation & Coding Scheme (MCS) configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct McsEntry {
    pub mcs_index: u8,
    pub modulation_order: u8, // 2: QPSK, 4: 16QAM, 6: 64QAM, 8: 256QAM
    pub target_code_rate_x1024: u16,
    pub spectral_efficiency_bits_s_hz: f64,
    pub min_sinr_db: f64,
}

/// SBFD Link Adapter.
pub struct SbfdLinkAdapter;

impl SbfdLinkAdapter {
    /// Select highest supported MCS for a given effective SINR.
    pub fn select_mcs(effective_sinr_db: f64) -> Option<McsEntry> {
        let table = Self::get_mcs_table();
        table
            .into_iter()
            .rev()
            .find(|entry| effective_sinr_db >= entry.min_sinr_db)
    }

    /// Standard subset of TS 38.214 Table 5.1.3.1-1.
    pub fn get_mcs_table() -> Vec<McsEntry> {
        vec![
            McsEntry {
                mcs_index: 0,
                modulation_order: 2,
                target_code_rate_x1024: 120,
                spectral_efficiency_bits_s_hz: 0.2344,
                min_sinr_db: -6.0,
            },
            McsEntry {
                mcs_index: 2,
                modulation_order: 2,
                target_code_rate_x1024: 193,
                spectral_efficiency_bits_s_hz: 0.3770,
                min_sinr_db: -4.0,
            },
            McsEntry {
                mcs_index: 4,
                modulation_order: 2,
                target_code_rate_x1024: 308,
                spectral_efficiency_bits_s_hz: 0.6016,
                min_sinr_db: -2.0,
            },
            McsEntry {
                mcs_index: 6,
                modulation_order: 2,
                target_code_rate_x1024: 449,
                spectral_efficiency_bits_s_hz: 0.8770,
                min_sinr_db: 0.5,
            },
            McsEntry {
                mcs_index: 9,
                modulation_order: 2,
                target_code_rate_x1024: 679,
                spectral_efficiency_bits_s_hz: 1.3262,
                min_sinr_db: 3.5,
            },
            McsEntry {
                mcs_index: 11,
                modulation_order: 4,
                target_code_rate_x1024: 378,
                spectral_efficiency_bits_s_hz: 1.4766,
                min_sinr_db: 5.5,
            },
            McsEntry {
                mcs_index: 14,
                modulation_order: 4,
                target_code_rate_x1024: 553,
                spectral_efficiency_bits_s_hz: 2.1602,
                min_sinr_db: 9.0,
            },
            McsEntry {
                mcs_index: 16,
                modulation_order: 4,
                target_code_rate_x1024: 658,
                spectral_efficiency_bits_s_hz: 2.5703,
                min_sinr_db: 11.5,
            },
            McsEntry {
                mcs_index: 19,
                modulation_order: 6,
                target_code_rate_x1024: 567,
                spectral_efficiency_bits_s_hz: 3.3223,
                min_sinr_db: 15.0,
            },
            McsEntry {
                mcs_index: 22,
                modulation_order: 6,
                target_code_rate_x1024: 719,
                spectral_efficiency_bits_s_hz: 4.2129,
                min_sinr_db: 18.5,
            },
            McsEntry {
                mcs_index: 25,
                modulation_order: 6,
                target_code_rate_x1024: 873,
                spectral_efficiency_bits_s_hz: 5.1152,
                min_sinr_db: 22.0,
            },
            McsEntry {
                mcs_index: 27,
                modulation_order: 8,
                target_code_rate_x1024: 841,
                spectral_efficiency_bits_s_hz: 6.5703,
                min_sinr_db: 26.5,
            },
        ]
    }
}

// ---------------------------------------------------------------------------
// Ultra-Low Latency SBFD Scheduler
// ---------------------------------------------------------------------------

/// Result of an Uplink transmission grant decision.
#[derive(Debug, Clone, PartialEq)]
pub struct UlGrantDecision {
    pub slot_number: u32,
    pub is_sbfd: bool,
    pub allocated_prbs: u16,
    pub effective_sinr_db: f64,
    pub mcs: McsEntry,
    pub transport_block_bytes: usize,
    pub wait_latency_ms: f64,
}

/// Dynamic SBFD Protocol and Scheduler Engine.
pub struct SbfdEngine {
    pub total_carrier_prbs: u16,
    pub gnb_tx_power_dbm: f64,
    pub noise_figure_db: f64,
    pub sic_model: SelfInterferenceCancellationModel,
    pub cli_model: CrossLinkInterferenceModel,
    pub slot_patterns: Vec<SbfdSlotConfig>,
    pub metrics: SbfdMetrics,
}

impl SbfdEngine {
    /// Create a new SBFD Engine.
    pub fn new(
        total_carrier_prbs: u16,
        gnb_tx_power_dbm: f64,
        noise_figure_db: f64,
        sic_model: SelfInterferenceCancellationModel,
        cli_model: CrossLinkInterferenceModel,
    ) -> Self {
        Self {
            total_carrier_prbs,
            gnb_tx_power_dbm,
            noise_figure_db,
            sic_model,
            cli_model,
            slot_patterns: Vec::new(),
            metrics: SbfdMetrics::default(),
        }
    }

    /// Register a slot configuration pattern.
    pub fn add_slot_config(&mut self, config: SbfdSlotConfig) {
        self.slot_patterns.push(config);
    }

    /// Schedule an urgent Uplink transmission for a UE at a given arrival slot.
    ///
    /// - `arrival_slot`: The slot index when the urgent UL packet arrives in the UE buffer.
    /// - `ue_rx_power_at_gnb_dbm`: Received UE signal power at gNodeB antenna in dBm.
    /// - `cli_interference_dbm`: External Cross-Link Interference from surrounding nodes in dBm.
    pub fn schedule_urgent_ul(
        &mut self,
        arrival_slot: u32,
        ue_rx_power_at_gnb_dbm: f64,
        cli_interference_dbm: Option<f64>,
    ) -> Result<UlGrantDecision, SbfdError> {
        self.metrics.total_slots_processed += 1;

        if self.slot_patterns.is_empty() {
            return Err(SbfdError::NoSlotConfiguration);
        }

        // Find current slot pattern
        let pattern_len = self.slot_patterns.len();
        let slot_idx = (arrival_slot as usize) % pattern_len;
        let current_slot = self.slot_patterns[slot_idx].clone();

        // 1. If current slot is SBFD, allocate immediately with 0 wait latency!
        if current_slot.slot_type == SbfdSlotType::SbfdFullDuplex {
            let ul_prbs = current_slot.total_ul_prbs();
            if ul_prbs == 0 {
                return Err(SbfdError::NoUplinkSubbandAvailable);
            }

            // Calculate Effective SINR considering thermal noise + RSI + CLI
            let n0_linear = 10.0f64.powf(
                SelfInterferenceCancellationModel::calculate_thermal_noise_dbm(
                    ul_prbs,
                    self.noise_figure_db,
                ) / 10.0,
            );

            let rsi_dbm = self
                .sic_model
                .calculate_residual_self_interference_dbm(self.gnb_tx_power_dbm);

            if rsi_dbm > MAX_TOLERABLE_RSI_DBM {
                return Err(SbfdError::SicFailure {
                    tx_power_dbm: self.gnb_tx_power_dbm,
                    rsi_dbm,
                    max_tolerable_dbm: MAX_TOLERABLE_RSI_DBM,
                });
            }

            let rsi_linear = 10.0f64.powf(rsi_dbm / 10.0);
            let cli_linear = cli_interference_dbm
                .map(|p| 10.0f64.powf(p / 10.0))
                .unwrap_or(0.0);

            let total_noise_plus_interference_linear = n0_linear + rsi_linear + cli_linear;
            let signal_linear = 10.0f64.powf(ue_rx_power_at_gnb_dbm / 10.0);

            let sinr_linear = signal_linear / total_noise_plus_interference_linear.max(1e-18);
            let effective_sinr_db = 10.0 * sinr_linear.log10();

            let mcs = SbfdLinkAdapter::select_mcs(effective_sinr_db).ok_or(
                SbfdError::InsufficientSinr {
                    sinr_db: effective_sinr_db,
                    min_required_db: -6.0,
                },
            )?;

            // Approximate TB size = num_prbs * 12 * 14 * spectral_efficiency / 8
            let tb_bytes =
                ((ul_prbs as f64) * 12.0 * 14.0 * mcs.spectral_efficiency_bits_s_hz / 8.0) as usize;

            self.metrics.sbfd_slots_count += 1;
            self.metrics.total_ul_bytes_sbfd += tb_bytes as u64;

            return Ok(UlGrantDecision {
                slot_number: arrival_slot,
                is_sbfd: true,
                allocated_prbs: ul_prbs,
                effective_sinr_db,
                mcs,
                transport_block_bytes: tb_bytes,
                wait_latency_ms: 0.0, // Zero slot wait in SBFD!
            });
        }

        // 2. If current slot is already LegacyUl, schedule immediately
        if current_slot.slot_type == SbfdSlotType::LegacyUl {
            let ul_prbs = self.total_carrier_prbs;
            let n0_linear = 10.0f64.powf(
                SelfInterferenceCancellationModel::calculate_thermal_noise_dbm(
                    ul_prbs,
                    self.noise_figure_db,
                ) / 10.0,
            );
            let signal_linear = 10.0f64.powf(ue_rx_power_at_gnb_dbm / 10.0);
            let sinr_linear = signal_linear / n0_linear;
            let effective_sinr_db = 10.0 * sinr_linear.log10();

            let mcs = SbfdLinkAdapter::select_mcs(effective_sinr_db).ok_or(
                SbfdError::InsufficientSinr {
                    sinr_db: effective_sinr_db,
                    min_required_db: -6.0,
                },
            )?;

            let tb_bytes =
                ((ul_prbs as f64) * 12.0 * 14.0 * mcs.spectral_efficiency_bits_s_hz / 8.0) as usize;

            self.metrics.legacy_ul_slots_count += 1;

            return Ok(UlGrantDecision {
                slot_number: arrival_slot,
                is_sbfd: false,
                allocated_prbs: ul_prbs,
                effective_sinr_db,
                mcs,
                transport_block_bytes: tb_bytes,
                wait_latency_ms: 0.0,
            });
        }

        // 3. Otherwise, current slot is LegacyDl: must wait for the next UL or SBFD slot
        let mut wait_slots = 0;
        let mut target_slot_config = None;

        for offset in 1..=pattern_len {
            let next_idx = (slot_idx + offset) % pattern_len;
            let next_slot = &self.slot_patterns[next_idx];
            if next_slot.slot_type == SbfdSlotType::LegacyUl
                || next_slot.slot_type == SbfdSlotType::SbfdFullDuplex
            {
                wait_slots = offset as u32;
                target_slot_config = Some(next_slot.clone());
                break;
            }
        }

        let target_slot = target_slot_config.ok_or(SbfdError::NoUplinkSubbandAvailable)?;
        let slot_duration_ms = 0.5; // 0.5 ms per slot at 30 kHz SCS
        let wait_latency_ms = (wait_slots as f64) * slot_duration_ms;

        let ul_prbs = target_slot.total_ul_prbs();
        let n0_linear = 10.0f64.powf(
            SelfInterferenceCancellationModel::calculate_thermal_noise_dbm(
                ul_prbs,
                self.noise_figure_db,
            ) / 10.0,
        );
        let signal_linear = 10.0f64.powf(ue_rx_power_at_gnb_dbm / 10.0);
        let effective_sinr_db = 10.0 * (signal_linear / n0_linear).log10();
        let mcs =
            SbfdLinkAdapter::select_mcs(effective_sinr_db).ok_or(SbfdError::InsufficientSinr {
                sinr_db: effective_sinr_db,
                min_required_db: -6.0,
            })?;
        let tb_bytes =
            ((ul_prbs as f64) * 12.0 * 14.0 * mcs.spectral_efficiency_bits_s_hz / 8.0) as usize;

        self.metrics.legacy_dl_slots_count += 1;

        Ok(UlGrantDecision {
            slot_number: arrival_slot + wait_slots,
            is_sbfd: target_slot.slot_type == SbfdSlotType::SbfdFullDuplex,
            allocated_prbs: ul_prbs,
            effective_sinr_db,
            mcs,
            transport_block_bytes: tb_bytes,
            wait_latency_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Telemetry & Metrics
// ---------------------------------------------------------------------------

/// Performance and telemetry metrics for SBFD operations.
#[derive(Debug, Clone, Default)]
pub struct SbfdMetrics {
    pub total_slots_processed: u64,
    pub sbfd_slots_count: u64,
    pub legacy_dl_slots_count: u64,
    pub legacy_ul_slots_count: u64,
    pub total_ul_bytes_sbfd: u64,
    pub total_dl_bytes_sbfd: u64,
}

impl SbfdMetrics {
    /// Calculate ratio of SBFD slots to total slots processed in percentage.
    pub fn sbfd_slot_ratio(&self) -> f64 {
        if self.total_slots_processed == 0 {
            return 0.0;
        }
        (self.sbfd_slots_count as f64 / self.total_slots_processed as f64) * 100.0
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors encountered in SBFD configuration and operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SbfdError {
    CarrierPrbOutOfBounds {
        requested: u16,
        max_prbs: u16,
    },
    PrbOverlap {
        subband_a: u8,
        subband_b: u8,
        prb: u16,
    },
    InsufficientGuardBand {
        actual_prbs: u16,
        required_prbs: u16,
    },
    SicFailure {
        tx_power_dbm: f64,
        rsi_dbm: f64,
        max_tolerable_dbm: f64,
    },
    InsufficientSinr {
        sinr_db: f64,
        min_required_db: f64,
    },
    NoSlotConfiguration,
    NoUplinkSubbandAvailable,
}

impl fmt::Display for SbfdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SbfdError::CarrierPrbOutOfBounds {
                requested,
                max_prbs,
            } => {
                write!(
                    f,
                    "SBFD PRB {} exceeds carrier bandwidth {}",
                    requested, max_prbs
                )
            }
            SbfdError::PrbOverlap {
                subband_a,
                subband_b,
                prb,
            } => {
                write!(
                    f,
                    "SBFD subband {} and {} overlap at PRB {}",
                    subband_a, subband_b, prb
                )
            }
            SbfdError::InsufficientGuardBand {
                actual_prbs,
                required_prbs,
            } => {
                write!(
                    f,
                    "Insufficient SBFD guard band: {} PRBs (minimum required: {})",
                    actual_prbs, required_prbs
                )
            }
            SbfdError::SicFailure {
                tx_power_dbm,
                rsi_dbm,
                max_tolerable_dbm,
            } => {
                write!(
                    f,
                    "SIC breakdown: Tx power {:.1} dBm yields RSI {:.1} dBm exceeding limit {:.1} dBm",
                    tx_power_dbm, rsi_dbm, max_tolerable_dbm
                )
            }
            SbfdError::InsufficientSinr {
                sinr_db,
                min_required_db,
            } => {
                write!(
                    f,
                    "Effective SINR {:.2} dB below minimum decodable threshold {:.2} dB",
                    sinr_db, min_required_db
                )
            }
            SbfdError::NoSlotConfiguration => {
                write!(f, "No SBFD slot configuration pattern registered")
            }
            SbfdError::NoUplinkSubbandAvailable => {
                write!(f, "No uplink subband available in SBFD slot")
            }
        }
    }
}

impl std::error::Error for SbfdError {}
