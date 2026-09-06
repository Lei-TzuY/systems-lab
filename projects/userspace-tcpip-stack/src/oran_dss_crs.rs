//! O-RAN WG4 Open Fronthaul Dynamic Spectrum Sharing (DSS) & LTE CRS Rate Matching Engine.
//!
//! Implements O-RAN.WG4.CUS.0-v07.00 Section 7.5.3.3 and 3GPP TS 38.214 §5.1.4.3 / TS 36.211 §6.10.1:
//! - LTE Cell-specific Reference Signal (CRS) coordinate calculation for 1, 2, and 4 antenna ports.
//! - Bit-parallel PRB puncture masks ($14 \times \text{u16}$) for zero-overhead PDSCH rate matching.
//! - Mixed numerology translation (15 kHz LTE vs 15 kHz / 30 kHz NR subcarrier spacing).
//! - MBSFN subframe CRS muting/exclusion modeling for enhanced NR spectral efficiency.
//! - O-RAN C-Plane Section Type 1 with DSS Section Extension descriptors serialization/deserialization.
//! - Capacity degradation and MCS/code-rate scaling telemetry.
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::fmt;

/// Standard number of subcarriers per Physical Resource Block.
pub const SUBCARRIERS_PER_PRB: usize = 12;

/// Standard number of OFDM symbols per slot with normal cyclic prefix.
pub const SYMBOLS_PER_SLOT_NORMAL_CP: usize = 14;

/// Standard number of OFDM symbols per slot with extended cyclic prefix.
pub const SYMBOLS_PER_SLOT_EXTENDED_CP: usize = 12;

/// Maximum number of LTE physical cell IDs ($0..503$).
pub const MAX_CELL_ID: u16 = 503;

/// Maximum theoretical code rate threshold before transmission failure occurs (0.93).
pub const MAX_EFFECTIVE_CODE_RATE: f64 = 0.93;

/// Number of LTE antenna ports transmitting CRS (3GPP TS 36.211 §6.10.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LteAntennaPorts {
    OnePort = 1,
    TwoPorts = 2,
    FourPorts = 4,
}

/// LTE Cyclic Prefix type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LteCyclicPrefix {
    Normal,
    Extended,
}

/// 5G NR Subcarrier Spacing (SCS) numerology ($\mu$).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NrSubcarrierSpacing {
    /// $\mu = 0$: 15 kHz SCS (1 ms slot, 14 symbols, matches LTE grid 1:1).
    Scs15kHz = 0,
    /// $\mu = 1$: 30 kHz SCS (0.5 ms slot, 14 symbols, 1 NR PRB spans 2 LTE PRBs).
    Scs30kHz = 1,
}

/// Errors raised during DSS and CRS processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DssError {
    InvalidCellId(u16),
    InvalidCarrierPrb(u16),
    InvalidSubframe(u8),
    InvalidSlotIndex(u8),
    EffectiveCodeRateExceeded { rate: u32, threshold: u32 }, // Fixed-point: 1000 * rate
    BufferTooShort { need: usize, got: usize },
    InvalidExtensionLength(usize),
}

impl fmt::Display for DssError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DssError::InvalidCellId(id) => {
                write!(f, "Invalid LTE Cell ID {} (must be 0..503)", id)
            }
            DssError::InvalidCarrierPrb(prb) => {
                write!(
                    f,
                    "Invalid carrier PRB count {} (must be > 0 and <= 275)",
                    prb
                )
            }
            DssError::InvalidSubframe(sf) => {
                write!(f, "Invalid subframe number {} (must be 0..9)", sf)
            }
            DssError::InvalidSlotIndex(slot) => {
                write!(f, "Invalid slot index {} for current numerology", slot)
            }
            DssError::EffectiveCodeRateExceeded { rate, threshold } => {
                write!(
                    f,
                    "Effective code rate {:.3} exceeds threshold {:.3}",
                    (*rate as f64) / 1000.0,
                    (*threshold as f64) / 1000.0
                )
            }
            DssError::BufferTooShort { need, got } => {
                write!(f, "Buffer truncated: need {} bytes, got {}", need, got)
            }
            DssError::InvalidExtensionLength(len) => {
                write!(f, "Invalid O-RAN DSS extension length: {} bytes", len)
            }
        }
    }
}

impl std::error::Error for DssError {}

/// LTE Cell-specific Reference Signal (CRS) Configuration (3GPP TS 38.214 §5.1.4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LteCrsConfig {
    pub cell_id: u16,
    pub carrier_prb: u16,
    pub antenna_ports: LteAntennaPorts,
    pub cyclic_prefix: LteCyclicPrefix,
    pub v_shift: u8,
    /// 10-bit bitmask indicating which subframes ($0..9$) are configured as MBSFN.
    /// In MBSFN subframes, CRS is only present in the non-MBSFN region (symbols 0 and 1).
    pub mbsfn_subframe_mask: u16,
}

