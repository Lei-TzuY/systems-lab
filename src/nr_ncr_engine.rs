//! 3GPP Rel-18 Network Controlled Repeater (NCR) Protocol & Control Engine.
//!
//! Implements 3GPP TS 38.300 §16.14, TS 38.213 §17, TS 38.331, and TS 38.106:
//! - Dual-link architecture: Control Link (C-link) via NCR-MT and Access Link (A-link) via NCR-Fwd.
//! - Side Control Information (SCI) handling: dynamic direction, dual beam indications, and RF gain.
//! - Dynamic directional amplification: Downlink, Uplink, Muted (power gating), and Guard switching.
//! - Guard period switching ($T_{g1}$ DL-to-UL and $T_{g2}$ UL-to-DL) enforcement.
//! - RF amplifier modeling: linear gain amplification, power backoff, and saturation clipping ($P_{sat}$).
//! - Thermal noise floor amplification ($-174\text{ dBm/Hz} + 10\log_{10}(BW) + G_{RF} + NF$).
//! - Energy-saving power gating telemetry and beam switching metrics.
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::fmt;

/// Standard number of OFDM symbols per slot with normal cyclic prefix.
pub const SYMBOLS_PER_SLOT: usize = 14;

/// Maximum number of spatial beam indices supported on C-link and A-link ($0..63$).
pub const MAX_BEAM_ID: u8 = 63;

/// Thermal noise spectral density at room temperature ($T = 290\text{ K}$) in dBm/Hz.
pub const THERMAL_NOISE_FLOOR_DBM_HZ: f64 = -174.0;

/// Direction or power state of an individual OFDM symbol in NCR forwarding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmplifyDirection {
    /// Amplifying downlink signal from gNB towards UEs.
    Downlink,
    /// Amplifying uplink signal from UEs towards gNB.
    Uplink,
    /// Power amplifier gated off to save energy and eliminate cell-to-cell noise.
    Muted,
    /// Guard interval switching between DL and UL.
    Guard,
}

/// Operational state of the NCR forwarding engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NcrState {
    Idle,
    Synced,
    ActiveAmplifyDl,
    ActiveAmplifyUl,
    GuardSwitching,
    PowerGated,
}

/// Errors raised during NCR processing.
#[derive(Debug, Clone, PartialEq)]
pub enum NcrError {
    InvalidGain {
        requested_db: f64,
        max_db: f64,
    },
    InvalidBeamId {
        beam_id: u8,
        max: u8,
    },
    GuardTimeViolation {
        symbol_idx: usize,
        elapsed_us: f64,
        required_us: f64,
    },
    NoSciScheduledForSlot(u32),
    InvalidSymbolIndex(usize),
}

impl fmt::Display for NcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NcrError::InvalidGain {
                requested_db,
                max_db,
            } => {
                write!(
                    f,
                    "Requested RF gain {:.1} dB exceeds maximum capability {:.1} dB",
                    requested_db, max_db
                )
            }
            NcrError::InvalidBeamId { beam_id, max } => {
                write!(f, "Beam ID {} exceeds maximum allowable {}", beam_id, max)
            }
            NcrError::GuardTimeViolation {
                symbol_idx,
                elapsed_us,
                required_us,
            } => {
                write!(
                    f,
                    "Symbol {} switched without satisfying guard time: elapsed {:.1} µs, required {:.1} µs",
                    symbol_idx, elapsed_us, required_us
                )
            }
            NcrError::NoSciScheduledForSlot(slot) => {
                write!(
                    f,
                    "No Side Control Information (SCI) scheduled for slot {}",
                    slot
                )
            }
            NcrError::InvalidSymbolIndex(sym) => {
                write!(f, "Invalid symbol index {} (must be 0..13)", sym)
            }
        }
    }
}

impl std::error::Error for NcrError {}

/// Hardware profile and RF capabilities of the Network Controlled Repeater.
#[derive(Debug, Clone, PartialEq)]
pub struct NcrHardwareProfile {
    /// Maximum saturated RF output power in dBm ($P_{sat}$).
    pub max_tx_power_dbm: f64,
    /// Maximum allowable programmable RF gain in dB.
    pub max_gain_db: f64,
    /// Repeater RF noise figure ($NF$) in dB.
    pub noise_figure_db: f64,
    /// Channel operating bandwidth in Hz (e.g. 100 MHz = 1e8 Hz).
    pub channel_bandwidth_hz: f64,
    /// DL-to-UL switching guard time in microseconds ($T_{g1}$).
    pub tg1_guard_us: f64,
    /// UL-to-DL switching guard time in microseconds ($T_{g2}$).
    pub tg2_guard_us: f64,
}

