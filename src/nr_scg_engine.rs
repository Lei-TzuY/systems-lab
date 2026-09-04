//! 3GPP Rel-17 5G NR Dual Connectivity & Fast SCG Activation/Deactivation Engine.
//!
//! Compliant with 3GPP TS 38.331 Rel-17 §5.3.5 ("Fast SCG activation/deactivation"),
//! TS 38.321 §6.1.3.36 ("SCG Activation/Deactivation MAC CE"), and TS 38.133 §8.2.
//!
//! Dual Connectivity (MR-DC / NR-DC) connects a UE to both a Master Cell Group (MCG)
//! and a Secondary Cell Group (SCG). Rel-17 Fast SCG Activation/Deactivation allows:
//! - Instantaneous suspension of SCG radio transmissions (quiescent RF state).
//! - Preservation of full SCG RRC configuration, cell IDs, and security keys ($S-K_{\text{gNB}}$).
//! - Fast sub-10ms resumption via 1-byte MAC CE (LCID 52) or RRC reconfiguration.
//! - Power-saving telemetry and SCG failure reporting over MCG.

// ---------------------------------------------------------------------------
// Configuration & Types
// ---------------------------------------------------------------------------

/// Configuration of an SCG Cell (PSCell or SCell).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScgCellConfig {
    /// Physical Cell Identity (0..1007).
    pub pci: u16,
    /// Absolute Radio Frequency Channel Number (NR-ARFCN).
    pub arfcn: u32,
    /// Serving cell index (PSCell is typically 1, SCells are 2..7).
    pub serv_cell_index: u8,
    /// Timing advance offset in $T_c$ units.
    pub timing_advance: u16,
    /// True if this cell is the Primary SCG Cell (PSCell).
    pub is_pscell: bool,
    /// Active state of the cell radio interface.
    pub active: bool,
}

/// Radio bearer termination type in Dual Connectivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScgBearerType {
    /// SCG bearer: RLC and MAC terminated solely in SCG node.
    ScgBearer,
    /// Split bearer: PDCP in SN or MN, with RLC/MAC legs on both MCG and SCG.
    SplitBearer,
}

/// Radio bearer configuration mapped to the SCG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScgBearerConfig {
    pub drb_id: u8,
    pub bearer_type: ScgBearerType,
    pub security_key_id: u8,
    pub pdcp_sn_bits: u8,
}

/// 3GPP Rel-17 SCG Operational State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScgState {
    /// SCG is active: monitoring PDCCH, transmitting PUCCH/PUSCH, periodic CSI and SRS.
    Activated,
    /// SCG is deactivated (quiescent RF mode): PDCCH monitoring stopped, PUCCH/PUSCH
    /// disabled, SRS halted, CSI reports frozen. Full RRC configuration preserved.
    Deactivated,
    /// In progress of fast activation: awaiting sync or PRACH completion.
    Activating {
        slots_remaining: u16,
        needs_rach: bool,
    },
    /// In progress of fast deactivation: flushing in-flight HARQ transmissions.
    Deactivating { slots_remaining: u16 },
}

/// Causes for SCG Failure reporting (3GPP TS 38.331 §5.3.10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScgFailureReason {
    /// Radio Link Failure on PSCell (T310 expiry).
    T310Expiry,
    /// Reconfiguration with sync failure on SCG.
    SynchReconfigFailure,
    /// SCG change failure.
    ScgChangeFailure,
    /// Maximum number of RLC retransmissions reached on SCG/split bearer.
    MaxRlcRetransmissions,
    /// Consistent Uplink LBT failure on PSCell (NR-U).
    SriLbtFailure,
}

/// SCG Failure Information report dispatched over MCG (TS 38.331 §5.3.10.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScgFailureInformation {
    pub failure_type: ScgFailureReason,
    pub failed_pscell_pci: u16,
    pub meas_result_pscell_rsrp: Option<i16>,
    pub meas_result_pscell_rsrq: Option<i8>,
}

/// Event emitted by the SCG Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScgEngineEvent {
    /// State machine transitioned to a new state.
    StateChanged {
        old_state: String,
        new_state: String,
    },
    /// Fast activation procedure completed; SCG radio fully operational.
    ActivationCompleted,
    /// Fast deactivation procedure completed; SCG entered quiescent RF sleep.
    DeactivationCompleted,
    /// Uplink traffic on SCG/split bearer arrived while deactivated, triggering SR on MCG.
    SrTriggeredOnMcg { drb_id: u8, buffer_bytes: usize },
    /// SCG Failure Information generated for transmission to MN over MCG.
    ScgFailureReported(ScgFailureInformation),
}

/// Configuration settings for the Fast SCG Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScgEngineConfig {
    /// Fast activation delay in slots when PSCell is known and synchronized (typically 8 slots).
    pub activation_delay_sync_slots: u16,
    /// Fast activation delay in slots when PSCell requires non-contention RACH (typically 24 slots).
    pub activation_delay_rach_slots: u16,
    /// Fast deactivation graceful flush duration in slots (typically 2 slots).
    pub deactivation_flush_slots: u16,
    /// Threshold of uplink buffer bytes to automatically request SCG activation.
    pub ul_buffer_activation_threshold: usize,
}