impl LteCrsConfig {
    pub fn new(
        cell_id: u16,
        carrier_prb: u16,
        antenna_ports: LteAntennaPorts,
        cyclic_prefix: LteCyclicPrefix,
        mbsfn_subframe_mask: u16,
    ) -> Result<Self, DssError> {
        if cell_id > MAX_CELL_ID {
            return Err(DssError::InvalidCellId(cell_id));
        }
        if carrier_prb == 0 || carrier_prb > 275 {
            return Err(DssError::InvalidCarrierPrb(carrier_prb));
        }
        let v_shift = (cell_id % 6) as u8;

        Ok(Self {
            cell_id,
            carrier_prb,
            antenna_ports,
            cyclic_prefix,
            v_shift,
            mbsfn_subframe_mask,
        })
    }

    /// Checks if a subframe ($0..9$) is designated as MBSFN.
    #[inline]
    pub fn is_mbsfn_subframe(&self, subframe: u8) -> bool {
        if subframe > 9 {
            return false;
        }
        (self.mbsfn_subframe_mask & (1 << subframe)) != 0
    }
}

/// Bit-parallel puncture mask for a single PRB across all OFDM symbols in a slot.
/// Each `u16` corresponds to one symbol, where bit `k` ($0 \le k < 12$) is 1 if subcarrier `k`
/// is punctured (occupied by LTE CRS or zero-power muted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrsPunctureMask {
    pub symbol_masks: [u16; SYMBOLS_PER_SLOT_NORMAL_CP],
}

impl Default for CrsPunctureMask {
    fn default() -> Self {
        Self {
            symbol_masks: [0u16; SYMBOLS_PER_SLOT_NORMAL_CP],
        }
    }
}

impl CrsPunctureMask {
    /// Returns true if the specific resource element $(k, l)$ is punctured.
    #[inline]
    pub fn is_punctured(&self, symbol: usize, subcarrier: usize) -> bool {
        if symbol >= SYMBOLS_PER_SLOT_NORMAL_CP || subcarrier >= SUBCARRIERS_PER_PRB {
            return false;
        }
        (self.symbol_masks[symbol] & (1 << subcarrier)) != 0
    }

    /// Total number of punctured Resource Elements in this PRB across the slot.
    #[inline]
    pub fn total_punctured_res(&self) -> usize {
        self.symbol_masks
            .iter()
            .map(|&m| m.count_ones() as usize)
            .sum()
    }

    /// Total number of available (usable) PDSCH Resource Elements in this PRB.
    #[inline]
    pub fn available_pdsch_res(&self) -> usize {
        let total = SYMBOLS_PER_SLOT_NORMAL_CP * SUBCARRIERS_PER_PRB;
        total.saturating_sub(self.total_punctured_res())
    }
}

/// High-performance Bit-Parallel CRS Puncture Filter Engine.
#[derive(Debug, Clone)]
pub struct CrsPunctureFilter {
    pub config: LteCrsConfig,
    pub nr_scs: NrSubcarrierSpacing,
}

impl CrsPunctureFilter {
    pub fn new(config: LteCrsConfig, nr_scs: NrSubcarrierSpacing) -> Self {
        Self { config, nr_scs }
    }

    /// Generates the exact bit-parallel puncture mask for a given PRB in a specified subframe and slot.
    pub fn generate_prb_mask(
        &self,
        subframe: u8,
        slot_in_subframe: u8,
        prb_idx: u16,
    ) -> Result<CrsPunctureMask, DssError> {
        if subframe > 9 {
            return Err(DssError::InvalidSubframe(subframe));
        }

        let is_mbsfn = self.config.is_mbsfn_subframe(subframe);
        let mut mask = CrsPunctureMask::default();

        match self.nr_scs {
            NrSubcarrierSpacing::Scs15kHz => {
                if slot_in_subframe != 0 {
                    return Err(DssError::InvalidSlotIndex(slot_in_subframe));
                }
                self.fill_mask_15khz(&mut mask, is_mbsfn, prb_idx);
            }
            NrSubcarrierSpacing::Scs30kHz => {
                if slot_in_subframe > 1 {
                    return Err(DssError::InvalidSlotIndex(slot_in_subframe));
                }
                self.fill_mask_30khz(&mut mask, is_mbsfn, slot_in_subframe, prb_idx);
            }
        }

        Ok(mask)
    }

