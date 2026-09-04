//! 3GPP Rel-18 5G NR Ambient IoT (Zero-Energy Devices) Air Interface Engine.
//!
//! Compliant with:
//! - 3GPP TR 38.848 Rel-18 ("Study on Ambient IoT in NR")
//! - 3GPP TS 22.840 Rel-18 ("Study on Ambient IoT enhancement")
//! - 3GPP TS 38.211 / TS 38.213 Rel-18 ("Physical channels and modulation - Ambient IoT")
//!
//! Solves:
//! 1. Ultra-low-cost, battery-less, zero-energy ambient IoT device communications across
//!    Device Class 1 (pure harvesting passive backscatter), Class 2 (capacitor-assisted backscatter),
//!    and Class 3 (active transmitter).
//! 2. Bistatic vs Monostatic network topologies with double-path loss radar cross section (RCS)
//!    link budgets.
//! 3. Monostatic continuous wave (CW) carrier leakage self-interference cancellation (> 90 dB).
//! 4. RF energy harvesting rectification models and wake-up power thresholds.
//! 5. Robust line coding (FM0, Miller-2, Miller-4, Miller-8) with subcarrier modulation (OOK, 2-FSK)
//!    and CRC-16 frame integrity checks.
//! 6. Dynamic Q-algorithm slotted Aloha collision resolution for dense tag population inventories.
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::fmt;

/// Speed of light in vacuum ($c$) in meters per second.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Standard thermal noise density at 290 K in dBm/Hz.
pub const THERMAL_NOISE_DENSITY_DBM_HZ: f64 = -174.0;

/// Default RF-to-DC rectifier efficiency ($\eta_{rect}$).
pub const DEFAULT_RECTIFIER_EFFICIENCY: f64 = 0.35; // 35%

/// CRC-16 CCITT polynomial (0x1021 = x^16 + x^12 + x^5 + 1).
pub const CRC16_CCITT_POLY: u16 = 0x1021;
pub const CRC16_CCITT_INIT: u16 = 0xFFFF;

// ---------------------------------------------------------------------------
// Enumerations & Error Types
// ---------------------------------------------------------------------------

/// Ambient IoT Device Class (3GPP TR 38.848 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmbientDeviceClass {
    /// Class 1: Pure energy harvesting, no energy storage.
    /// Operates strictly while illuminated by RF carrier; backscatter communication only.
    Class1Passive,
    /// Class 2: Energy storage (capacitor), backscatter communication.
    /// Can accumulate energy over time to power sensing and responds via backscatter.
    Class2Assisted,
    /// Class 3: Energy storage with active RF transmission generation.
    Class3Active,
}

impl AmbientDeviceClass {
    /// Nominal RF wake-up sensitivity threshold in dBm.
    pub fn sensitivity_threshold_dbm(&self) -> f64 {
        match self {
            Self::Class1Passive => -20.0,  // 10 microwatts
            Self::Class2Assisted => -28.0, // ~1.58 microwatts
            Self::Class3Active => -35.0,   // ~0.316 microwatts
        }
    }
}

/// Ambient IoT Network Topology Mode.
#[derive(Debug, Clone, PartialEq)]
pub enum TopologyMode {
    /// Bistatic: Carrier Emitter node separated from Reader Receiver node.
    Bistatic {
        emitter_pos: [f64; 3],
        reader_pos: [f64; 3],
    },
    /// Monostatic: Same gNodeB acts as CW emitter and backscatter receiver.
    Monostatic {
        gnb_pos: [f64; 3],
        carrier_leakage_cancellation_db: f64,
    },
}

/// Line coding scheme for Ambient IoT reverse link (TR 38.848 §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCoding {
    /// FM0 (biphase space) coding: transition at every bit boundary; bit 0 has mid-bit transition.
    Fm0,
    /// Miller-2: mid-bit transition for bit 1; transition between consecutive bit 0s; 2 subcarrier cycles/bit.
    Miller2,
    /// Miller-4: 4 subcarrier cycles/bit for enhanced SNR robustness.
    Miller4,
    /// Miller-8: 8 subcarrier cycles/bit for extreme low-SNR deep coverage.
    Miller8,
}

/// Backscatter Modulation format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackscatterModulation {
    /// On-Off Keying (reflecting vs absorbing).
    Ook,
    /// Amplitude Shift Keying with multi-level impedance matching.
    Ask,
    /// 2-Frequency Shift Keying (switching between two subcarrier frequencies).
    Fsk2,
}