impl Default for ScgEngineConfig {
    fn default() -> Self {
        Self {
            activation_delay_sync_slots: 8,
            activation_delay_rach_slots: 24,
            deactivation_flush_slots: 2,
            ul_buffer_activation_threshold: 1500,
        }
    }
}

// ---------------------------------------------------------------------------
// 5G NR Dual Connectivity & Fast SCG Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR Dual Connectivity & Fast SCG Engine.
#[derive(Debug)]
pub struct NrScgEngine {
    pub ue_c_rnti: u16,
    pub config: ScgEngineConfig,
    pub state: ScgState,

    // Cell configurations (PSCell + SCells)
    pub pscell: ScgCellConfig,
    pub scells: Vec<ScgCellConfig>,

    // Bearers configured on SCG
    pub bearers: Vec<ScgBearerConfig>,

    // Telemetry and statistics
    pub total_activations: u64,
    pub total_deactivations: u64,
    pub active_slots: u64,
    pub deactivated_slots: u64,
    pub failure_count: u64,
}

impl NrScgEngine {
    /// Create a new Fast SCG Engine instance.
    pub fn new(ue_c_rnti: u16, pscell: ScgCellConfig, config: ScgEngineConfig) -> Self {
        Self {
            ue_c_rnti,
            config,
            state: ScgState::Activated,
            pscell,
            scells: Vec::new(),
            bearers: Vec::new(),
            total_activations: 1, // Initially started as active
            total_deactivations: 0,
            active_slots: 0,
            deactivated_slots: 0,
            failure_count: 0,
        }
    }

    /// Add an SCG SCell to the cell group.
    pub fn add_scell(&mut self, mut scell: ScgCellConfig) {
        scell.is_pscell = false;
        scell.active = matches!(self.state, ScgState::Activated);
        self.scells.push(scell);
    }

    /// Register a radio bearer configured on SCG.
    pub fn add_bearer(&mut self, bearer: ScgBearerConfig) {
        self.bearers.push(bearer);
    }

    // -----------------------------------------------------------------------
    // MAC CE Encoding & Decoding (3GPP TS 38.321 §6.1.3.36)
    // -----------------------------------------------------------------------

    /// Serializes a 3GPP Rel-17 SCG Activation/Deactivation MAC CE (1 octet).
    ///
    /// Bit format:
    /// - Bit 7: A/D (1 = Activate SCG, 0 = Deactivate SCG)
    /// - Bits 6..0: C7 to C1 SCell activation/deactivation bitmap
    pub fn format_scg_activation_mac_ce(activate: bool, scell_bitmap: u8) -> u8 {
        let ad_bit = if activate { 0x80 } else { 0x00 };
        ad_bit | (scell_bitmap & 0x7F)
    }

    /// Parses a 3GPP Rel-17 SCG Activation/Deactivation MAC CE.
    ///
    /// Returns `(activate: bool, scell_bitmap: u8)`.
    pub fn parse_scg_activation_mac_ce(byte: u8) -> (bool, u8) {
        let activate = (byte & 0x80) != 0;
        let scell_bitmap = byte & 0x7F;
        (activate, scell_bitmap)
    }

    /// Processes an incoming SCG Activation/Deactivation MAC CE (LCID 52).
    pub fn handle_mac_ce(&mut self, byte: u8) -> Option<ScgEngineEvent> {
        let (activate, scell_bitmap) = Self::parse_scg_activation_mac_ce(byte);

        // Update SCells activation flags
        for scell in &mut self.scells {
            let bit_pos = scell.serv_cell_index;
            if bit_pos >= 1 && bit_pos <= 7 {
                let mask = 1 << (bit_pos - 1);
                scell.active = activate && ((scell_bitmap & mask) != 0);
            }
        }

        if activate {
            self.request_activation(false)
        } else {
            self.request_deactivation()
        }
    }

    /// Processes an RRC `RRCReconfiguration` with `scg-State` IE (TS 38.331 §5.3.5.3).
    pub fn handle_rrc_reconfiguration_state(&mut self, activate: bool) -> Option<ScgEngineEvent> {
        if activate {
            // RRC activation might require non-contention RACH if synchronization was lost
            self.request_activation(true)
        } else {
            self.request_deactivation()
        }
    }

    /// Trigger activation request with specified sync/RACH requirement.
    pub fn request_activation(&mut self, needs_rach: bool) -> Option<ScgEngineEvent> {
        match self.state {
            ScgState::Activated => None, // Already active
            ScgState::Activating { .. } => None,
            ScgState::Deactivated | ScgState::Deactivating { .. } => {
                let slots = if needs_rach {
                    self.config.activation_delay_rach_slots
                } else {
                    self.config.activation_delay_sync_slots
                };

                let old_state_str = format!("{:?}", self.state);
                self.state = ScgState::Activating {
                    slots_remaining: slots,
                    needs_rach,
                };
                let new_state_str = format!("{:?}", self.state);

                Some(ScgEngineEvent::StateChanged {
                    old_state: old_state_str,
                    new_state: new_state_str,
                })
            }
        }
    }

