//! 3GPP Rel-17 5G NR Coverage Enhancement (CovEnh) Protocol Engine.
//!
//! Implements 3GPP TS 38.214 §6.1.2.1, TS 38.213 §9.2.6 / §11.1, TS 38.211 §6.4.1.1.3,
//! and TS 38.331 specifications:
//! - PUSCH Repetition Type A (slot-level multi-slot repetitions with RV cycling and hopping).
//! - PUSCH Repetition Type B (sub-slot nominal repetitions with dynamic segmentation into
//!   actual repetitions across slot boundaries and invalid DL/SSB symbol collisions).
//! - Transport Block Over Multiple Slots (TBoMS) joint rate-matching and coding gain analysis.
//! - Cross-Slot DMRS Bundling maintaining RF phase continuity and power consistency across slots.
//! - TDD semi-static slot format validity auditing.
//! - Coverage range extension multiplier ($d_{new}/d_{old} = 10^{\Delta G / (10\alpha)}$).

use std::fmt;

/// Standard number of OFDM symbols per slot with normal cyclic prefix.
pub const NR_SYMBOLS_PER_SLOT: u8 = 14;

/// Standard number of subcarriers per Physical Resource Block.
pub const NR_SUBCARRIERS_PER_PRB: usize = 12;

/// Default terrestrial pathloss exponent ($\alpha = 3.5$) for cell-edge coverage extension.
pub const DEFAULT_PATHLOSS_EXPONENT: f64 = 3.5;

/// PUSCH repetition type (3GPP TS 38.214 Section 6.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PuschRepetitionType {
    /// Type A: Slot-level repetitions over consecutive slots with identical symbol allocation.
    TypeA,
    /// Type B: Sub-slot nominal repetitions dynamically segmented into actual repetitions.
    TypeB,
}

/// Redundancy Version (RV) sequence cycle pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RvPattern {
    /// Standard 3GPP Rel-15/16/17 pattern: [0, 2, 3, 1]
    Pattern0231,
    /// Alternate pattern for fast retransmission: [0, 3, 0, 3]
    Pattern0303,
}

impl RvPattern {
    pub fn get_rv(&self, rep_idx: usize) -> u8 {
        match self {
            RvPattern::Pattern0231 => {
                let seq = [0, 2, 3, 1];
                seq[rep_idx % 4]
            }
            RvPattern::Pattern0303 => {
                let seq = [0, 3, 0, 3];
                seq[rep_idx % 4]
            }
        }
    }
}

/// Direction or usage of an OFDM symbol in TDD frame structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TddSymbolType {
    Downlink,
    Uplink,
    Flexible,
    Invalid, // Reserved for SSB, guard period, or dynamic blanking
}

/// Reason for a phase discontinuity break in cross-slot DMRS bundling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseDiscontinuityReason {
    None,
    TransmitPowerStepExceeded { step_db: f64, max_db: f64 },
    FrequencyHoppingBreak,
    NonConsecutiveSlotGap { gap_slots: u32 },
    BundleBoundaryReached,
}

/// Errors raised during Coverage Enhancement processing.
#[derive(Debug, Clone, PartialEq)]
pub enum CovEnhError {
    InvalidSymbolRange { start: u8, duration: u8 },
    InvalidRepetitionCount(u32),
    ZeroAvailableUlSymbols,
    TbomsInvalidSlotCount(u8),
    EmptyTddPattern,
}

impl fmt::Display for CovEnhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CovEnhError::InvalidSymbolRange { start, duration } => {
                write!(
                    f,
                    "Invalid symbol range: start {} + duration {} exceeds slot boundary (14)",
                    start, duration
                )
            }
            CovEnhError::InvalidRepetitionCount(n) => {
                write!(f, "Invalid repetition count: {} (must be > 0 and <= 32)", n)
            }
            CovEnhError::ZeroAvailableUlSymbols => {
                write!(
                    f,
                    "No valid uplink symbols available for actual repetitions"
                )
            }
            CovEnhError::TbomsInvalidSlotCount(n) => {
                write!(f, "TBoMS requires 2, 4, 8, or 16 slots, requested: {}", n)
            }
            CovEnhError::EmptyTddPattern => {
                write!(f, "TDD slot configuration pattern cannot be empty")
            }
        }
    }
}

impl std::error::Error for CovEnhError {}

/// Pre-configured TDD frame format slot structure.
#[derive(Debug, Clone, PartialEq)]
pub struct TddSlotFormat {
    pub slot_period: u32,
    pub symbol_types: Vec<Vec<TddSymbolType>>, // [slot_in_period][symbol 0..13]
}