impl NcrHardwareProfile {
    pub fn new(
        max_tx_power_dbm: f64,
        max_gain_db: f64,
        noise_figure_db: f64,
        channel_bandwidth_hz: f64,
        tg1_guard_us: f64,
        tg2_guard_us: f64,
    ) -> Self {
        Self {
            max_tx_power_dbm,
            max_gain_db,
            noise_figure_db,
            channel_bandwidth_hz,
            tg1_guard_us,
            tg2_guard_us,
        }
    }
}

/// Side Control Information (SCI) dynamically signaled from gNB over C-link (TS 38.213 §17).
#[derive(Debug, Clone, PartialEq)]
pub struct SideControlInformation {
    pub slot_idx: u32,
    pub symbol_directions: [AmplifyDirection; SYMBOLS_PER_SLOT],
    pub c_link_beam_id: u8,
    pub a_link_beam_id: u8,
    pub gain_db: f64,
    pub power_backoff_db: f64,
}

impl SideControlInformation {
    pub fn new(
        slot_idx: u32,
        symbol_directions: [AmplifyDirection; SYMBOLS_PER_SLOT],
        c_link_beam_id: u8,
        a_link_beam_id: u8,
        gain_db: f64,
        power_backoff_db: f64,
    ) -> Result<Self, NcrError> {
        if c_link_beam_id > MAX_BEAM_ID {
            return Err(NcrError::InvalidBeamId {
                beam_id: c_link_beam_id,
                max: MAX_BEAM_ID,
            });
        }
        if a_link_beam_id > MAX_BEAM_ID {
            return Err(NcrError::InvalidBeamId {
                beam_id: a_link_beam_id,
                max: MAX_BEAM_ID,
            });
        }

        Ok(Self {
            slot_idx,
            symbol_directions,
            c_link_beam_id,
            a_link_beam_id,
            gain_db,
            power_backoff_db,
        })
    }
}

/// Output of RF amplification for an individual OFDM symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct AmplifiedOutput {
    pub symbol_idx: usize,
    pub state: NcrState,
    pub output_power_dbm: f64,
    pub output_noise_floor_dbm: f64,
    pub is_saturated: bool,
    pub is_power_gated: bool,
    pub active_c_beam: u8,
    pub active_a_beam: u8,
}

/// Operational Telemetry and Performance Metrics for NCR.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NcrMetrics {
    pub total_dl_symbols_amplified: u64,
    pub total_ul_symbols_amplified: u64,
    pub total_muted_symbols: u64,
    pub total_guard_symbols: u64,
    pub total_saturated_symbols: u64,
    pub beam_switches_count: u64,
    pub energy_savings_percentage: f64,
}

/// Network Controlled Repeater (NCR) Forwarding Engine.
#[derive(Debug, Clone)]
pub struct NcrForwardingEngine {
    pub profile: NcrHardwareProfile,
    current_state: NcrState,
    active_sci: Option<SideControlInformation>,
    last_direction: Option<AmplifyDirection>,
    current_c_beam: u8,
    current_a_beam: u8,
    metrics: NcrMetrics,
}

impl NcrForwardingEngine {
    pub fn new(profile: NcrHardwareProfile) -> Self {
        Self {
            profile,
            current_state: NcrState::Idle,
            active_sci: None,
            last_direction: None,
            current_c_beam: 0,
            current_a_beam: 0,
            metrics: NcrMetrics::default(),
        }
    }

    /// Current operational state of the repeater.
    pub fn state(&self) -> NcrState {
        self.current_state
    }

    /// Applies new Side Control Information (SCI) received over C-link.
    pub fn apply_sci(&mut self, sci: SideControlInformation) -> Result<(), NcrError> {
        if sci.gain_db > self.profile.max_gain_db {
            return Err(NcrError::InvalidGain {
                requested_db: sci.gain_db,
                max_db: self.profile.max_gain_db,
            });
        }

        // Check if spatial beams changed
        if sci.c_link_beam_id != self.current_c_beam || sci.a_link_beam_id != self.current_a_beam {
            self.metrics.beam_switches_count += 1;
            self.current_c_beam = sci.c_link_beam_id;
            self.current_a_beam = sci.a_link_beam_id;
        }

        self.active_sci = Some(sci);
        self.current_state = NcrState::Synced;
        Ok(())
    }

