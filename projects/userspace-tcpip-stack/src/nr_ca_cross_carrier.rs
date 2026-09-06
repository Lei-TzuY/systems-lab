//! 3GPP Rel-17 5G NR Carrier Aggregation (CA) & Cross-Carrier Scheduling Engine.
//!
//! Conforms to:
//! - 3GPP TS 38.300 Rel-17 §5.5 & §6.4: Carrier Aggregation architecture, PCell, SCell, and PUCCH SCell.
//! - 3GPP TS 38.212 Rel-17 §7.3.1: DCI formats with 3-bit Carrier Indicator Field (CIF).
//! - 3GPP TS 38.213 Rel-17 §9.2 & §10: Dual PUCCH groups and cross-carrier scheduling timing.
//! - 3GPP TS 38.321 Rel-17 §5.9 & §6.1.3.10: SCell Activation/Deactivation MAC Control Elements
//!   (1-octet format for SCells 1..7 and 4-octet format for SCells 1..31), sCellDeactivationTimer.
//! - 3GPP TS 38.331 Rel-17: CrossCarrierSchedulingConfig, serving cell configuration.
//!
//! Pure standard Rust (`std` / `core` only) with zero external dependencies.

use std::collections::HashMap;

/// Standard MAC LCID for 1-octet SCell Activation/Deactivation MAC CE (TS 38.321 §6.2.1).
pub const LCID_SCELL_ACT_DEACT_1_OCTET: u8 = 62;

/// Standard MAC LCID for 4-octet SCell Activation/Deactivation MAC CE (TS 38.321 §6.2.1).
pub const LCID_SCELL_ACT_DEACT_4_OCTET: u8 = 61;

// ===========================================================================
// 1. Serving Cell Identifiers & Numerology
// ===========================================================================

/// 5G NR Subcarrier Spacing (SCS) and numerology mu (TS 38.211 §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NrSubcarrierSpacing {
    /// 15 kHz (mu = 0, slot = 1ms)
    Scs15kHz,
    /// 30 kHz (mu = 1, slot = 0.5ms)
    Scs30kHz,
    /// 60 kHz (mu = 2, slot = 0.25ms)
    Scs60kHz,
    /// 120 kHz (mu = 3, slot = 0.125ms)
    Scs120kHz,
}

impl NrSubcarrierSpacing {
    pub fn mu(&self) -> u8 {
        match self {
            NrSubcarrierSpacing::Scs15kHz => 0,
            NrSubcarrierSpacing::Scs30kHz => 1,
            NrSubcarrierSpacing::Scs60kHz => 2,
            NrSubcarrierSpacing::Scs120kHz => 3,
        }
    }

    pub fn slots_per_subframe(&self) -> u32 {
        1 << self.mu()
    }

    pub fn slot_duration_us(&self) -> u32 {
        1000 >> self.mu()
    }
}

/// PUCCH Group Identifier (TS 38.331 / TS 38.213 §9.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PucchGroupId {
    /// Primary PUCCH group: HARQ feedback routed to PCell
    PrimaryGroup,
    /// Secondary PUCCH group: HARQ feedback routed to designated PUCCH SCell
    SecondaryGroup,
}

/// Configuration of a Serving Cell in Carrier Aggregation (TS 38.331).
#[derive(Debug, Clone, PartialEq)]
pub struct CaServingCellConfig {
    /// Serving cell index (0 = PCell, 1..31 = SCell)
    pub serv_cell_index: u8,
    pub pci: u16,
    pub dl_carrier_freq_mhz: f64,
    pub ul_carrier_freq_mhz: Option<f64>,
    pub scs: NrSubcarrierSpacing,
    pub bandwidth_prb: u16,
    pub is_pcell: bool,
    pub is_pucch_scell: bool,
    pub pucch_group: PucchGroupId,
    /// If scheduling_cell_id == serv_cell_index => Self-scheduling.
    /// Otherwise => Cross-carrier scheduling by scheduling_cell_id.
    pub scheduling_cell_id: u8,
    pub cif_presence: bool,
    /// 3-bit CIF value (0..7)
    pub cif_val: u8,
}

