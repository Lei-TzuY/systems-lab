//! 3GPP Rel-17 5G NR Unified TCI State & Multi-TRP Beam Management Engine
//!
//! Conforms to:
//! - 3GPP TS 38.214 §5.1.5 (Antenna ports quasi co-location, unified TCI state)
//! - 3GPP TS 38.214 §6.1.5 (Uplink spatial relation / unified UL TCI)
//! - 3GPP TS 38.213 §10 (UE procedure for determining physical downlink control channel assignment)
//! - 3GPP TS 38.321 §6.1.3 (Rel-17 Enhanced MAC CE for Unified TCI state activation/deactivation)
//! - 3GPP TS 38.331 (RRC Information Elements: `TCI-State`, `unifiedTCI-State`)
//!
//! Pure standard Rust (`std`/`core` only), zero external dependencies.

use std::fmt;

/// Maximum number of configurable TCI states per BWP/CC.
pub const MAX_TCI_STATES: usize = 128;
/// Maximum number of dynamic DCI codepoints (typically 3 bits -> 8 codepoints).
pub const MAX_DCI_CODEPOINTS: usize = 8;
/// Maximum number of component carriers supported in CC list bitmap.
pub const MAX_CARRIERS: usize = 8;
/// Default radio frame cycle (SFN 0..1023).
pub const MAX_SFN: u16 = 1024;

/// Quasi-Co-Location (QCL) Types defined in 3GPP TS 38.214 §5.1.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QclType {
    /// QCL-TypeA: Doppler shift, Doppler spread, average delay, delay spread.
    TypeA,
    /// QCL-TypeB: Doppler shift, Doppler spread.
    TypeB,
    /// QCL-TypeC: Doppler shift, average delay.
    TypeC,
    /// QCL-TypeD: Spatial Rx parameter (Spatial filter / beam direction).
    TypeD,
}

impl fmt::Display for QclType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QclType::TypeA => write!(f, "QCL-TypeA"),
            QclType::TypeB => write!(f, "QCL-TypeB"),
            QclType::TypeC => write!(f, "QCL-TypeC"),
            QclType::TypeD => write!(f, "QCL-TypeD"),
        }
    }
}

/// Reference signal source for QCL association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceSignal {
    /// Synchronization Signal / Physical Broadcast Channel (SS/PBCH) block (0..63).
    Ssb { ssb_index: u8 },
    /// CSI Reference Signal (0..127).
    CsiRs { resource_id: u8, is_periodic: bool },
    /// Sounding Reference Signal for UL spatial relation (0..63).
    Srs { resource_id: u8 },
}

/// QCL Information element associating a reference signal with a QCL type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QclInfo {
    /// Reference signal source.
    pub reference_signal: ReferenceSignal,
    /// QCL property type.
    pub qcl_type: QclType,
    /// Serving cell ID of the RS (0..31).
    pub cell_id: u8,
    /// Bandwidth Part ID of the RS (0..4).
    pub bwp_id: u8,
}

impl QclInfo {
    /// Create a new QCL Info element.
    pub fn new(
        reference_signal: ReferenceSignal,
        qcl_type: QclType,
        cell_id: u8,
        bwp_id: u8,
    ) -> Self {
        Self {
            reference_signal,
            qcl_type,
            cell_id,
            bwp_id,
        }
    }
}

/// Rel-17 Unified TCI Direction Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TciDirectionMode {
    /// Joint TCI state: Common beam applied to both DL reception and UL transmission.
    Joint,
    /// Separate DL TCI state: Applied specifically to DL channels (PDCCH, PDSCH, CSI-RS).
    SeparateDl,
    /// Separate UL TCI state: Applied specifically to UL channels (PUCCH, PUSCH, SRS).
    SeparateUl,
}

/// Transmission/Reception Point (TRP) identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrpId {
    /// Primary Transmission Point (CORESET pool index 0).
    Trp0,
    /// Secondary Transmission Point (CORESET pool index 1).
    Trp1,
}

impl fmt::Display for TrpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrpId::Trp0 => write!(f, "TRP-0"),
            TrpId::Trp1 => write!(f, "TRP-1"),
        }
    }
}