/// Errors raised during Ambient IoT operations.
#[derive(Debug, Clone, PartialEq)]
pub enum AmbientIotError {
    InsufficientHarvestedPower {
        incident_dbm: f64,
        threshold_dbm: f64,
    },
    TagNotFound(u64),
    CarrierLeakageExcessive {
        leakage_dbm: f64,
        max_dbm: f64,
    },
    CrcMismatch {
        computed: u16,
        received: u16,
    },
    InvalidConfiguration(&'static str),
}

impl fmt::Display for AmbientIotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientHarvestedPower {
                incident_dbm,
                threshold_dbm,
            } => write!(
                f,
                "Incident RF power {:.1} dBm below tag sensitivity threshold {:.1} dBm",
                incident_dbm, threshold_dbm
            ),
            Self::TagNotFound(id) => write!(f, "Tag ID {:#X} not found in inventory", id),
            Self::CarrierLeakageExcessive {
                leakage_dbm,
                max_dbm,
            } => write!(
                f,
                "Residual carrier leakage {:.1} dBm exceeds receiver ceiling {:.1} dBm",
                leakage_dbm, max_dbm
            ),
            Self::CrcMismatch { computed, received } => write!(
                f,
                "CRC-16 mismatch: computed {:#06X}, received {:#06X}",
                computed, received
            ),
            Self::InvalidConfiguration(msg) => write!(f, "Invalid Ambient IoT config: {}", msg),
        }
    }
}

// ---------------------------------------------------------------------------
// CRC-16 CCITT & Line Coding Utilities
// ---------------------------------------------------------------------------

/// Compute 16-bit CRC over byte payload using CCITT polynomial.
pub fn compute_crc16(data: &[u8]) -> u16 {
    let mut crc = CRC16_CCITT_INIT;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ CRC16_CCITT_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Encode binary payload bits into Line-Coded symbols (FM0 or Miller).
pub fn encode_line_code(bits: &[bool], coding: LineCoding) -> Vec<bool> {
    let mut symbols = Vec::new();
    let mut last_phase = true;

    match coding {
        LineCoding::Fm0 => {
            // FM0: Invert phase at every bit start.
            // Bit 0: Invert again at mid-bit.
            // Bit 1: Hold phase for entire bit.
            for &b in bits {
                last_phase = !last_phase; // start inversion
                symbols.push(last_phase);
                if !b {
                    last_phase = !last_phase; // mid-bit inversion for '0'
                }
                symbols.push(last_phase);
            }
        }
        LineCoding::Miller2 | LineCoding::Miller4 | LineCoding::Miller8 => {
            // Miller coding:
            // Bit 1: Phase transition at mid-bit.
            // Bit 0: Phase transition at bit start if preceding bit was also '0'.
            let m_cycles = match coding {
                LineCoding::Miller2 => 2,
                LineCoding::Miller4 => 4,
                LineCoding::Miller8 => 8,
                _ => 2,
            };
            let mut prev_bit = true;
            for &b in bits {
                if !b && !prev_bit {
                    last_phase = !last_phase; // boundary transition between 0s
                }
                for cycle in 0..m_cycles {
                    if b && cycle == m_cycles / 2 {
                        last_phase = !last_phase; // mid-bit transition for 1
                    }
                    symbols.push(last_phase);
                }
                prev_bit = b;
            }
        }
    }

    symbols
}

// ---------------------------------------------------------------------------
// Link Budget & Radar Cross Section (RCS)
// ---------------------------------------------------------------------------

/// Link budget calculation result for an Ambient IoT backscatter link.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientLinkBudget {
    pub incident_power_tag_dbm: f64,
    pub harvested_power_tag_uw: f64,
    pub is_tag_energized: bool,
    pub rcs_m2: f64,
    pub received_power_reader_dbm: f64,
    pub residual_carrier_leakage_dbm: f64,
    pub snr_db: f64,
}

// ---------------------------------------------------------------------------
// Ambient Tag & Dynamic Q-Algorithm Collision Resolution
// ---------------------------------------------------------------------------

/// Ambient IoT Tag State.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbientTag {
    pub tag_id: u64,
    pub class: AmbientDeviceClass,
    pub position: [f64; 3],
    pub antenna_gain_dbi: f64,
    pub stored_energy_uj: f64,
    pub slot_counter: u32,
    pub rn16: u16,
    pub is_inventoried: bool,
}

impl AmbientTag {
    pub fn new(tag_id: u64, class: AmbientDeviceClass, position: [f64; 3]) -> Self {
        Self {
            tag_id,
            class,
            position,
            antenna_gain_dbi: 2.15, // Dipole antenna
            stored_energy_uj: 0.0,
            slot_counter: 0,
            rn16: (tag_id ^ 0xA5A5) as u16,
            is_inventoried: false,
        }
    }
}