impl CaServingCellConfig {
    pub fn is_cross_carrier_scheduled(&self) -> bool {
        self.scheduling_cell_id != self.serv_cell_index
    }
}

// ===========================================================================
// 2. Cross-Carrier Scheduling Engine & DCI with CIF
// ===========================================================================

/// Downlink or Uplink Grant with 3-bit Carrier Indicator Field (CIF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossCarrierGrant {
    pub scheduling_cell_index: u8,
    pub scheduled_cell_index: u8,
    /// 3-bit Carrier Indicator Field (0..7)
    pub cif: u8,
    pub is_downlink: bool,
    /// Scheduling slot offset: K0 for PDSCH, K2 for PUSCH
    pub k_offset_slots: u8,
    pub prb_start: u16,
    pub prb_count: u16,
    pub mcs: u8,
    pub harq_process_id: u8,
    pub ndi: bool,
    pub rv: u8,
}

/// Cross-Carrier Scheduling and Timing Coordinator.
pub struct CrossCarrierScheduler;

impl CrossCarrierScheduler {
    /// Calculates target transmission slot on the scheduled cell when scheduling cell
    /// transmits PDCCH at `scheduling_slot`, handling mixed numerologies (TS 38.214).
    pub fn calculate_target_slot(
        scheduling_scs: NrSubcarrierSpacing,
        scheduled_scs: NrSubcarrierSpacing,
        scheduling_slot: u32,
        k_offset: u8,
    ) -> u32 {
        let mu_sched = scheduling_scs.mu() as i32;
        let mu_target = scheduled_scs.mu() as i32;
        let delta_mu = mu_target - mu_sched;

        let base_target_slot = if delta_mu >= 0 {
            scheduling_slot * (1 << delta_mu)
        } else {
            scheduling_slot / (1 << (-delta_mu))
        };

        base_target_slot + (k_offset as u32)
    }

    /// Validates and constructs a cross-carrier grant.
    pub fn create_cross_carrier_grant(
        scheduling_cell: &CaServingCellConfig,
        scheduled_cell: &CaServingCellConfig,
        is_downlink: bool,
        k_offset: u8,
        prb_start: u16,
        prb_count: u16,
        mcs: u8,
        harq_pid: u8,
    ) -> Result<CrossCarrierGrant, String> {
        if !scheduled_cell.cif_presence && scheduled_cell.is_cross_carrier_scheduled() {
            return Err("CIF must be configured for cross-carrier scheduled cell".to_string());
        }
        if scheduled_cell.scheduling_cell_id != scheduling_cell.serv_cell_index {
            return Err(format!(
                "Cell {} is scheduled by Cell {}, not Cell {}",
                scheduled_cell.serv_cell_index,
                scheduled_cell.scheduling_cell_id,
                scheduling_cell.serv_cell_index
            ));
        }
        if prb_start + prb_count > scheduled_cell.bandwidth_prb {
            return Err("PRB allocation exceeds target cell bandwidth".to_string());
        }

        Ok(CrossCarrierGrant {
            scheduling_cell_index: scheduling_cell.serv_cell_index,
            scheduled_cell_index: scheduled_cell.serv_cell_index,
            cif: scheduled_cell.cif_val,
            is_downlink,
            k_offset_slots: k_offset,
            prb_start,
            prb_count,
            mcs,
            harq_process_id: harq_pid,
            ndi: true,
            rv: 0,
        })
    }
}

// ===========================================================================
// 3. SCell State Machine & Deactivation Timer
// ===========================================================================

/// Operational state of an SCell (TS 38.321 §5.9 & TS 38.331).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScellState {
    /// Fully active: PDCCH monitoring, CSI reporting, SRS active.
    Active,
    /// Dormant state: CSI reporting active, but no PDCCH monitoring (Rel-17 fast resume).
    Dormant,
    /// Deactivated: all RF chains and monitoring suspended, HARQ buffers flushed.
    Deactivated,
}

/// SCell entity managing lifecycle and deactivation timer.
#[derive(Debug)]
pub struct ScellManager {
    pub config: CaServingCellConfig,
    pub state: ScellState,
    /// Configured sCellDeactivationTimer in subframes (1ms each)
    pub deactivation_timer_subframes: u32,
    /// Running countdown of deactivation timer
    pub timer_countdown: u32,
    /// Flags indicating lower layer activity
    pub pdcch_monitoring_active: bool,
    pub csi_reporting_active: bool,
    pub srs_transmission_active: bool,
    pub harq_buffers_flushed: bool,
}

