//! 3GPP Release 18 (5G-Advanced) Network Energy Savings (NES) Protocol Engine.
//!
//! Conforms to:
//! - 3GPP TR 38.864: Study on Network Energy Savings for NR (Release 18).
//! - 3GPP TS 38.300: NR Overall description; Stage-2 (Release 18 NES features).
//! - 3GPP TS 38.213 §12: Physical layer procedures for control (Air interface cell DTX/DRX).
//! - 3GPP TS 38.331: Radio Resource Control (RRC) SSB periodicity and beam adaptation.
//! - ETSI ES 203 228 / 3GPP TS 28.554: Energy Efficiency (EE) KPI specifications.
//!
//! Features:
//! 1. Base Station Multi-Component Power Consumption Model ($P_{gNB} = P_0 + \Delta_{slope} P_{tx} \rho$).
//! 2. 4-Level Sleep State Machine:
//!    - Level 1: Micro-sleep (symbol-level PA/LNA bias gating, sub-microsecond transition).
//!    - Level 2: Slot-level sleep (subframe/slot muting, fast transition).
//!    - Level 3: Light Dormancy (digital baseband & LO muting, millisecond wake-up).
//!    - Level 4: Deep Dormancy (carrier RF shutdown with wake-up paging).
//! 3. Dynamic SSB Adaptation:
//!    - Periodicity scaling ($T_{SSB} \in \{20, 40, 80, 160\text{ ms}\}$).
//!    - Spatial beam skipping/muting (bitmask of active SSB beams).
//!    - SSB-less Secondary Cell (SCell) operation in Carrier Aggregation.
//! 4. Spatial Domain Massive MIMO Antenna Branch Muting ($64T \to 32T \to 16T \to 8T \to 4T$).
//! 5. Cell DTX/DRX Burst Scheduling with latency budget enforcement and hysteresis guards.
//! 6. Energy Efficiency KPI Tracker (Bits/Joule and Energy Saving Ratio).
//!
//! Pure standard Rust with zero external dependencies.

use std::fmt;

// ---------------------------------------------------------------------------
// Constants & Standard Profiles
// ---------------------------------------------------------------------------

/// Standard symbols per slot in NR normal cyclic prefix.
pub const NR_SYMBOLS_PER_SLOT: usize = 14;
pub const NES_SYMBOLS_PER_SLOT: usize = NR_SYMBOLS_PER_SLOT;

/// Default maximum number of SSB candidate beams in FR1 (3 - 6 GHz).
pub const DEFAULT_MAX_SSB_BEAMS_FR1: usize = 8;
pub const NES_DEFAULT_MAX_SSB_BEAMS_FR1: usize = DEFAULT_MAX_SSB_BEAMS_FR1;

/// Default maximum number of SSB candidate beams in FR2 (mmWave).
pub const DEFAULT_MAX_SSB_BEAMS_FR2: usize = 64;
pub const NES_DEFAULT_MAX_SSB_BEAMS_FR2: usize = DEFAULT_MAX_SSB_BEAMS_FR2;

/// Default maximum antenna elements for Macro Massive MIMO.
pub const DEFAULT_MAX_MIMO_ANTENNAS: usize = 64;
pub const NES_DEFAULT_MAX_MIMO_ANTENNAS: usize = DEFAULT_MAX_MIMO_ANTENNAS;

// ---------------------------------------------------------------------------
// Error Definitions
// ---------------------------------------------------------------------------

/// Errors encountered in NES processing and state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NesError {
    /// Requested sleep state violates the active latency budget of queued traffic.
    LatencyBudgetViolation {
        requested_sleep_ms: u32,
        max_allowed_latency_ms: u32,
    },
    /// Antenna configuration is invalid (must be power of 2 between 4 and 64).
    InvalidAntennaCount(usize),
    /// Invalid SSB periodicity (must be 20, 40, 80, or 160 ms).
    InvalidSsbPeriodicity(u32),
    /// Invalid load factor (must be between 0.0 and 1.0).
    InvalidLoadFactor(String),
    /// State transition rejected because minimum dwell time has not elapsed.
    DwellTimeNotElapsed {
        current_dwell_slots: u32,
        required_dwell_slots: u32,
    },
    /// General configuration error.
    InvalidConfiguration(String),
}