impl TddSlotFormat {
    /// Creates a typical 5G NR TDD pattern: e.g. DDDFU (8 DL, 2 Flex, 4 UL in last slot).
    pub fn standard_5g_tdd_4to1() -> Self {
        let slot_period = 5;
        let mut symbol_types = Vec::with_capacity(5);

        // Slots 0, 1, 2: All Downlink (14 DL)
        for _ in 0..3 {
            symbol_types.push(vec![TddSymbolType::Downlink; NR_SYMBOLS_PER_SLOT as usize]);
        }

        // Slot 3: Special/Flexible slot (10 DL, 2 Flex/Guard, 2 UL)
        let mut special_slot = Vec::with_capacity(14);
        for _ in 0..10 {
            special_slot.push(TddSymbolType::Downlink);
        }
        special_slot.push(TddSymbolType::Invalid); // Guard period
        special_slot.push(TddSymbolType::Invalid);
        special_slot.push(TddSymbolType::Uplink);
        special_slot.push(TddSymbolType::Uplink);
        symbol_types.push(special_slot);

        // Slot 4: All Uplink (14 UL)
        symbol_types.push(vec![TddSymbolType::Uplink; NR_SYMBOLS_PER_SLOT as usize]);

        Self {
            slot_period,
            symbol_types,
        }
    }

    /// Creates an all-uplink FDD configuration (every symbol in every slot is UL).
    pub fn all_uplink_fdd() -> Self {
        Self {
            slot_period: 1,
            symbol_types: vec![vec![TddSymbolType::Uplink; NR_SYMBOLS_PER_SLOT as usize]],
        }
    }

    /// Checks if a specific symbol in a slot is available for uplink transmission.
    pub fn is_uplink_symbol(&self, slot: u32, symbol: u8) -> bool {
        if self.symbol_types.is_empty() || symbol >= NR_SYMBOLS_PER_SLOT {
            return false;
        }
        let period_idx = (slot % self.slot_period) as usize;
        let sym_type = self.symbol_types[period_idx][symbol as usize];
        sym_type == TddSymbolType::Uplink
    }
}

/// Nominal Repetition before sub-slot segmentation (TS 38.214 §6.1.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NominalRepetition {
    pub nominal_idx: u32,
    pub start_slot: u32,
    pub start_symbol: u8,
    pub num_symbols: u8,
}

/// Actual Repetition scheduled on the physical channel after segmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActualRepetition {
    pub actual_idx: u32,
    pub nominal_idx: u32,
    pub slot_idx: u32,
    pub start_symbol: u8,
    pub num_symbols: u8,
    pub rv: u8,
}

/// PUSCH Repetition Type B Segmentation Engine (TS 38.214 Section 6.1.2.1).
#[derive(Debug, Clone)]
pub struct PuschTypeBSegmenter {
    pub tdd_format: TddSlotFormat,
    pub rv_pattern: RvPattern,
}

impl PuschTypeBSegmenter {
    pub fn new(tdd_format: TddSlotFormat, rv_pattern: RvPattern) -> Self {
        Self {
            tdd_format,
            rv_pattern,
        }
    }