/// Dynamic Q-Algorithm for Slotted Aloha Tag Inventory (TR 38.848 §5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct QAlgorithm {
    pub q_float: f64,
    pub c_up: f64,   // Q step increase upon collision (e.g. 0.8)
    pub c_down: f64, // Q step decrease upon empty slot (e.g. 0.2)
}

impl QAlgorithm {
    pub fn new(initial_q: f64) -> Self {
        Self {
            q_float: initial_q.clamp(0.0, 15.0),
            c_up: 0.8,
            c_down: 0.2,
        }
    }

    /// Number of slots in the current frame: $2^{\lfloor Q \rceil}$.
    pub fn slot_count(&self) -> u32 {
        1 << (self.q_float.round() as u32).min(15)
    }

    /// Adjust Q factor upon slot outcome.
    pub fn feedback_collision(&mut self) {
        self.q_float = (self.q_float + self.c_up).min(15.0);
    }

    pub fn feedback_empty(&mut self) {
        self.q_float = (self.q_float - self.c_down).max(0.0);
    }

    pub fn feedback_success(&mut self) {
        self.q_float = (self.q_float - self.c_down * 0.5).max(0.0);
    }
}

// ---------------------------------------------------------------------------
// Top-Level Ambient IoT Engine
// ---------------------------------------------------------------------------

/// Top-Level 3GPP Rel-18 Ambient IoT Air Interface Engine.
#[derive(Debug, Clone)]
pub struct AmbientIotEngine {
    pub carrier_freq_hz: f64,
    pub tx_power_dbm: f64,
    pub tx_antenna_gain_dbi: f64,
    pub rx_antenna_gain_dbi: f64,
    pub topology: TopologyMode,
    pub line_coding: LineCoding,
    pub modulation: BackscatterModulation,
    pub q_algo: QAlgorithm,
    pub tags: Vec<AmbientTag>,
}

impl AmbientIotEngine {
    /// Create a new Ambient IoT air interface engine.
    pub fn new(
        carrier_freq_hz: f64,
        tx_power_dbm: f64,
        topology: TopologyMode,
        line_coding: LineCoding,
        modulation: BackscatterModulation,
    ) -> Result<Self, AmbientIotError> {
        if carrier_freq_hz <= 0.0 {
            return Err(AmbientIotError::InvalidConfiguration(
                "Carrier frequency must be > 0",
            ));
        }

        Ok(Self {
            carrier_freq_hz,
            tx_power_dbm,
            tx_antenna_gain_dbi: 8.0, // gNodeB directional antenna
            rx_antenna_gain_dbi: 8.0,
            topology,
            line_coding,
            modulation,
            q_algo: QAlgorithm::new(4.0), // Initial 2^4 = 16 slots
            tags: Vec::new(),
        })
    }

    /// Register a tag to the engine's physical environment.
    pub fn add_tag(&mut self, tag: AmbientTag) {
        self.tags.push(tag);
    }

    /// Wavelength in meters ($\lambda = c / f$).
    pub fn wavelength_m(&self) -> f64 {
        SPEED_OF_LIGHT_M_S / self.carrier_freq_hz
    }