impl fmt::Display for NesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NesError::LatencyBudgetViolation {
                requested_sleep_ms,
                max_allowed_latency_ms,
            } => {
                write!(
                    f,
                    "NES sleep transition rejected: wake-up latency {} ms exceeds max allowed {} ms",
                    requested_sleep_ms, max_allowed_latency_ms
                )
            }
            NesError::InvalidAntennaCount(count) => {
                write!(
                    f,
                    "Invalid active antenna count {}: must be one of [4, 8, 16, 32, 64]",
                    count
                )
            }
            NesError::InvalidSsbPeriodicity(p) => {
                write!(
                    f,
                    "Invalid SSB periodicity {} ms: standard values are 20, 40, 80, or 160 ms",
                    p
                )
            }
            NesError::InvalidLoadFactor(msg) => {
                write!(f, "Invalid PRB traffic load factor: {}", msg)
            }
            NesError::DwellTimeNotElapsed {
                current_dwell_slots,
                required_dwell_slots,
            } => {
                write!(
                    f,
                    "NES state transition flapping guard: current dwell {} slots < required {} slots",
                    current_dwell_slots, required_dwell_slots
                )
            }
            NesError::InvalidConfiguration(msg) => {
                write!(f, "NES configuration error: {}", msg)
            }
        }
    }
}

impl std::error::Error for NesError {}

// ---------------------------------------------------------------------------
// 1. Multi-Level Sleep States (3GPP TR 38.864 §7)
// ---------------------------------------------------------------------------

/// 3GPP Rel-18 4-level base station energy-saving sleep state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NesSleepLevel {
    /// Fully active transmission / reception (0% sleep).
    Active = 0,
    /// Level 1: Symbol-level Micro-sleep (PA bias gated off during unallocated symbols).
    Level1MicroSleep = 1,
    /// Level 2: Slot/Subframe-level sleep (PA shutdown across unallocated slots).
    Level2SlotSleep = 2,
    /// Level 3: Light Dormancy (digital baseband, LO, and RF front-end gated off).
    Level3LightDormancy = 3,
    /// Level 4: Deep Dormancy (carrier shut down; wake-up receiver or paging triggered).
    Level4DeepDormancy = 4,
}

impl NesSleepLevel {
    /// Approximate wake-up latency in milliseconds.
    pub fn wakeup_latency_ms(&self) -> f64 {
        match self {
            NesSleepLevel::Active => 0.0,
            NesSleepLevel::Level1MicroSleep => 0.005, // 5 microseconds
            NesSleepLevel::Level2SlotSleep => 0.080,  // 80 microseconds
            NesSleepLevel::Level3LightDormancy => 2.5, // 2.5 ms
            NesSleepLevel::Level4DeepDormancy => 100.0, // 100 ms
        }
    }