impl ScellManager {
    pub fn new(config: CaServingCellConfig, deactivation_timer_subframes: u32) -> Self {
        Self {
            config,
            state: ScellState::Deactivated,
            deactivation_timer_subframes,
            timer_countdown: 0,
            pdcch_monitoring_active: false,
            csi_reporting_active: false,
            srs_transmission_active: false,
            harq_buffers_flushed: true,
        }
    }

    /// Activates SCell upon receiving MAC CE or RRC configuration.
    pub fn activate(&mut self) {
        self.state = ScellState::Active;
        self.timer_countdown = self.deactivation_timer_subframes;
        self.pdcch_monitoring_active = true;
        self.csi_reporting_active = true;
        self.srs_transmission_active = true;
        self.harq_buffers_flushed = false;
    }

    /// Transitions to dormant state (Rel-17 low-latency BWP).
    pub fn set_dormant(&mut self) {
        self.state = ScellState::Dormant;
        self.timer_countdown = 0; // sCellDeactivationTimer stopped in dormant BWP per TS 38.321 §5.9
        self.pdcch_monitoring_active = false; // No PDCCH monitoring in dormant state
        self.csi_reporting_active = true; // CSI reporting maintained
        self.srs_transmission_active = false;
    }

    /// Alias for set_dormant per 3GPP dormant BWP transition.
    pub fn transition_to_dormant(&mut self) {
        self.set_dormant();
    }

    /// Deactivates SCell upon timer expiration or MAC CE command.
    pub fn deactivate(&mut self) {
        self.state = ScellState::Deactivated;
        self.timer_countdown = 0;
        self.pdcch_monitoring_active = false;
        self.csi_reporting_active = false;
        self.srs_transmission_active = false;
        self.harq_buffers_flushed = true; // Flush all HARQ buffers per TS 38.321 §5.9
    }

    /// Refreshes the sCellDeactivationTimer upon grant or activity on this SCell.
    pub fn restart_deactivation_timer(&mut self) {
        if self.state == ScellState::Active || self.state == ScellState::Dormant {
            self.timer_countdown = self.deactivation_timer_subframes;
        }
    }

    /// Advances subframe clock by 1ms (1 subframe).
    /// Returns true if the SCell deactivated due to timer expiration during this tick.
    pub fn step_subframe(&mut self) -> bool {
        if self.state != ScellState::Deactivated && self.timer_countdown > 0 {
            self.timer_countdown -= 1;
            if self.timer_countdown == 0 {
                self.deactivate();
                return true;
            }
        }
        false
    }

    /// Advances subframe clock by given number of subframes (ms).
    /// Returns true if the SCell deactivated during this elapsed time.
    pub fn tick_subframes(&mut self, subframes: u32) -> bool {
        let mut deactivated = false;
        for _ in 0..subframes {
            if self.step_subframe() {
                deactivated = true;
            }
        }
        deactivated
    }
}

// ===========================================================================
// 4. SCell Activation / Deactivation MAC Control Elements
// ===========================================================================

/// SCell Activation/Deactivation MAC CE Encoder and Decoder (TS 38.321 §6.1.3.10).
pub struct ScellMacCeCodec;

impl ScellMacCeCodec {
    /// Helper to format 1-octet MAC CE using a list of active SCell indices (1..7).
    pub fn encode_one_octet_indices(active_cell_indices: &[u8]) -> [u8; 1] {
        let mut flags = [false; 8];
        for &idx in active_cell_indices {
            if (1..=7).contains(&idx) {
                flags[idx as usize] = true;
            }
        }
        Self::encode_one_octet(&flags)
    }