/// Rel-17 Unified Transmission Configuration Indicator (TCI) State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedTciState {
    /// Unique TCI state ID (0..127).
    pub tci_state_id: u8,
    /// Direction mode (Joint, Separate DL, or Separate UL).
    pub direction_mode: TciDirectionMode,
    /// Primary QCL source (typically QCL-TypeA/B/C for Doppler/Delay).
    pub qcl_source_1: QclInfo,
    /// Secondary QCL source (typically QCL-TypeD for Spatial Rx beam parameter).
    pub qcl_source_2: Option<QclInfo>,
    /// Associated serving cell ID.
    pub serving_cell_id: u8,
    /// Associated BWP ID.
    pub bwp_id: u8,
    /// Physical Cell ID (PCI) for inter-cell multi-TRP operation.
    pub pci: Option<u16>,
}

impl UnifiedTciState {
    /// Construct a new unified TCI state.
    pub fn new(
        tci_state_id: u8,
        direction_mode: TciDirectionMode,
        qcl_source_1: QclInfo,
        qcl_source_2: Option<QclInfo>,
        serving_cell_id: u8,
        bwp_id: u8,
    ) -> Result<Self, &'static str> {
        if tci_state_id >= MAX_TCI_STATES as u8 {
            return Err("TCI state ID exceeds maximum allowed (127)");
        }
        if serving_cell_id > 31 {
            return Err("Serving cell ID must be <= 31");
        }
        if bwp_id > 4 {
            return Err("BWP ID must be <= 4");
        }

        // Validate QCL combinations per TS 38.214 §5.1.5:
        // Source 1 should not be TypeD if Source 2 is present.
        if let Some(src2) = qcl_source_2 {
            if qcl_source_1.qcl_type == QclType::TypeD && src2.qcl_type == QclType::TypeD {
                return Err("Cannot have both QCL sources configured as QCL-TypeD");
            }
        }

        Ok(Self {
            tci_state_id,
            direction_mode,
            qcl_source_1,
            qcl_source_2,
            serving_cell_id,
            bwp_id,
            pci: None,
        })
    }

    /// Set an optional Physical Cell ID for inter-cell multi-TRP.
    pub fn with_pci(mut self, pci: u16) -> Self {
        self.pci = Some(pci);
        self
    }

    /// Check if this state provides a spatial Rx beam (QCL-TypeD).
    pub fn has_spatial_beam(&self) -> bool {
        self.qcl_source_1.qcl_type == QclType::TypeD
            || self
                .qcl_source_2
                .map(|s| s.qcl_type == QclType::TypeD)
                .unwrap_or(false)
    }
}

/// Rel-17 Unified TCI State MAC Control Element (MAC CE).
///
/// Encodes activation/deactivation of unified TCI states for a serving cell
/// and across component carriers (TS 38.321 §6.1.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedTciMacCe {
    /// Serving Cell ID (5 bits: 0..31).
    pub serving_cell_id: u8,
    /// BWP ID (2 bits: 0..3).
    pub bwp_id: u8,
    /// Direction mode.
    pub direction_mode: TciDirectionMode,
    /// Primary TCI state ID for TRP 0.
    pub tci_state_id_trp0: u8,
    /// Secondary TCI state ID for TRP 1 (if dual-TRP multi-beam activated).
    pub tci_state_id_trp1: Option<u8>,
    /// Bitmap of Component Carriers to which this TCI activation applies (8 bits).
    pub cc_list_bitmap: u8,
}