    /// Sleep power reduction factor $\delta_{sleep}$ relative to static power $P_0$.
    pub fn sleep_power_factor(&self) -> f64 {
        match self {
            NesSleepLevel::Active => 1.0,
            NesSleepLevel::Level1MicroSleep => 0.50, // 50% power reduction on static/PA
            NesSleepLevel::Level2SlotSleep => 0.35,  // 65% power reduction
            NesSleepLevel::Level3LightDormancy => 0.15, // 85% power reduction
            NesSleepLevel::Level4DeepDormancy => 0.05, // 95% power reduction
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Base Station Power Consumption Model (3GPP TR 38.864 §6)
// ---------------------------------------------------------------------------

/// Base station power consumption profile per TR 38.864 Section 6.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseStationPowerModel {
    /// Static baseband power consumption in Watts (DSP, FPGA, backhaul).
    pub p_baseband_watts: f64,
    /// Static RF hardware baseline power per antenna branch in Watts.
    pub p_rf_static_per_antenna_watts: f64,
    /// Maximum radiated RF transmit power per antenna branch in Watts (e.g. 0.625W for 64T = 40W total).
    pub p_tx_max_per_antenna_watts: f64,
    /// Power amplifier slope factor $\Delta_{slope}$ (reflecting PA efficiency & losses, typically 3.0..4.2).
    pub delta_slope: f64,
    /// Maximum configured antenna branches (e.g. 64 for 64T64R).
    pub max_antennas: usize,
}

impl Default for BaseStationPowerModel {
    fn default() -> Self {
        Self {
            p_baseband_watts: 180.0,
            p_rf_static_per_antenna_watts: 3.5,
            p_tx_max_per_antenna_watts: 0.625, // 64 * 0.625W = 40W RF
            delta_slope: 3.8,
            max_antennas: DEFAULT_MAX_MIMO_ANTENNAS,
        }
    }
}

impl BaseStationPowerModel {
    /// Calculate static power consumption $P_0$ for a given active antenna count.
    pub fn calculate_static_power(&self, active_antennas: usize) -> f64 {
        let active = active_antennas.clamp(1, self.max_antennas);
        self.p_baseband_watts + (self.p_rf_static_per_antenna_watts * active as f64)
    }

    /// Calculate instantaneous power consumption in Watts for a specific sleep level,
    /// active antenna count, and PRB traffic load $\rho \in [0.0, 1.0]$.
    pub fn calculate_instantaneous_power(
        &self,
        level: NesSleepLevel,
        active_antennas: usize,
        prb_load: f64,
    ) -> f64 {
        let load = prb_load.clamp(0.0, 1.0);
        let active = active_antennas.clamp(1, self.max_antennas);
        let p_0 = self.calculate_static_power(active);

        match level {
            NesSleepLevel::Active => {
                let total_rf_tx_max = self.p_tx_max_per_antenna_watts * active as f64;
                p_0 + (self.delta_slope * total_rf_tx_max * load)
            }
            other_sleep => {
                let factor = other_sleep.sleep_power_factor();
                p_0 * factor
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Dynamic SSB Adaptation Configuration (3GPP TS 38.331 & TS 38.213)
// ---------------------------------------------------------------------------

/// Configuration for Dynamic Synchronization Signal Block (SSB) adaptation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsbAdaptationConfig {
    /// SSB transmission periodicity in milliseconds (20, 40, 80, 160 ms).
    pub periodicity_ms: u32,
    /// Total configured candidate SSB beams (4 or 8 in FR1, up to 64 in FR2).
    pub total_candidate_beams: usize,
    /// Bitmask of actively transmitted SSB beams (e.g. 0b0000_0011 means only beams 0 and 1 active).
    pub active_beam_mask: u64,
    /// Whether Secondary Cell (SCell) operates in SSB-less mode.
    pub is_ssb_less_scell: bool,
}

impl Default for SsbAdaptationConfig {
    fn default() -> Self {
        Self {
            periodicity_ms: 20,
            total_candidate_beams: DEFAULT_MAX_SSB_BEAMS_FR1,
            active_beam_mask: 0xFF, // All 8 beams active by default
            is_ssb_less_scell: false,
        }
    }
}

impl SsbAdaptationConfig {
    /// Create new SSB configuration with validation.
    pub fn new(
        periodicity_ms: u32,
        total_candidate_beams: usize,
        active_beam_mask: u64,
        is_ssb_less_scell: bool,
    ) -> Result<Self, NesError> {
        if ![20, 40, 80, 160].contains(&periodicity_ms) {
            return Err(NesError::InvalidSsbPeriodicity(periodicity_ms));
        }
        if total_candidate_beams == 0 || total_candidate_beams > 64 {
            return Err(NesError::InvalidConfiguration(
                "total_candidate_beams must be between 1 and 64".to_string(),
            ));
        }
        Ok(Self {
            periodicity_ms,
            total_candidate_beams,
            active_beam_mask,
            is_ssb_less_scell,
        })
    }

    /// Count of actively transmitted SSB beams.
    pub fn active_beam_count(&self) -> usize {
        if self.is_ssb_less_scell {
            return 0;
        }
        let mut count = 0;
        for i in 0..self.total_candidate_beams {
            if (self.active_beam_mask & (1 << i)) != 0 {
                count += 1;
            }
        }
        count
    }

    /// Calculate the fractional active SSB transmission overhead relative to full 20ms broadcast.
    pub fn calculate_ssb_overhead_fraction(&self) -> f64 {
        if self.is_ssb_less_scell {
            return 0.0;
        }
        let active_beams = self.active_beam_count() as f64;
        let total_beams = self.total_candidate_beams as f64;
        let beam_ratio = active_beams / total_beams;
        let periodicity_ratio = 20.0 / self.periodicity_ms as f64;
        beam_ratio * periodicity_ratio
    }
}

// ---------------------------------------------------------------------------
// 4. Spatial MIMO Antenna Branch Muting (TR 38.864 §8)
// ---------------------------------------------------------------------------

/// Dynamic Massive MIMO antenna muting configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialMimoConfig {
    /// Currently active transmit/receive antenna branches (e.g. 64, 32, 16, 8, 4).
    pub active_antennas: usize,
    /// Maximum configured antenna array size (e.g. 64).
    pub max_antennas: usize,
    /// High PRB load threshold above which antenna branches scale up (e.g. 0.70).
    pub scale_up_load_threshold_pct: u8,
    /// Low PRB load threshold below which antenna branches mute down (e.g. 0.25).
    pub scale_down_load_threshold_pct: u8,
}

impl Default for SpatialMimoConfig {
    fn default() -> Self {
        Self {
            active_antennas: DEFAULT_MAX_MIMO_ANTENNAS,
            max_antennas: DEFAULT_MAX_MIMO_ANTENNAS,
            scale_up_load_threshold_pct: 70,
            scale_down_load_threshold_pct: 25,
        }
    }
}

impl SpatialMimoConfig {
    pub fn new(
        active_antennas: usize,
        max_antennas: usize,
        scale_up_load_threshold_pct: u8,
        scale_down_load_threshold_pct: u8,
    ) -> Result<Self, NesError> {
        if ![4, 8, 16, 32, 64].contains(&active_antennas) {
            return Err(NesError::InvalidAntennaCount(active_antennas));
        }
        Ok(Self {
            active_antennas,
            max_antennas,
            scale_up_load_threshold_pct,
            scale_down_load_threshold_pct,
        })
    }

    /// Evaluates PRB load and adapts active antenna count up or down.
    pub fn adapt_antenna_count(&mut self, prb_load: f64) -> (usize, bool) {
        let load_pct = (prb_load.clamp(0.0, 1.0) * 100.0).round() as u8;
        let old_count = self.active_antennas;

        if load_pct >= self.scale_up_load_threshold_pct && self.active_antennas < self.max_antennas
        {
            // Scale up: double active antennas (4 -> 8 -> 16 -> 32 -> 64)
            self.active_antennas = (self.active_antennas * 2).min(self.max_antennas);
        } else if load_pct <= self.scale_down_load_threshold_pct && self.active_antennas > 4 {
            // Scale down: halve active antennas (64 -> 32 -> 16 -> 8 -> 4)
            self.active_antennas = (self.active_antennas / 2).max(4);
        }

        (self.active_antennas, self.active_antennas != old_count)
    }
}

// ---------------------------------------------------------------------------
// 5. Cell DTX/DRX Burst Pattern & Slot Scheduler (TS 38.213 §12)
// ---------------------------------------------------------------------------

/// Pattern for discontinuous transmission / reception (Cell DTX/DRX) at the gNB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellDtxDrxPattern {
    /// Active on-duration in slots where traffic is scheduled.
    pub on_duration_slots: u32,
    /// Total cycle periodicity in slots (on_duration + sleep).
    pub cycle_periodicity_slots: u32,
    /// Sleep level engaged during inactive slots.
    pub inactive_sleep_level: NesSleepLevel,
}

impl Default for CellDtxDrxPattern {
    fn default() -> Self {
        Self {
            on_duration_slots: 4,
            cycle_periodicity_slots: 20, // 4 slots ON, 16 slots SLEEP
            inactive_sleep_level: NesSleepLevel::Level2SlotSleep,
        }
    }
}

impl CellDtxDrxPattern {
    /// Returns true if slot index is within the active on-duration window.
    pub fn is_active_slot(&self, slot_index: u64) -> bool {
        if self.cycle_periodicity_slots == 0 {
            return true;
        }
        let phase = (slot_index % (self.cycle_periodicity_slots as u64)) as u32;
        phase < self.on_duration_slots
    }
}

// ---------------------------------------------------------------------------
// 6. Energy Efficiency KPI & Telemetry (ETSI ES 203 228)
// ---------------------------------------------------------------------------

/// Operational telemetry counters and Energy Efficiency (EE) metrics.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NesMetrics {
    /// Total cumulative energy consumed in Joules ($W \cdot s$).
    pub energy_consumed_joules: f64,
    /// Theoretical energy that would have been consumed without NES (baseline).
    pub baseline_energy_joules: f64,
    /// Total user plane payload bits successfully delivered.
    pub data_bits_delivered: u64,
    /// Count of micro-sleep symbol opportunities taken.
    pub micro_sleep_symbols_count: u64,
    /// Count of slot-level sleep slots engaged.
    pub slot_sleep_slots_count: u64,
    /// Count of light dormancy periods engaged.
    pub light_dormancy_periods_count: u64,
    /// Count of deep dormancy periods engaged.
    pub deep_dormancy_periods_count: u64,
    /// Number of antenna muting reconfigurations executed.
    pub antenna_reconfigurations_count: u32,
}

impl NesMetrics {
    /// Energy Efficiency (EE) KPI in bits per Joule.
    pub fn energy_efficiency_bits_per_joule(&self) -> f64 {
        if self.energy_consumed_joules <= 1e-9 {
            return 0.0;
        }
        (self.data_bits_delivered as f64) / self.energy_consumed_joules
    }

    /// Energy Saving Ratio (ESR) $\in [0.0, 1.0]$.
    pub fn energy_saving_ratio(&self) -> f64 {
        if self.baseline_energy_joules <= 1e-9 {
            return 0.0;
        }
        let saved = self.baseline_energy_joules - self.energy_consumed_joules;
        (saved / self.baseline_energy_joules).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// 7. Complete 3GPP Rel-18 Network Energy Savings Engine
// ---------------------------------------------------------------------------

/// Complete 3GPP Rel-18 5G-Advanced Network Energy Savings (NES) Engine.
pub struct NrNesEngine {
    /// Power model defining hardware components.
    pub power_model: BaseStationPowerModel,
    /// Dynamic SSB adaptation settings.
    pub ssb_config: SsbAdaptationConfig,
    /// Spatial Massive MIMO antenna muting configuration.
    pub mimo_config: SpatialMimoConfig,
    /// Cell DTX/DRX pattern.
    pub dtx_pattern: CellDtxDrxPattern,
    /// Current operating sleep state.
    pub current_sleep_level: NesSleepLevel,
    /// Minimum dwell time in slots to prevent flapping.
    pub min_dwell_slots: u32,
    /// Elapsed slots in the current sleep state.
    pub current_dwell_slots: u32,
    /// Operational telemetry metrics.
    pub metrics: NesMetrics,
    /// Slot duration in seconds (e.g. 0.001s for 15 kHz, 0.0005s for 30 kHz SCS).
    pub slot_duration_s: f64,
}

impl NrNesEngine {
    /// Create new NES Engine with standard FR1 default parameters.
    pub fn new(slot_duration_s: f64) -> Self {
        Self {
            power_model: BaseStationPowerModel::default(),
            ssb_config: SsbAdaptationConfig::default(),
            mimo_config: SpatialMimoConfig::default(),
            dtx_pattern: CellDtxDrxPattern::default(),
            current_sleep_level: NesSleepLevel::Active,
            min_dwell_slots: 10,
            current_dwell_slots: 10,
            metrics: NesMetrics::default(),
            slot_duration_s,
        }
    }

    /// Request a transition to a target sleep state while enforcing latency budgets and dwell limits.
    pub fn request_state_transition(
        &mut self,
        target_level: NesSleepLevel,
        max_allowed_latency_ms: u32,
    ) -> Result<NesSleepLevel, NesError> {
        if target_level == self.current_sleep_level {
            return Ok(self.current_sleep_level);
        }

        // 1. Enforce anti-flapping minimum dwell time
        if self.current_dwell_slots < self.min_dwell_slots {
            return Err(NesError::DwellTimeNotElapsed {
                current_dwell_slots: self.current_dwell_slots,
                required_dwell_slots: self.min_dwell_slots,
            });
        }

        // 2. Enforce QoS latency budget
        let target_wakeup_ms = target_level.wakeup_latency_ms();
        if target_wakeup_ms > (max_allowed_latency_ms as f64) {
            return Err(NesError::LatencyBudgetViolation {
                requested_sleep_ms: target_wakeup_ms.ceil() as u32,
                max_allowed_latency_ms,
            });
        }

        // Record metrics for entry into sleep modes
        match target_level {
            NesSleepLevel::Level3LightDormancy => self.metrics.light_dormancy_periods_count += 1,
            NesSleepLevel::Level4DeepDormancy => self.metrics.deep_dormancy_periods_count += 1,
            _ => {}
        }

        self.current_sleep_level = target_level;
        self.current_dwell_slots = 0;
        Ok(self.current_sleep_level)
    }

    /// Process a slot scheduling tick: evaluates DTX/DRX pattern, PRB load, active symbols,
    /// performs micro-sleep in idle symbols, and updates energy accounting.
    pub fn tick_slot(
        &mut self,
        slot_index: u64,
        allocated_prbs: usize,
        total_carrier_prbs: usize,
        allocated_symbol_mask: u16, // 14-bit mask of symbols having scheduled data
        delivered_bits: u64,
    ) -> (NesSleepLevel, f64) {
        self.current_dwell_slots += 1;
        let prb_load = if total_carrier_prbs > 0 {
            (allocated_prbs as f64 / total_carrier_prbs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // 1. Adapt spatial antenna configuration based on PRB traffic load
        let (_new_antennas, reconfigured) = self.mimo_config.adapt_antenna_count(prb_load);
        if reconfigured {
            self.metrics.antenna_reconfigurations_count += 1;
        }

        // 2. Evaluate Cell DTX/DRX schedule
        let is_active_window = self.dtx_pattern.is_active_slot(slot_index);
        let active_level = if is_active_window {
            if prb_load > 0.0 {
                NesSleepLevel::Active
            } else {
                // In active window but empty slot -> Level 1 MicroSleep
                NesSleepLevel::Level1MicroSleep
            }
        } else {
            // In inactive window -> engage configured sleep level
            self.dtx_pattern.inactive_sleep_level
        };

        // 3. Intra-slot Symbol Micro-Sleep Accounting
        // If slot is active, calculate micro-sleep in unallocated symbols (symbols without PDCCH/PDSCH)
        let mut slot_energy_joules = 0.0;
        let symbol_duration_s = self.slot_duration_s / (NR_SYMBOLS_PER_SLOT as f64);

        if active_level == NesSleepLevel::Active {
            for symbol in 0..NR_SYMBOLS_PER_SLOT {
                let symbol_has_data = (allocated_symbol_mask & (1 << symbol)) != 0;
                let symbol_level = if symbol_has_data {
                    NesSleepLevel::Active
                } else {
                    self.metrics.micro_sleep_symbols_count += 1;
                    NesSleepLevel::Level1MicroSleep
                };

                let power_watts = self.power_model.calculate_instantaneous_power(
                    symbol_level,
                    self.mimo_config.active_antennas,
                    prb_load,
                );
                slot_energy_joules += power_watts * symbol_duration_s;
            }
        } else {
            if active_level == NesSleepLevel::Level2SlotSleep {
                self.metrics.slot_sleep_slots_count += 1;
            }
            let power_watts = self.power_model.calculate_instantaneous_power(
                active_level,
                self.mimo_config.active_antennas,
                0.0,
            );
            slot_energy_joules = power_watts * self.slot_duration_s;
        }

        // 4. Baseline Energy Calculation (Full 64T active without sleep)
        let baseline_power_watts = self.power_model.calculate_instantaneous_power(
            NesSleepLevel::Active,
            self.power_model.max_antennas,
            prb_load,
        );
        let baseline_slot_energy = baseline_power_watts * self.slot_duration_s;

        // 5. Update Telemetry
        self.metrics.energy_consumed_joules += slot_energy_joules;
        self.metrics.baseline_energy_joules += baseline_slot_energy;
        self.metrics.data_bits_delivered += delivered_bits;

        (active_level, slot_energy_joules)
    }

    /// Reset energy counters.
    pub fn reset_metrics(&mut self) {
        self.metrics = NesMetrics::default();
    }
}