    /// Computes 15 kHz 1:1 mapping with LTE subframe.
    fn fill_mask_15khz(&self, mask: &mut CrsPunctureMask, is_mbsfn: bool, prb_idx: u16) {
        let v_shift = self.config.v_shift;
        let ports = self.config.antenna_ports;

        // In MBSFN subframes, CRS is absent in symbols 2..13 (only present in symbols 0 and 1)
        let max_symbol = if is_mbsfn { 2 } else { 14 };

        // Symbol indices for ports 0 and 1: {0, 4, 7, 11}
        for &l in &[0usize, 4, 7, 11] {
            if l >= max_symbol {
                continue;
            }
            let v = match l {
                0 | 7 => 0u8,
                4 | 11 => 3u8,
                _ => unreachable!(),
            };

            // Port 0 / 1 subcarrier calculation
            let offset_p0 = (v + v_shift) % 6;
            let offset_p1 = (v + 3 + v_shift) % 6;

            let mut sym_mask = 0u16;

            // Across the 12 subcarriers of this PRB
            for sc in 0..12 {
                let global_sc = (prb_idx as usize * 12 + sc) % 6;
                if global_sc == offset_p0 as usize {
                    sym_mask |= 1 << sc;
                }
                if ports != LteAntennaPorts::OnePort && global_sc == offset_p1 as usize {
                    sym_mask |= 1 << sc;
                }
            }
            mask.symbol_masks[l] |= sym_mask;
        }

        // Ports 2 and 3 symbols: {1, 8}
        if ports == LteAntennaPorts::FourPorts {
            for &l in &[1usize, 8] {
                if l >= max_symbol {
                    continue;
                }
                let v = if l == 1 { 0u8 } else { 3u8 };
                let offset_p2 = (v + v_shift) % 6;
                let offset_p3 = (v + 3 + v_shift) % 6;

                let mut sym_mask = 0u16;
                for sc in 0..12 {
                    let global_sc = (prb_idx as usize * 12 + sc) % 6;
                    if global_sc == offset_p2 as usize || global_sc == offset_p3 as usize {
                        sym_mask |= 1 << sc;
                    }
                }
                mask.symbol_masks[l] |= sym_mask;
            }
        }
    }

    /// Computes 30 kHz NR mapping over 15 kHz LTE grid.
    /// Each 30 kHz NR subcarrier corresponds to 2 LTE 15 kHz subcarriers in frequency.
    /// Each 30 kHz NR slot corresponds to half of an LTE 15 kHz subframe in time.
    fn fill_mask_30khz(
        &self,
        mask: &mut CrsPunctureMask,
        is_mbsfn: bool,
        slot_in_subframe: u8,
        prb_idx: u16,
    ) -> () {
        // Generate underlying 15 kHz masks for the 2 LTE PRBs covered by this 1 NR PRB
        let lte_prb0 = prb_idx * 2;
        let lte_prb1 = prb_idx * 2 + 1;

        let mut lte_mask0 = CrsPunctureMask::default();
        let mut lte_mask1 = CrsPunctureMask::default();
        self.fill_mask_15khz(&mut lte_mask0, is_mbsfn, lte_prb0);
        self.fill_mask_15khz(&mut lte_mask1, is_mbsfn, lte_prb1);

        // Map LTE symbols into NR symbols:
        // NR Slot 0 maps to LTE symbols 0..6
        // NR Slot 1 maps to LTE symbols 7..13
        let lte_sym_offset = if slot_in_subframe == 0 { 0 } else { 7 };

        for nr_sym in 0..14 {
            let lte_sym = lte_sym_offset + (nr_sym / 2);
            if lte_sym >= 14 {
                continue;
            }

            let m0 = lte_mask0.symbol_masks[lte_sym];
            let m1 = lte_mask1.symbol_masks[lte_sym];

            // 1 NR subcarrier covers 2 LTE subcarriers
            let mut nr_sym_mask = 0u16;
            for nr_sc in 0..12 {
                let lte_sc_start = nr_sc * 2;
                let punctured = if lte_sc_start < 12 {
                    let sc0 = lte_sc_start;
                    let sc1 = lte_sc_start + 1;
                    ((m0 & (1 << sc0)) != 0) || (sc1 < 12 && (m0 & (1 << sc1)) != 0)
                } else {
                    let sc0 = lte_sc_start - 12;
                    let sc1 = sc0 + 1;
                    ((m1 & (1 << sc0)) != 0) || (sc1 < 12 && (m1 & (1 << sc1)) != 0)
                };

                if punctured {
                    nr_sym_mask |= 1 << nr_sc;
                }
            }
            mask.symbol_masks[nr_sym] = nr_sym_mask;
        }
    }
}

/// O-RAN C-Plane Section Type 1 Dynamic Spectrum Sharing (DSS) Section Codec.
/// Encodes and parses Section Extension 5 / 9 DSS Puncture Descriptors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OranDssSectionCodec;