impl UnifiedTciMacCe {
    /// Create a new Unified TCI MAC CE.
    pub fn new(
        serving_cell_id: u8,
        bwp_id: u8,
        direction_mode: TciDirectionMode,
        tci_state_id_trp0: u8,
        tci_state_id_trp1: Option<u8>,
        cc_list_bitmap: u8,
    ) -> Result<Self, &'static str> {
        if serving_cell_id > 31 {
            return Err("Serving cell ID must be <= 31");
        }
        if bwp_id > 3 {
            return Err("BWP ID must be <= 3 for MAC CE encoding");
        }
        if tci_state_id_trp0 >= MAX_TCI_STATES as u8 {
            return Err("TCI State ID TRP0 must be < 128");
        }
        if let Some(trp1_id) = tci_state_id_trp1 {
            if trp1_id >= MAX_TCI_STATES as u8 {
                return Err("TCI State ID TRP1 must be < 128");
            }
        }
        Ok(Self {
            serving_cell_id,
            bwp_id,
            direction_mode,
            tci_state_id_trp0,
            tci_state_id_trp1,
            cc_list_bitmap,
        })
    }

    /// Serialize this MAC CE into standard binary wire bytes.
    ///
    /// Byte format:
    /// Byte 0: [R (1b)][Serving Cell ID (5b)][BWP ID (2b)]
    /// Byte 1: [Mode (2b: 00=Joint, 01=SepDL, 10=SepUL)][Dual-TRP Flag (1b)][Reserved (5b)]
    /// Byte 2: [R (1b)][TCI State ID TRP0 (7b)]
    /// Byte 3 (Optional if Dual-TRP): [R (1b)][TCI State ID TRP1 (7b)]
    /// Final Byte: [CC List Bitmap (8b)]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Byte 0
        let b0 = ((self.serving_cell_id & 0x1F) << 2) | (self.bwp_id & 0x03);
        buf.push(b0);

        // Byte 1
        let mode_bits = match self.direction_mode {
            TciDirectionMode::Joint => 0b00,
            TciDirectionMode::SeparateDl => 0b01,
            TciDirectionMode::SeparateUl => 0b10,
        };
        let dual_trp_bit = if self.tci_state_id_trp1.is_some() {
            1
        } else {
            0
        };
        let b1 = (mode_bits << 6) | (dual_trp_bit << 5);
        buf.push(b1);

        // Byte 2: TRP0 TCI ID
        let b2 = self.tci_state_id_trp0 & 0x7F;
        buf.push(b2);

        // Byte 3: TRP1 TCI ID (if present)
        if let Some(trp1_id) = self.tci_state_id_trp1 {
            let b3 = trp1_id & 0x7F;
            buf.push(b3);
        }

        // Final Byte: CC List Bitmap
        buf.push(self.cc_list_bitmap);
        buf
    }

    /// Parse a Unified TCI MAC CE from wire bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 4 {
            return Err("Unified TCI MAC CE payload too short (minimum 4 bytes required)");
        }

        let b0 = bytes[0];
        let serving_cell_id = (b0 >> 2) & 0x1F;
        let bwp_id = b0 & 0x03;

        let b1 = bytes[1];
        let mode_bits = (b1 >> 6) & 0x03;
        let direction_mode = match mode_bits {
            0b00 => TciDirectionMode::Joint,
            0b01 => TciDirectionMode::SeparateDl,
            0b10 => TciDirectionMode::SeparateUl,
            _ => return Err("Invalid TCI direction mode in MAC CE"),
        };
        let is_dual_trp = ((b1 >> 5) & 0x01) == 1;

        let tci_state_id_trp0 = bytes[2] & 0x7F;

        let (tci_state_id_trp1, cc_list_bitmap) = if is_dual_trp {
            if bytes.len() < 5 {
                return Err("Dual-TRP MAC CE requires at least 5 bytes");
            }
            let trp1_id = bytes[3] & 0x7F;
            let cc_bitmap = bytes[4];
            (Some(trp1_id), cc_bitmap)
        } else {
            let cc_bitmap = bytes[3];
            (None, cc_bitmap)
        };

        Ok(Self {
            serving_cell_id,
            bwp_id,
            direction_mode,
            tci_state_id_trp0,
            tci_state_id_trp1,
            cc_list_bitmap,
        })
    }
}