    /// Segments nominal repetitions into actual repetitions:
    /// - Splits nominal repetitions crossing slot boundaries into separate segments.
    /// - Omits invalid or non-uplink symbols (DL, guard, SSB).
    /// - Discards any segment of duration 0 symbols.
    /// - Assigns redundancy version (RV) only to valid actual repetitions.
    pub fn segment_nominal_repetitions(
        &self,
        start_slot: u32,
        start_symbol: u8,
        nominal_length: u8,
        num_nominal_repetitions: u32,
    ) -> Result<Vec<ActualRepetition>, CovEnhError> {
        if start_symbol >= NR_SYMBOLS_PER_SLOT
            || nominal_length == 0
            || nominal_length > NR_SYMBOLS_PER_SLOT
        {
            return Err(CovEnhError::InvalidSymbolRange {
                start: start_symbol,
                duration: nominal_length,
            });
        }
        if num_nominal_repetitions == 0 || num_nominal_repetitions > 32 {
            return Err(CovEnhError::InvalidRepetitionCount(num_nominal_repetitions));
        }

        let mut actual_repetitions = Vec::new();
        let mut actual_counter = 0u32;

        // Current nominal allocation tracking
        let mut cur_slot = start_slot;
        let mut cur_sym = start_symbol;

        for nom_idx in 0..num_nominal_repetitions {
            let mut remaining_nominal_symbols = nominal_length;

            while remaining_nominal_symbols > 0 {
                let symbols_in_cur_slot =
                    (NR_SYMBOLS_PER_SLOT - cur_sym).min(remaining_nominal_symbols);

                // Check symbol-by-symbol validity within this slot segment
                let mut seg_start: Option<u8> = None;
                let mut seg_len = 0u8;

                for s in 0..symbols_in_cur_slot {
                    let sym_idx = cur_sym + s;
                    let is_ul = self.tdd_format.is_uplink_symbol(cur_slot, sym_idx);

                    if is_ul {
                        if seg_start.is_none() {
                            seg_start = Some(sym_idx);
                        }
                        seg_len += 1;
                    } else if let Some(start) = seg_start {
                        // Segment ended due to non-UL symbol
                        let rv = self.rv_pattern.get_rv(actual_counter as usize);
                        actual_repetitions.push(ActualRepetition {
                            actual_idx: actual_counter,
                            nominal_idx: nom_idx,
                            slot_idx: cur_slot,
                            start_symbol: start,
                            num_symbols: seg_len,
                            rv,
                        });
                        actual_counter += 1;
                        seg_start = None;
                        seg_len = 0;
                    }
                }

                // If a segment reached the end of the slot slice
                if let Some(start) = seg_start {
                    if seg_len > 0 {
                        let rv = self.rv_pattern.get_rv(actual_counter as usize);
                        actual_repetitions.push(ActualRepetition {
                            actual_idx: actual_counter,
                            nominal_idx: nom_idx,
                            slot_idx: cur_slot,
                            start_symbol: start,
                            num_symbols: seg_len,
                            rv,
                        });
                        actual_counter += 1;
                    }
                }

                remaining_nominal_symbols -= symbols_in_cur_slot;

                // Advance to next slot boundary
                cur_slot += 1;
                cur_sym = 0;
            }

            // Next nominal repetition starts immediately after current nominal ends
            // Wait, if it didn't cross boundary, adjust cur_slot and cur_sym
            let total_symbols_from_start =
                (start_symbol as u32) + (nom_idx + 1) * (nominal_length as u32);
            cur_slot = start_slot + total_symbols_from_start / (NR_SYMBOLS_PER_SLOT as u32);
            cur_sym = (total_symbols_from_start % (NR_SYMBOLS_PER_SLOT as u32)) as u8;
        }

        if actual_repetitions.is_empty() {
            return Err(CovEnhError::ZeroAvailableUlSymbols);
        }

        Ok(actual_repetitions)
    }
}

/// Transport Block Over Multiple Slots (TBoMS) Engine (TS 38.214 §6.1.2.1 Rel-17).
#[derive(Debug, Clone, PartialEq)]
pub struct TbomsConfig {
    pub tb_size_bits: usize,
    pub num_slots: u8, // N in {2, 4, 8, 16}
    pub prbs_per_slot: usize,
    pub ul_symbols_per_slot: u8,
    pub dmrs_symbols_per_slot: u8,
    pub modulation_order: u8, // Q_m: 2 (QPSK), 4 (16QAM), 6 (64QAM)
}

impl TbomsConfig {
    pub fn new(
        tb_size_bits: usize,
        num_slots: u8,
        prbs_per_slot: usize,
        ul_symbols_per_slot: u8,
        dmrs_symbols_per_slot: u8,
        modulation_order: u8,
    ) -> Result<Self, CovEnhError> {
        if ![2, 4, 8, 16].contains(&num_slots) {
            return Err(CovEnhError::TbomsInvalidSlotCount(num_slots));
        }
        if ul_symbols_per_slot <= dmrs_symbols_per_slot || ul_symbols_per_slot > NR_SYMBOLS_PER_SLOT
        {
            return Err(CovEnhError::InvalidSymbolRange {
                start: dmrs_symbols_per_slot,
                duration: ul_symbols_per_slot,
            });
        }

        Ok(Self {
            tb_size_bits,
            num_slots,
            prbs_per_slot,
            ul_symbols_per_slot,
            dmrs_symbols_per_slot,
            modulation_order,
        })
    }

    /// Total available REs per slot for PUSCH data (excluding DMRS).
    pub fn data_res_per_slot(&self) -> usize {
        let data_symbols = (self.ul_symbols_per_slot - self.dmrs_symbols_per_slot) as usize;
        self.prbs_per_slot * NR_SUBCARRIERS_PER_PRB * data_symbols
    }

    /// Total available coded bits across all N slots.
    pub fn total_available_coded_bits(&self) -> usize {
        let total_res = self.data_res_per_slot() * (self.num_slots as usize);
        total_res * (self.modulation_order as usize)
    }

    /// Effective code rate: (TB_bits + CRC) / Total_Coded_Bits.
    pub fn effective_code_rate(&self) -> f64 {
        let crc_bits = if self.tb_size_bits > 3824 { 24 } else { 16 };
        let total_info_bits = (self.tb_size_bits + crc_bits) as f64;
        let total_coded = self.total_available_coded_bits() as f64;
        total_info_bits / total_coded
    }