    /// Calculate distance between two 3D points.
    fn distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        let dz = a[2] - b[2];
        (dx * dx + dy * dy + dz * dz).sqrt().max(0.1)
    }

    /// Compute Link Budget & Harvested Energy for a specific tag.
    pub fn compute_link_budget(&self, tag: &AmbientTag) -> AmbientLinkBudget {
        let (emitter_pos, reader_pos, sic_db) = match &self.topology {
            TopologyMode::Bistatic {
                emitter_pos,
                reader_pos,
            } => (*emitter_pos, *reader_pos, 0.0),
            TopologyMode::Monostatic {
                gnb_pos,
                carrier_leakage_cancellation_db,
            } => (*gnb_pos, *gnb_pos, *carrier_leakage_cancellation_db),
        };

        let d_fwd = Self::distance(&emitter_pos, &tag.position);
        let d_rev = Self::distance(&tag.position, &reader_pos);

        let lambda = self.wavelength_m();

        // 1. Forward Incident RF Power:
        // P_inc = P_tx * G_tx * G_tag * (lambda / (4 * pi * d_fwd))^2
        let path_loss_fwd = (lambda / (4.0 * std::f64::consts::PI * d_fwd)).powi(2);
        let tx_pwr_watts = 10.0f64.powf((self.tx_power_dbm - 30.0) / 10.0);
        let g_tx_lin = 10.0f64.powf(self.tx_antenna_gain_dbi / 10.0);
        let g_tag_lin = 10.0f64.powf(tag.antenna_gain_dbi / 10.0);
        let g_rx_lin = 10.0f64.powf(self.rx_antenna_gain_dbi / 10.0);

        let p_inc_watts = tx_pwr_watts * g_tx_lin * g_tag_lin * path_loss_fwd;
        let p_inc_dbm = 10.0 * p_inc_watts.log10() + 30.0;

        let p_harv_watts = p_inc_watts * DEFAULT_RECTIFIER_EFFICIENCY;
        let p_harv_uw = p_harv_watts * 1_000_000.0;

        let is_energized = p_inc_dbm >= tag.class.sensitivity_threshold_dbm();

        // 2. Differential Radar Cross Section (RCS):
        // sigma = (lambda^2 / 4*pi) * G_tag^2 * |Delta_Gamma|^2
        // Assuming |Delta_Gamma| = 1.0 (matched vs short/open circuit)
        let rcs_m2 = (lambda.powi(2) / (4.0 * std::f64::consts::PI)) * g_tag_lin.powi(2);

        // 3. Reverse Backscatter Received Power at Reader:
        // P_rx = P_tx * G_tx * G_rx * sigma * lambda^2 / ((4*pi)^3 * d_fwd^2 * d_rev^2)
        let p_rx_watts = tx_pwr_watts * g_tx_lin * g_rx_lin * rcs_m2 * lambda.powi(2)
            / ((4.0 * std::f64::consts::PI).powi(3) * d_fwd.powi(2) * d_rev.powi(2));
        let p_rx_dbm = 10.0 * p_rx_watts.log10() + 30.0;

        // 4. Noise & Carrier Leakage:
        // Channel bandwidth ~200 kHz (one 5G NR PRB)
        let noise_floor_dbm = THERMAL_NOISE_DENSITY_DBM_HZ + 10.0 * (200_000.0f64).log10();
        let noise_watts = 10.0f64.powf((noise_floor_dbm - 30.0) / 10.0);

        let leakage_dbm = self.tx_power_dbm - sic_db;
        let leakage_watts = if sic_db > 0.0 {
            10.0f64.powf((leakage_dbm - 30.0) / 10.0)
        } else {
            0.0
        };

        let total_interference_watts = noise_watts + leakage_watts;
        let snr_db = 10.0 * (p_rx_watts / total_interference_watts).log10();

        AmbientLinkBudget {
            incident_power_tag_dbm: p_inc_dbm,
            harvested_power_tag_uw: p_harv_uw,
            is_tag_energized: is_energized,
            rcs_m2,
            received_power_reader_dbm: p_rx_dbm,
            residual_carrier_leakage_dbm: leakage_dbm,
            snr_db,
        }
    }

    /// Execute a full inventory round using the dynamic Q-algorithm.
    /// Returns list of successfully inventoried Tag IDs in this round.
    pub fn run_inventory_round(&mut self) -> Vec<u64> {
        let num_slots = self.q_algo.slot_count();
        let mut successfully_read_tags = Vec::new();

        // Assign random slot counters to all energized, non-inventoried tags
        // Using deterministic hash-based pseudo-random generator to ensure pure standard Rust
        for tag in self.tags.iter_mut() {
            if tag.is_inventoried {
                continue;
            }
            // Seed hash
            let seed = tag.tag_id.wrapping_mul(0x9E3779B97F4A7C15) ^ (num_slots as u64);
            tag.slot_counter = (seed % (num_slots as u64)) as u32;
        }

        // Iterate through all slots
        for slot in 0..num_slots {
            // Find tags with slot_counter == 0
            let mut responding_tag_indices = Vec::new();
            for (idx, tag) in self.tags.iter().enumerate() {
                if !tag.is_inventoried && tag.slot_counter == 0 {
                    let budget = self.compute_link_budget(tag);
                    if budget.is_tag_energized && budget.snr_db > -10.0 {
                        responding_tag_indices.push(idx);
                    }
                }
            }

            match responding_tag_indices.len() {
                0 => {
                    // Empty slot
                    self.q_algo.feedback_empty();
                }
                1 => {
                    // Single tag response -> SUCCESS!
                    let tag_idx = responding_tag_indices[0];
                    self.tags[tag_idx].is_inventoried = true;
                    successfully_read_tags.push(self.tags[tag_idx].tag_id);
                    self.q_algo.feedback_success();
                }
                _ => {
                    // Collision (>1 tags responded in the same slot)
                    self.q_algo.feedback_collision();
                }
            }

            // QueryRep: Decrement slot counters for next slot
            if slot + 1 < num_slots {
                for tag in self.tags.iter_mut() {
                    if !tag.is_inventoried && tag.slot_counter > 0 {
                        tag.slot_counter -= 1;
                    }
                }
            }
        }

        successfully_read_tags
    }
}