/// Multi-TRP Transmission and Multiplexing Scheme (TS 38.214 §5.1.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MTrpTransmissionMode {
    /// Space-Division Multiplexing (SDM): Independent spatial layers across TRP 0 and TRP 1.
    SpaceDivisionMultiplexing,
    /// Time-Division Multiplexing (TDM): Alternating slots or OFDM symbol allocations.
    TimeDivisionMultiplexing,
    /// Frequency-Division Multiplexing (FDM): Split PRB allocation across TRPs.
    FrequencyDivisionMultiplexing,
    /// Single-Frequency Network (SFN): Simultaneous joint transmission on identical PRBs.
    SingleFrequencyNetwork,
}

/// Multi-TRP Channel condition estimate per TRP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrpChannelCondition {
    /// Reference Signal Received Power in dBm.
    pub rsrp_dbm: f32,
    /// Signal to Interference plus Noise Ratio in dB.
    pub sinr_db: f32,
    /// Estimated path loss in dB.
    pub path_loss_db: f32,
}

impl Default for TrpChannelCondition {
    fn default() -> Self {
        Self {
            rsrp_dbm: -90.0,
            sinr_db: 15.0,
            path_loss_db: 80.0,
        }
    }
}

/// Active Beam Pair Link for multi-TRP operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveBeamSet {
    /// Active TCI state for TRP 0.
    pub trp0_tci: Option<u8>,
    /// Active TCI state for TRP 1.
    pub trp1_tci: Option<u8>,
}

/// Beam switch state tracking Beam Application Time ($k_{\text{BAT}}$).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeamSwitchState {
    /// Beam configuration is steady-state.
    Steady,
    /// Beam switch was triggered at `trigger_slot` and pending until `applied_slot`.
    Pending {
        trigger_sfn: u16,
        trigger_slot: u8,
        target_trp0: u8,
        target_trp1: Option<u8>,
        remaining_slots: u8,
    },
}

/// Per-TRP Beam Failure Detection (BFD) tracker (TS 38.213 §6).
#[derive(Debug, Clone, PartialEq)]
pub struct TrpBfdState {
    /// Associated TRP.
    pub trp_id: TrpId,
    /// Consecutive Beam Failure Instance (BFI) counter.
    pub bfi_count: u32,
    /// Maximum BFI threshold before declaring beam failure.
    pub bfi_threshold: u32,
    /// Flag indicating whether this TRP has declared beam failure.
    pub is_failed: bool,
    /// Candidate beam identifier identified for recovery.
    pub candidate_beam_tci: Option<u8>,
}

impl TrpBfdState {
    /// Create a new BFD tracker for a TRP.
    pub fn new(trp_id: TrpId, bfi_threshold: u32) -> Self {
        Self {
            trp_id,
            bfi_count: 0,
            bfi_threshold,
            is_failed: false,
            candidate_beam_tci: None,
        }
    }

    /// Record a Beam Failure Instance (BFI) indication. Returns true if failure newly triggered.
    pub fn record_bfi(&mut self) -> bool {
        if self.is_failed {
            return false;
        }
        self.bfi_count += 1;
        if self.bfi_count >= self.bfi_threshold {
            self.is_failed = true;
            true
        } else {
            false
        }
    }

    /// Reset BFI counter on successful beam sync.
    pub fn reset(&mut self) {
        self.bfi_count = 0;
        self.is_failed = false;
        self.candidate_beam_tci = None;
    }
}

/// Engine managing Rel-17 Unified TCI States, DCI Codepoints, and Multi-TRP Beams.
#[derive(Debug, Clone)]
pub struct UnifiedTciEngine {
    /// Configured RRC TCI states (0..127).
    configured_states: Vec<Option<UnifiedTciState>>,
    /// Activated dynamic DCI codepoints (up to 8).
    dci_codepoint_table: [Option<(u8, Option<u8>)>; MAX_DCI_CODEPOINTS],
    /// Currently active beams.
    active_beams: ActiveBeamSet,
    /// Beam switch transition state machine.
    beam_switch_state: BeamSwitchState,
    /// Beam Application Time ($k_{\text{BAT}}$) in slots.
    k_bat_slots: u8,
    /// Current System Frame Number (0..1023).
    current_sfn: u16,
    /// Current Slot index within frame (e.g. 0..19 for 30 kHz SCS).
    current_slot: u8,
    /// Total slots per frame (e.g., 20 for 30 kHz numerology).
    slots_per_frame: u8,
    /// TRP 0 Channel Condition.
    pub trp0_channel: TrpChannelCondition,
    /// TRP 1 Channel Condition.
    pub trp1_channel: TrpChannelCondition,
    /// Per-TRP BFD trackers.
    pub trp0_bfd: TrpBfdState,
    pub trp1_bfd: TrpBfdState,
    /// Whether operating in autonomous single-TRP fallback mode due to one TRP beam failure.
    pub in_single_trp_fallback: bool,
}