    /// Processes and amplifies an incoming RF symbol.
    pub fn process_symbol(
        &mut self,
        symbol_idx: usize,
        input_power_dbm: f64,
    ) -> Result<AmplifiedOutput, NcrError> {
        if symbol_idx >= SYMBOLS_PER_SLOT {
            return Err(NcrError::InvalidSymbolIndex(symbol_idx));
        }

        let (direction, gain_db, power_backoff_db) = match &self.active_sci {
            Some(s) => (
                s.symbol_directions[symbol_idx],
                s.gain_db,
                s.power_backoff_db,
            ),
            None => return Err(NcrError::NoSciScheduledForSlot(0)),
        };

        // Guard transition validation
        if let Some(prev) = self.last_direction {
            if prev == AmplifyDirection::Downlink && direction == AmplifyDirection::Uplink {
                return Err(NcrError::GuardTimeViolation {
                    symbol_idx,
                    elapsed_us: 0.0,
                    required_us: self.profile.tg1_guard_us,
                });
            }
            if prev == AmplifyDirection::Uplink && direction == AmplifyDirection::Downlink {
                return Err(NcrError::GuardTimeViolation {
                    symbol_idx,
                    elapsed_us: 0.0,
                    required_us: self.profile.tg2_guard_us,
                });
            }
        }
        self.last_direction = Some(direction);

        let (output_pwr, state, is_sat, is_gated) = match direction {
            AmplifyDirection::Downlink => {
                self.metrics.total_dl_symbols_amplified += 1;
                let effective_gain = (gain_db - power_backoff_db).max(0.0);
                let linear_pwr = input_power_dbm + effective_gain;
                let is_sat = linear_pwr >= self.profile.max_tx_power_dbm;
                if is_sat {
                    self.metrics.total_saturated_symbols += 1;
                }
                let out_pwr = linear_pwr.min(self.profile.max_tx_power_dbm);
                (out_pwr, NcrState::ActiveAmplifyDl, is_sat, false)
            }
            AmplifyDirection::Uplink => {
                self.metrics.total_ul_symbols_amplified += 1;
                let effective_gain = (gain_db - power_backoff_db).max(0.0);
                let linear_pwr = input_power_dbm + effective_gain;
                let is_sat = linear_pwr >= self.profile.max_tx_power_dbm;
                if is_sat {
                    self.metrics.total_saturated_symbols += 1;
                }
                let out_pwr = linear_pwr.min(self.profile.max_tx_power_dbm);
                (out_pwr, NcrState::ActiveAmplifyUl, is_sat, false)
            }
            AmplifyDirection::Muted => {
                self.metrics.total_muted_symbols += 1;
                (-120.0, NcrState::PowerGated, false, true)
            }
            AmplifyDirection::Guard => {
                self.metrics.total_guard_symbols += 1;
                (-120.0, NcrState::GuardSwitching, false, true)
            }
        };

        self.current_state = state;
        self.update_energy_metrics();

        let noise_floor = if is_gated {
            -174.0 + 10.0 * self.profile.channel_bandwidth_hz.log10()
        } else {
            self.output_noise_floor_dbm(gain_db)
        };

        Ok(AmplifiedOutput {
            symbol_idx,
            state,
            output_power_dbm: output_pwr,
            output_noise_floor_dbm: noise_floor,
            is_saturated: is_sat,
            is_power_gated: is_gated,
            active_c_beam: self.current_c_beam,
            active_a_beam: self.current_a_beam,
        })
    }

    /// Computes output thermal noise floor in dBm:
    /// $$N_{out} = -174 + 10 \log_{10}(BW) + G_{RF} + NF$$
    pub fn output_noise_floor_dbm(&self, active_gain_db: f64) -> f64 {
        let bw_term = 10.0 * self.profile.channel_bandwidth_hz.log10();
        THERMAL_NOISE_FLOOR_DBM_HZ + bw_term + active_gain_db + self.profile.noise_figure_db
    }

    fn update_energy_metrics(&mut self) {
        let total = self.metrics.total_dl_symbols_amplified
            + self.metrics.total_ul_symbols_amplified
            + self.metrics.total_muted_symbols
            + self.metrics.total_guard_symbols;
        if total > 0 {
            self.metrics.energy_savings_percentage =
                (self.metrics.total_muted_symbols as f64 / total as f64) * 100.0;
        }
    }

    /// Returns telemetry metrics.
    pub fn metrics(&self) -> &NcrMetrics {
        &self.metrics
    }
}