    /// Estimated coding gain in dB compared to transmitting the TB in a single slot.
    pub fn estimated_coding_gain_db(&self) -> f64 {
        // Splitting TB across N slots lowers effective code rate by factor of N,
        // yielding coding gain approximately 10 * log10(N) dB minus small implementation penalty
        let n = self.num_slots as f64;
        let theoretical_gain = 10.0 * n.log10();
        let implementation_penalty = 0.5; // ~0.5 dB penalty due to channel variance
        (theoretical_gain - implementation_penalty).max(0.0)
    }
}

/// Cross-Slot DMRS Bundling Controller (TS 38.211 §6.4.1.1.3 / TS 38.331 Rel-17).
#[derive(Debug, Clone)]
pub struct DmrsBundlingController {
    pub bundle_size: u8, // Nominal bundle size (e.g. 2, 4, 8 consecutive slots)
    pub max_power_step_db: f64,
    current_bundle_slots: u8,
    last_slot_idx: Option<u32>,
    last_power_dbm: Option<f64>,
}

impl DmrsBundlingController {
    pub fn new(bundle_size: u8, max_power_step_db: f64) -> Self {
        Self {
            bundle_size,
            max_power_step_db,
            current_bundle_slots: 0,
            last_slot_idx: None,
            last_power_dbm: None,
        }
    }

    /// Evaluates phase continuity for an incoming slot within a DMRS bundle.
    /// Returns the continuity evaluation outcome.
    pub fn evaluate_slot(
        &mut self,
        slot_idx: u32,
        power_dbm: f64,
        frequency_hopped: bool,
    ) -> PhaseDiscontinuityReason {
        // 1. Frequency hopping immediately breaks phase continuity
        if frequency_hopped {
            self.reset_bundle(slot_idx, power_dbm);
            return PhaseDiscontinuityReason::FrequencyHoppingBreak;
        }

        // 2. Non-consecutive slot gap breaks phase continuity
        if let Some(prev_slot) = self.last_slot_idx {
            if slot_idx != prev_slot + 1 {
                let gap = slot_idx.saturating_sub(prev_slot);
                self.reset_bundle(slot_idx, power_dbm);
                return PhaseDiscontinuityReason::NonConsecutiveSlotGap { gap_slots: gap };
            }
        }

        // 3. Transmit power change exceeding threshold breaks phase continuity
        if let Some(prev_pwr) = self.last_power_dbm {
            let step = (power_dbm - prev_pwr).abs();
            if step > self.max_power_step_db {
                self.reset_bundle(slot_idx, power_dbm);
                return PhaseDiscontinuityReason::TransmitPowerStepExceeded {
                    step_db: step,
                    max_db: self.max_power_step_db,
                };
            }
        }

        // 4. Check if bundle boundary has been reached
        self.current_bundle_slots += 1;
        self.last_slot_idx = Some(slot_idx);
        self.last_power_dbm = Some(power_dbm);

        if self.current_bundle_slots >= self.bundle_size {
            self.current_bundle_slots = 0; // Next slot will start a new bundle
            return PhaseDiscontinuityReason::BundleBoundaryReached;
        }

        PhaseDiscontinuityReason::None
    }

    fn reset_bundle(&mut self, slot_idx: u32, power_dbm: f64) {
        self.current_bundle_slots = 1;
        self.last_slot_idx = Some(slot_idx);
        self.last_power_dbm = Some(power_dbm);
    }

    /// Estimated SNR gain in dB achieved through joint channel estimation over the bundle.
    pub fn joint_channel_est_gain_db(&self) -> f64 {
        if self.bundle_size <= 1 {
            return 0.0;
        }
        let n = self.bundle_size as f64;
        // Joint channel filtering improves channel estimate SNR by ~10 * log10(N) - 0.5 dB
        (10.0 * n.log10() - 0.5).max(0.0)
    }
}

/// Operational Telemetry and Coverage Extension Gain Metrics.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CovEnhMetrics {
    pub total_nominal_repetitions: u32,
    pub total_actual_repetitions: u32,
    pub discarded_segments: u32,
    pub cumulative_snr_gain_db: f64,
    pub coverage_range_multiplier: f64,
}

impl CovEnhMetrics {
    /// Computes cell radius coverage extension multiplier from cumulative link gain:
    /// $d_{new} / d_{old} = 10^{\Delta G / (10 \cdot \alpha)}$
    pub fn compute_range_extension(snr_gain_db: f64, pathloss_exp: f64) -> f64 {
        let exponent = snr_gain_db / (10.0 * pathloss_exp);
        10.0f64.powf(exponent)
    }
}