impl UnifiedTciEngine {
    /// Construct a new Unified TCI Engine.
    pub fn new(k_bat_slots: u8, slots_per_frame: u8, bfi_threshold: u32) -> Self {
        let mut configured = Vec::with_capacity(MAX_TCI_STATES);
        for _ in 0..MAX_TCI_STATES {
            configured.push(None);
        }

        Self {
            configured_states: configured,
            dci_codepoint_table: [None; MAX_DCI_CODEPOINTS],
            active_beams: ActiveBeamSet {
                trp0_tci: None,
                trp1_tci: None,
            },
            beam_switch_state: BeamSwitchState::Steady,
            k_bat_slots: k_bat_slots.max(1),
            current_sfn: 0,
            current_slot: 0,
            slots_per_frame: slots_per_frame.max(10),
            trp0_channel: TrpChannelCondition::default(),
            trp1_channel: TrpChannelCondition::default(),
            trp0_bfd: TrpBfdState::new(TrpId::Trp0, bfi_threshold),
            trp1_bfd: TrpBfdState::new(TrpId::Trp1, bfi_threshold),
            in_single_trp_fallback: false,
        }
    }

    /// Add or update a configured Unified TCI State via RRC.
    pub fn configure_tci_state(&mut self, state: UnifiedTciState) -> Result<(), &'static str> {
        let id = state.tci_state_id as usize;
        if id >= MAX_TCI_STATES {
            return Err("TCI state ID out of range");
        }
        self.configured_states[id] = Some(state);
        Ok(())
    }

    /// Retrieve a configured TCI state by ID.
    pub fn get_tci_state(&self, id: u8) -> Option<&UnifiedTciState> {
        self.configured_states.get(id as usize)?.as_ref()
    }

    /// Process an activated Rel-17 Unified TCI MAC CE.
    /// Activates the specified TCI state(s) and updates the codepoint table.
    pub fn apply_mac_ce(&mut self, mac_ce: &UnifiedTciMacCe) -> Result<(), &'static str> {
        // Validate TRP 0 state existence
        if self.get_tci_state(mac_ce.tci_state_id_trp0).is_none() {
            return Err("MAC CE referenced unconfigured TCI state for TRP0");
        }
        // Validate TRP 1 state existence if present
        if let Some(trp1_id) = mac_ce.tci_state_id_trp1 {
            if self.get_tci_state(trp1_id).is_none() {
                return Err("MAC CE referenced unconfigured TCI state for TRP1");
            }
        }

        // Set default codepoint 0 to the newly activated MAC CE states
        self.dci_codepoint_table[0] = Some((mac_ce.tci_state_id_trp0, mac_ce.tci_state_id_trp1));

        // If no active beam is yet set, immediately apply it
        if self.active_beams.trp0_tci.is_none() {
            self.active_beams.trp0_tci = Some(mac_ce.tci_state_id_trp0);
            self.active_beams.trp1_tci = mac_ce.tci_state_id_trp1;
        }

        Ok(())
    }

    /// Map a dynamic DCI codepoint (0..7) to target TCI state(s).
    pub fn set_dci_codepoint_mapping(
        &mut self,
        codepoint: u8,
        trp0_tci: u8,
        trp1_tci: Option<u8>,
    ) -> Result<(), &'static str> {
        if codepoint as usize >= MAX_DCI_CODEPOINTS {
            return Err("Codepoint exceeds maximum of 8 (3 bits)");
        }
        if self.get_tci_state(trp0_tci).is_none() {
            return Err("TRP0 TCI state is not configured in RRC");
        }
        if let Some(t1) = trp1_tci {
            if self.get_tci_state(t1).is_none() {
                return Err("TRP1 TCI state is not configured in RRC");
            }
        }

        self.dci_codepoint_table[codepoint as usize] = Some((trp0_tci, trp1_tci));
        Ok(())
    }

    /// Receive a dynamic DCI beam indication at current (SFN, slot).
    /// Initiates the Beam Application Time ($k_{\text{BAT}}$) countdown.
    pub fn receive_dci_beam_indication(&mut self, codepoint: u8) -> Result<(), &'static str> {
        let entry = self
            .dci_codepoint_table
            .get(codepoint as usize)
            .and_then(|&opt| opt)
            .ok_or("Unassigned DCI codepoint received")?;

        let (target_trp0, target_trp1) = entry;

        self.beam_switch_state = BeamSwitchState::Pending {
            trigger_sfn: self.current_sfn,
            trigger_slot: self.current_slot,
            target_trp0,
            target_trp1,
            remaining_slots: self.k_bat_slots,
        };

        Ok(())
    }

    /// Advance simulation time by one slot.
    /// Decrements pending beam switch timers and applies target beams when $k_{\text{BAT}}$ expires.
    /// Returns `Some(ActiveBeamSet)` if a new beam set just took effect in this slot.
    pub fn advance_slot(&mut self) -> Option<ActiveBeamSet> {
        // Advance slot and SFN counters
        self.current_slot += 1;
        if self.current_slot >= self.slots_per_frame {
            self.current_slot = 0;
            self.current_sfn = (self.current_sfn + 1) % MAX_SFN;
        }

        // Process pending beam switch state
        let mut completed_switch = None;
        if let BeamSwitchState::Pending {
            trigger_sfn,
            trigger_slot,
            target_trp0,
            target_trp1,
            remaining_slots,
        } = self.beam_switch_state
        {
            if remaining_slots <= 1 {
                // Beam switch takes effect!
                self.active_beams.trp0_tci = Some(target_trp0);
                self.active_beams.trp1_tci = target_trp1;
                self.beam_switch_state = BeamSwitchState::Steady;
                completed_switch = Some(self.active_beams);
            } else {
                self.beam_switch_state = BeamSwitchState::Pending {
                    trigger_sfn,
                    trigger_slot,
                    target_trp0,
                    target_trp1,
                    remaining_slots: remaining_slots - 1,
                };
            }
        }

        completed_switch
    }

    /// Record a Beam Failure Instance (BFI) for the designated TRP.
    /// If failure is triggered on one TRP, enters autonomous single-TRP fallback mode.
    pub fn record_beam_failure_instance(&mut self, trp: TrpId) -> bool {
        let newly_failed = match trp {
            TrpId::Trp0 => self.trp0_bfd.record_bfi(),
            TrpId::Trp1 => self.trp1_bfd.record_bfi(),
        };

        if newly_failed {
            // Check fallback status
            if self.trp0_bfd.is_failed && !self.trp1_bfd.is_failed {
                // TRP0 failed, fallback to TRP1
                self.in_single_trp_fallback = true;
            } else if !self.trp0_bfd.is_failed && self.trp1_bfd.is_failed {
                // TRP1 failed, fallback to TRP0
                self.in_single_trp_fallback = true;
            }
        }

        newly_failed
    }

    /// Recover a failed TRP using an identified candidate beam.
    pub fn recover_trp_beam(&mut self, trp: TrpId, candidate_tci: u8) -> Result<(), &'static str> {
        if self.get_tci_state(candidate_tci).is_none() {
            return Err("Candidate TCI state is not configured");
        }

        match trp {
            TrpId::Trp0 => {
                self.trp0_bfd.reset();
                self.active_beams.trp0_tci = Some(candidate_tci);
            }
            TrpId::Trp1 => {
                self.trp1_bfd.reset();
                self.active_beams.trp1_tci = Some(candidate_tci);
            }
        }

        // If neither TRP is failed now, exit fallback
        if !self.trp0_bfd.is_failed && !self.trp1_bfd.is_failed {
            self.in_single_trp_fallback = false;
        }

        Ok(())
    }

    /// Compute combined effective SINR (in dB) and Shannon capacity (in Mbps)
    /// under the chosen multi-TRP transmission mode and channel conditions.
    pub fn compute_link_metrics(
        &self,
        bandwidth_mhz: f32,
        mode: MTrpTransmissionMode,
    ) -> (f32, f32) {
        let p0_linear = 10.0f32.powf(self.trp0_channel.sinr_db / 10.0);
        let p1_linear = 10.0f32.powf(self.trp1_channel.sinr_db / 10.0);

        // If in fallback mode or single-TRP active, only use healthy TRP
        let (effective_sinr_db, capacity_mbps) = if self.in_single_trp_fallback {
            if !self.trp0_bfd.is_failed {
                let sinr = self.trp0_channel.sinr_db;
                let cap = bandwidth_mhz * (1.0 + p0_linear).log2();
                (sinr, cap)
            } else if !self.trp1_bfd.is_failed {
                let sinr = self.trp1_channel.sinr_db;
                let cap = bandwidth_mhz * (1.0 + p1_linear).log2();
                (sinr, cap)
            } else {
                (-30.0, 0.0) // Total link failure
            }
        } else if self.active_beams.trp1_tci.is_none() {
            // Single TRP operation (TRP0 only)
            let sinr = self.trp0_channel.sinr_db;
            let cap = bandwidth_mhz * (1.0 + p0_linear).log2();
            (sinr, cap)
        } else {
            // Dual TRP operation
            match mode {
                MTrpTransmissionMode::SingleFrequencyNetwork => {
                    // SFN / Joint Transmission: Coherent/non-coherent power combination
                    let comb_p = p0_linear + p1_linear;
                    let comb_sinr = 10.0 * comb_p.log10();
                    let cap = bandwidth_mhz * (1.0 + comb_p).log2();
                    (comb_sinr, cap)
                }
                MTrpTransmissionMode::SpaceDivisionMultiplexing => {
                    // SDM: Parallel MIMO spatial streams across TRPs
                    let avg_sinr = (self.trp0_channel.sinr_db + self.trp1_channel.sinr_db) / 2.0;
                    let cap_trp0 = (bandwidth_mhz / 1.0) * (1.0 + p0_linear).log2();
                    let cap_trp1 = (bandwidth_mhz / 1.0) * (1.0 + p1_linear).log2();
                    (avg_sinr, cap_trp0 + cap_trp1)
                }
                MTrpTransmissionMode::FrequencyDivisionMultiplexing => {
                    // FDM: 50% PRBs per TRP
                    let avg_sinr = (self.trp0_channel.sinr_db + self.trp1_channel.sinr_db) / 2.0;
                    let cap_trp0 = (bandwidth_mhz * 0.5) * (1.0 + p0_linear).log2();
                    let cap_trp1 = (bandwidth_mhz * 0.5) * (1.0 + p1_linear).log2();
                    (avg_sinr, cap_trp0 + cap_trp1)
                }
                MTrpTransmissionMode::TimeDivisionMultiplexing => {
                    // TDM: Alternating time resources (50% each)
                    let avg_sinr = (self.trp0_channel.sinr_db + self.trp1_channel.sinr_db) / 2.0;
                    let cap_trp0 = 0.5 * bandwidth_mhz * (1.0 + p0_linear).log2();
                    let cap_trp1 = 0.5 * bandwidth_mhz * (1.0 + p1_linear).log2();
                    (avg_sinr, cap_trp0 + cap_trp1)
                }
            }
        };

        (effective_sinr_db, capacity_mbps)
    }

    /// Current active beams.
    pub fn active_beams(&self) -> ActiveBeamSet {
        self.active_beams
    }

    /// Current beam switch status.
    pub fn beam_switch_state(&self) -> BeamSwitchState {
        self.beam_switch_state
    }

    /// Current (SFN, Slot).
    pub fn current_timing(&self) -> (u16, u8) {
        (self.current_sfn, self.current_slot)
    }
}