impl OranDssSectionCodec {
    /// Serializes DSS puncture configuration into O-RAN C-Plane Section Extension 5 payload.
    /// Format (O-RAN.WG4.CUS.0 §7.5.3.5):
    /// - `ext_type` (1 byte = 5)
    /// - `ext_len` (1 byte = length in 32-bit words)
    /// - `cell_id` (2 bytes)
    /// - `num_ports` (1 byte: 1, 2, 4)
    /// - `v_shift` (1 byte: 0..5)
    /// - `start_prb` (2 bytes)
    /// - `num_prb` (2 bytes)
    pub fn serialize_dss_extension(config: &LteCrsConfig, start_prb: u16, num_prb: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.push(5); // Section Extension 5 (DSS / CRS rate matching)
        buf.push(3); // 3 32-bit words (12 bytes total)
        buf.extend_from_slice(&config.cell_id.to_be_bytes());
        buf.push(config.antenna_ports as u8);
        buf.push(config.v_shift);
        buf.extend_from_slice(&start_prb.to_be_bytes());
        buf.extend_from_slice(&num_prb.to_be_bytes());
        buf.push(0); // Reserved padding
        buf.push(0);
        buf
    }

    /// Parses an O-RAN C-Plane Section Extension 5 buffer.
    pub fn parse_dss_extension(buf: &[u8]) -> Result<(LteCrsConfig, u16, u16), DssError> {
        if buf.len() < 12 {
            return Err(DssError::BufferTooShort {
                need: 12,
                got: buf.len(),
            });
        }
        let ext_type = buf[0];
        if ext_type != 5 {
            return Err(DssError::InvalidExtensionLength(buf.len()));
        }
        let ext_words = buf[1] as usize;
        if ext_words * 4 > buf.len() {
            return Err(DssError::BufferTooShort {
                need: ext_words * 4,
                got: buf.len(),
            });
        }

        let cell_id = u16::from_be_bytes([buf[2], buf[3]]);
        let ports_raw = buf[4];
        let ports = match ports_raw {
            1 => LteAntennaPorts::OnePort,
            2 => LteAntennaPorts::TwoPorts,
            4 => LteAntennaPorts::FourPorts,
            _ => return Err(DssError::InvalidCellId(cell_id)),
        };

        let start_prb = u16::from_be_bytes([buf[6], buf[7]]);
        let num_prb = u16::from_be_bytes([buf[8], buf[9]]);

        let config = LteCrsConfig::new(
            cell_id,
            num_prb,
            ports,
            LteCyclicPrefix::Normal,
            0, // Default no MBSFN
        )?;

        Ok((config, start_prb, num_prb))
    }
}

/// DSS Spectral Efficiency and Throughput Degradation Capacity Metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct DssCapacityMetrics {
    pub raw_res_per_prb: usize,
    pub punctured_res_per_prb: usize,
    pub usable_pdsch_res_per_prb: usize,
    pub crs_overhead_pct: f64,
    pub nominal_code_rate: f64,
    pub effective_code_rate: f64,
    pub capacity_loss_pct: f64,
}

impl DssCapacityMetrics {
    /// Computes capacity impact and effective code rate scaling on DSS shared carrier.
    pub fn compute(mask: &CrsPunctureMask, nominal_code_rate: f64) -> Result<Self, DssError> {
        let raw_res_per_prb = SYMBOLS_PER_SLOT_NORMAL_CP * SUBCARRIERS_PER_PRB;
        let punctured_res_per_prb = mask.total_punctured_res();
        let usable_pdsch_res_per_prb = mask.available_pdsch_res();

        let crs_overhead_pct = (punctured_res_per_prb as f64 / raw_res_per_prb as f64) * 100.0;
        let capacity_loss_pct = crs_overhead_pct;

        // Effective code rate scaled up due to loss of punctured data REs:
        // R_eff = R_nom * (N_raw / N_usable)
        let effective_code_rate = if usable_pdsch_res_per_prb > 0 {
            nominal_code_rate * (raw_res_per_prb as f64 / usable_pdsch_res_per_prb as f64)
        } else {
            1.0
        };

        if effective_code_rate > MAX_EFFECTIVE_CODE_RATE {
            return Err(DssError::EffectiveCodeRateExceeded {
                rate: (effective_code_rate * 1000.0) as u32,
                threshold: (MAX_EFFECTIVE_CODE_RATE * 1000.0) as u32,
            });
        }

        Ok(Self {
            raw_res_per_prb,
            punctured_res_per_prb,
            usable_pdsch_res_per_prb,
            crs_overhead_pct,
            nominal_code_rate,
            effective_code_rate,
            capacity_loss_pct,
        })
    }
}