    /// Formats a 1-Octet SCell Activation/Deactivation MAC CE (LCID 62).
    /// Fields: C7, C6, C5, C4, C3, C2, C1, R (bit 0 = Reserved).
    /// `activation_flags[i]` corresponds to ServCellIndex `i` (1..7).
    pub fn encode_one_octet(activation_flags: &[bool; 8]) -> [u8; 1] {
        let mut byte = 0u8;
        for i in 1..=7 {
            if activation_flags[i] {
                byte |= 1 << i;
            }
        }
        // Bit 0 is reserved (0)
        [byte]
    }

    /// Parses a 1-Octet SCell Activation/Deactivation MAC CE (LCID 62).
    /// Returns a boolean array where index `i` indicates activation status of ServCellIndex `i`.
    pub fn decode_one_octet(raw: &[u8]) -> Result<[bool; 8], String> {
        if raw.len() != 1 {
            return Err("1-Octet SCell MAC CE payload must be exactly 1 byte".to_string());
        }
        let b = raw[0];
        let mut flags = [false; 8];
        for i in 1..=7 {
            flags[i] = (b & (1 << i)) != 0;
        }
        Ok(flags)
    }

    /// Helper to format 4-octet MAC CE using a list of active SCell indices (1..31).
    pub fn encode_four_octet_indices(active_cell_indices: &[u8]) -> [u8; 4] {
        let mut flags = [false; 32];
        for &idx in active_cell_indices {
            if (1..=31).contains(&idx) {
                flags[idx as usize] = true;
            }
        }
        Self::encode_four_octet(&flags)
    }

    /// Formats a 4-Octet SCell Activation/Deactivation MAC CE (LCID 61).
    /// Covers ServCellIndex 1..31 (31 bits + 1 reserved bit).
    pub fn encode_four_octet(activation_flags: &[bool; 32]) -> [u8; 4] {
        let mut val = 0u32;
        for i in 1..=31 {
            if activation_flags[i] {
                val |= 1 << i;
            }
        }
        // Val in little-endian wire representation per 3GPP bit ordering
        val.to_le_bytes()
    }

    /// Parses a 4-Octet SCell Activation/Deactivation MAC CE (LCID 61).
    pub fn decode_four_octet(raw: &[u8]) -> Result<[bool; 32], String> {
        if raw.len() != 4 {
            return Err("4-Octet SCell MAC CE payload must be exactly 4 bytes".to_string());
        }
        let val = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let mut flags = [false; 32];
        for i in 1..=31 {
            flags[i] = (val & (1 << i)) != 0;
        }
        Ok(flags)
    }
}

// ===========================================================================
// 5. Multi-Carrier HARQ-ACK Codebook Multiplexer
// ===========================================================================

/// Individual HARQ feedback bit for a serving cell transport block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellHarqFeedback {
    pub serv_cell_index: u8,
    pub harq_process_id: u8,
    pub is_ack: bool,
}

/// Multiplexed HARQ payloads routed to respective PUCCH groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiplexedPucchReport {
    /// HARQ payload for Primary PUCCH Group (transmitted on PCell PUCCH)
    pub primary_group_harq_bits: Vec<bool>,
    /// HARQ payload for Secondary PUCCH Group (transmitted on PUCCH SCell PUCCH)
    pub secondary_group_harq_bits: Vec<bool>,
}

/// Carrier Aggregation HARQ Multiplexer.
pub struct CaHarqMultiplexer;

impl CaHarqMultiplexer {
    /// Multiplexes HARQ feedback from multiple carriers into Primary and Secondary PUCCH groups.
    pub fn multiplex_feedback(
        feedbacks: &[CellHarqFeedback],
        cell_configs: &HashMap<u8, CaServingCellConfig>,
    ) -> MultiplexedPucchReport {
        let mut primary_bits = Vec::new();
        let mut secondary_bits = Vec::new();

        for fb in feedbacks {
            let config = cell_configs.get(&fb.serv_cell_index);
            let pucch_group = config
                .map(|c| c.pucch_group)
                .unwrap_or(PucchGroupId::PrimaryGroup);

            match pucch_group {
                PucchGroupId::PrimaryGroup => primary_bits.push(fb.is_ack),
                PucchGroupId::SecondaryGroup => secondary_bits.push(fb.is_ack),
            }
        }

        MultiplexedPucchReport {
            primary_group_harq_bits: primary_bits,
            secondary_group_harq_bits: secondary_bits,
        }
    }
}