    /// Trigger deactivation request.
    pub fn request_deactivation(&mut self) -> Option<ScgEngineEvent> {
        match self.state {
            ScgState::Deactivated => None, // Already deactivated
            ScgState::Deactivating { .. } => None,
            ScgState::Activated | ScgState::Activating { .. } => {
                let slots = self.config.deactivation_flush_slots;
                let old_state_str = format!("{:?}", self.state);
                self.state = ScgState::Deactivating {
                    slots_remaining: slots,
                };
                let new_state_str = format!("{:?}", self.state);

                Some(ScgEngineEvent::StateChanged {
                    old_state: old_state_str,
                    new_state: new_state_str,
                })
            }
        }
    }

    /// Evaluates uplink data arrival for SCG / Split bearers.
    ///
    /// If SCG is deactivated and traffic exceeds the activation threshold,
    /// triggers a Scheduling Request (SR) or BSR over MCG.
    pub fn handle_ul_data_arrival(
        &mut self,
        drb_id: u8,
        buffer_bytes: usize,
    ) -> Option<ScgEngineEvent> {
        let bearer = self.bearers.iter().find(|b| b.drb_id == drb_id)?;

        if matches!(self.state, ScgState::Deactivated)
            && buffer_bytes >= self.config.ul_buffer_activation_threshold
        {
            // Trigger SR on MCG to awaken SCG!
            Some(ScgEngineEvent::SrTriggeredOnMcg {
                drb_id: bearer.drb_id,
                buffer_bytes,
            })
        } else {
            None
        }
    }

    /// Triggers an SCG Radio Link Failure or integrity failure (TS 38.331 §5.3.10.3).
    pub fn trigger_scg_failure(
        &mut self,
        reason: ScgFailureReason,
        rsrp: Option<i16>,
        rsrq: Option<i8>,
    ) -> ScgEngineEvent {
        self.failure_count += 1;
        self.pscell.active = false;
        for scell in &mut self.scells {
            scell.active = false;
        }
        self.state = ScgState::Deactivated;

        let report = ScgFailureInformation {
            failure_type: reason,
            failed_pscell_pci: self.pscell.pci,
            meas_result_pscell_rsrp: rsrp,
            meas_result_pscell_rsrq: rsrq,
        };

        ScgEngineEvent::ScgFailureReported(report)
    }

    /// Advance time by one slot: manages activation/deactivation countdowns and energy metrics.
    pub fn step_slot(&mut self) -> Option<ScgEngineEvent> {
        match self.state {
            ScgState::Activated => {
                self.active_slots += 1;
                None
            }
            ScgState::Deactivated => {
                self.deactivated_slots += 1;
                None
            }
            ScgState::Activating {
                ref mut slots_remaining,
                ..
            } => {
                if *slots_remaining > 0 {
                    *slots_remaining -= 1;
                }

                if *slots_remaining == 0 {
                    self.state = ScgState::Activated;
                    self.pscell.active = true;
                    self.total_activations += 1;
                    self.active_slots += 1;
                    Some(ScgEngineEvent::ActivationCompleted)
                } else {
                    None
                }
            }
            ScgState::Deactivating {
                ref mut slots_remaining,
            } => {
                if *slots_remaining > 0 {
                    *slots_remaining -= 1;
                }

                if *slots_remaining == 0 {
                    self.state = ScgState::Deactivated;
                    self.pscell.active = false;
                    for scell in &mut self.scells {
                        scell.active = false;
                    }
                    self.total_deactivations += 1;
                    self.deactivated_slots += 1;
                    Some(ScgEngineEvent::DeactivationCompleted)
                } else {
                    None
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Radio Procedures Interrogation (TS 38.331 §5.3.5.3)
    // -----------------------------------------------------------------------

    /// Whether the UE monitors PDCCH on PSCell and active SCG SCells.
    pub fn is_pdcch_monitored(&self) -> bool {
        matches!(self.state, ScgState::Activated)
    }

    /// Whether periodic CSI reporting is transmitted for SCG cells.
    pub fn is_csi_reporting_active(&self) -> bool {
        matches!(self.state, ScgState::Activated)
    }

    /// Whether Sounding Reference Signals (SRS) are transmitted on SCG cells.
    pub fn is_srs_active(&self) -> bool {
        matches!(self.state, ScgState::Activated)
    }

    /// Whether PUSCH uplink transmissions are allowed on SCG cells.
    pub fn is_pusch_enabled(&self) -> bool {
        matches!(self.state, ScgState::Activated)
    }

    /// Calculates the RF energy savings ratio achieved via Fast SCG Deactivation.
    ///
    /// Returns ratio 0.0 .. 1.0 (e.g. 0.75 = 75% power savings).
    pub fn get_power_savings_percentage(&self) -> f64 {
        let total = self.active_slots + self.deactivated_slots;
        if total == 0 {
            0.0
        } else {
            (self.deactivated_slots as f64 / total as f64) * 100.0
        }
    }
}
