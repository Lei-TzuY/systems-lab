//! 3GPP TS 38.321 §5.1 / TS 38.213 §8 / TS 38.300 §9.2 Release 17 5G NR Random Access Channel (RACH) Procedure & Preamble Engine.
//!
//! Implements 5G NR Layer 2 / MAC Random Access Procedure:
//! - 4-step Contention-Based Random Access (CBRA):
//!   - Msg1: Preamble Group A/B selection, PRACH occasion binding, RA-RNTI calculation, and power ramping
//!   - Msg2: Random Access Response (MAC RAR) with BI subheader, RAPID, Timing Advance (TA), and UL grant
//!   - Msg3: Scheduled PUSCH transmission carrying CCCH SDU (RRCSetupRequest, etc.) or C-RNTI MAC CE
//!   - Msg4: Contention Resolution verification, 48-bit CCCH echo matching, and TC-RNTI to C-RNTI promotion
//! - Contention-Free Random Access (CFRA):
//!   - Dedicated preamble and dedicated RO assignment for Handover and Beam Failure Recovery (BFR)
//! - 2-step RACH (Rel-16/Rel-17 Type-2 CBRA):
//!   - MsgA (Preamble + PUSCH payload) and MsgB (SuccessRAR / FallbackRAR)
//! - Power ramping and exponential backoff retry state machine
//!
//! Pure Rust standard library implementation with zero external dependencies.

// ---------------------------------------------------------------------------
// 5G NR RACH Enums & Configuration (TS 38.321 Section 5.1 / TS 38.213 Section 8)
// ---------------------------------------------------------------------------

/// RACH Procedure Type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RachType {
    /// 4-Step Contention-Based Random Access (Msg1 -> Msg2 -> Msg3 -> Msg4).
    Cbra4Step,
    /// 4-Step Contention-Free Random Access with dedicated preamble (Msg1 -> Msg2).
    Cfra4Step,
    /// 2-Step Contention-Based Random Access (MsgA -> MsgB).
    Cbra2Step,
}

/// Random Access Cause / Trigger (TS 38.300 Section 9.2.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RachCause {
    InitialAccess,
    RrcReestablishment,
    Handover,
    UlDataArrival,
    BeamFailureRecovery,
    OtherSiRequest,
}

/// Preamble Group for Msg1 selection (TS 38.321 Section 5.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreambleGroup {
    GroupA,
    GroupB,
}

/// PRACH Occasion (RO) definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrachOccasion {
    /// First OFDM symbol in slot (0..13).
    pub symbol_id: u8,
    /// Slot index in system frame (0..79).
    pub slot_id: u8,
    /// Frequency domain index (0..7).
    pub freq_id: u8,
    /// 0 = Normal Uplink (NUL), 1 = Supplementary Uplink (SUL).
    pub ul_carrier_id: u8,
}

impl PrachOccasion {
    pub fn new(symbol_id: u8, slot_id: u8, freq_id: u8, ul_carrier_id: u8) -> Self {
        PrachOccasion {
            symbol_id: symbol_id & 0x0F,
            slot_id: slot_id % 80,
            freq_id: freq_id & 0x07,
            ul_carrier_id: ul_carrier_id & 0x01,
        }
    }

    /// Calculate standard 5G RA-RNTI (3GPP TS 38.321 Section 5.1.2).
    /// RA-RNTI = 1 + s_id + 14 * t_id + 14 * 80 * f_id + 14 * 80 * 8 * ul_carrier_id
    pub fn calculate_ra_rnti(&self) -> u16 {
        let s = self.symbol_id as u32;
        let t = self.slot_id as u32;
        let f = self.freq_id as u32;
        let c = self.ul_carrier_id as u32;
        (1 + s + 14 * t + 14 * 80 * f + 14 * 80 * 8 * c) as u16
    }
}

/// Backoff Indicator (BI) to delay mapping (TS 38.321 Table 7.2-1).
pub fn bi_to_delay_ms(bi: u8) -> u32 {
    match bi & 0x0F {
        0 => 5,
        1 => 10,
        2 => 20,
        3 => 30,
        4 => 40,
        5 => 60,
        6 => 80,
        7 => 120,
        8 => 160,
        9 => 240,
        10 => 320,
        11 => 480,
        12 => 960,
        _ => 960,
    }
}

/// 5G NR Cell RACH Configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RachConfig {
    pub preamble_trans_max: u32,
    pub power_ramping_step_db: i8,
    pub preamble_init_target_power_dbm: i8,
    pub ra_response_window_slots: u32,
    pub ra_contention_resolution_timer_slots: u32,
    pub total_preambles: u8,            // Typically 64
    pub cb_preambles_per_ssb: u8,       // Preambles for CBRA (e.g. 56)
    pub group_b_threshold_bytes: usize, // Msg3 size threshold to use Group B
    pub group_b_pathloss_threshold_db: i16,
}

impl Default for RachConfig {
    fn default() -> Self {
        RachConfig {
            preamble_trans_max: 6,
            power_ramping_step_db: 2,
            preamble_init_target_power_dbm: -108,
            ra_response_window_slots: 20,
            ra_contention_resolution_timer_slots: 64,
            total_preambles: 64,
            cb_preambles_per_ssb: 56,
            group_b_threshold_bytes: 56,
            group_b_pathloss_threshold_db: 90,
        }
    }
}

// ---------------------------------------------------------------------------
// RACH Messages & Payloads
// ---------------------------------------------------------------------------

/// Msg1 Preamble Transmission parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg1Transmission {
    pub preamble_index: u8,
    pub ra_rnti: u16,
    pub tx_power_dbm: i16,
    pub ssb_index: u8,
    pub ro: PrachOccasion,
    pub transmission_counter: u32,
}

/// MAC Random Access Response (RAR) payload (TS 38.321 Section 6.2.2 & 6.2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacRarPayload {
    pub rapid: u8,
    pub timing_advance: u16, // 12 bits: 0..3846
    pub ul_grant: u32,       // 27-bit PUSCH grant
    pub tc_rnti: u16,        // Temporary C-RNTI
}

/// Msg2 MAC RAR Message containing subheaders and payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg2RarMessage {
    pub backoff_indicator: Option<u8>,
    pub rar_payloads: Vec<MacRarPayload>,
}

/// Msg3 PUSCH Scheduled Transmission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg3Transmission {
    pub tc_rnti: u16,
    pub payload: Vec<u8>, // CCCH SDU or C-RNTI MAC CE
    pub ul_grant: u32,
    pub tx_slot: u64,
}

/// Msg4 Contention Resolution Message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg4ContentionResolution {
    pub contention_resolution_id: [u8; 6], // 48-bit CCCH SDU echo
}

/// 2-Step RACH MsgA Transmission (Rel-16/Rel-17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsgATransmission {
    pub preamble_index: u8,
    pub msg_a_pusch_payload: Vec<u8>,
    pub ssb_index: u8,
    pub ro: PrachOccasion,
}

/// 2-Step RACH MsgB Response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgBResponse {
    SuccessRar {
        c_rnti: u16,
        timing_advance: u16,
        contention_resolution_id: [u8; 6],
    },
    FallbackRar {
        rapid: u8,
        timing_advance: u16,
        ul_grant: u32,
        tc_rnti: u16,
    },
}

/// RACH Operating State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RachState {
    Idle,
    Msg1Transmitted,
    Msg2Received,
    Msg3Transmitted,
    Completed { c_rnti: u16, ta: u16 },
    Failed(RachFailureReason),
}

/// RACH Failure Reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RachFailureReason {
    MaxPreambleReached,
    ContentionResolutionFailed,
    RarTimeout,
}

// ---------------------------------------------------------------------------
// Top-Level 5G NR RACH Engine
// ---------------------------------------------------------------------------

pub struct NrRachEngine {
    pub ue_id: String,
    pub config: RachConfig,
    pub state: RachState,
    pub rach_type: RachType,
    pub rach_cause: RachCause,
    pub c_rnti: Option<u16>,
    pub dedicated_preamble: Option<u8>, // for CFRA
    pub current_ssb: u8,
    pub preamble_trans_counter: u32,
    pub power_ramping_counter: u32,
    pub current_tx_power_dbm: i16,
    pub active_msg1: Option<Msg1PreambleState>,
    pub active_msg3: Option<Msg3Transmission>,
    pub contention_timer_remaining: Option<u32>,
    pub backoff_slots_remaining: u32,
    pub total_ra_attempts: u32,
    pub successful_ra_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg1PreambleState {
    pub info: Msg1Transmission,
    pub window_remaining_slots: u32,
}

impl NrRachEngine {
    /// Create a new 5G NR RACH Engine.
    pub fn new(ue_id: &str, config: RachConfig) -> Self {
        let init_power = config.preamble_init_target_power_dbm as i16;
        NrRachEngine {
            ue_id: ue_id.to_string(),
            config,
            state: RachState::Idle,
            rach_type: RachType::Cbra4Step,
            rach_cause: RachCause::InitialAccess,
            c_rnti: None,
            dedicated_preamble: None,
            current_ssb: 0,
            preamble_trans_counter: 0,
            power_ramping_counter: 0,
            current_tx_power_dbm: init_power,
            active_msg1: None,
            active_msg3: None,
            contention_timer_remaining: None,
            backoff_slots_remaining: 0,
            total_ra_attempts: 0,
            successful_ra_count: 0,
        }
    }

    /// Initiate 4-step CBRA procedure.
    pub fn initiate_4step_cbra(
        &mut self,
        cause: RachCause,
        selected_ssb: u8,
        msg3_size_bytes: usize,
        estimated_pathloss_db: i16,
        ro: PrachOccasion,
    ) -> Result<Msg1Transmission, RachFailureReason> {
        self.rach_type = RachType::Cbra4Step;
        self.rach_cause = cause;
        self.current_ssb = selected_ssb;
        self.dedicated_preamble = None;
        if self.preamble_trans_counter == 0 {
            self.preamble_trans_counter = 1;
            self.power_ramping_counter = 0;
            self.current_tx_power_dbm = self.config.preamble_init_target_power_dbm as i16;
        }
        self.total_ra_attempts += 1;

        // Select Group A or Group B
        let group = if msg3_size_bytes > self.config.group_b_threshold_bytes
            && estimated_pathloss_db < self.config.group_b_pathloss_threshold_db
        {
            PreambleGroup::GroupB
        } else {
            PreambleGroup::GroupA
        };

        // Select preamble index within group
        let preamble_index = match group {
            PreambleGroup::GroupA => (selected_ssb * 8 + 1) % self.config.cb_preambles_per_ssb,
            PreambleGroup::GroupB => {
                let offset = self.config.cb_preambles_per_ssb / 2;
                (offset + selected_ssb * 4 + 1) % self.config.cb_preambles_per_ssb
            }
        };

        let ra_rnti = ro.calculate_ra_rnti();
        let msg1 = Msg1Transmission {
            preamble_index,
            ra_rnti,
            tx_power_dbm: self.current_tx_power_dbm,
            ssb_index: selected_ssb,
            ro,
            transmission_counter: self.preamble_trans_counter,
        };

        self.active_msg1 = Some(Msg1PreambleState {
            info: msg1.clone(),
            window_remaining_slots: self.config.ra_response_window_slots,
        });
        self.state = RachState::Msg1Transmitted;

        Ok(msg1)
    }

    /// Initiate Contention-Free Random Access (CFRA) for Handover or BFR.
    pub fn initiate_cfra(
        &mut self,
        cause: RachCause,
        dedicated_preamble: u8,
        selected_ssb: u8,
        ro: PrachOccasion,
    ) -> Msg1Transmission {
        self.rach_type = RachType::Cfra4Step;
        self.rach_cause = cause;
        self.dedicated_preamble = Some(dedicated_preamble);
        self.current_ssb = selected_ssb;
        if self.preamble_trans_counter == 0 {
            self.preamble_trans_counter = 1;
            self.power_ramping_counter = 0;
            self.current_tx_power_dbm = self.config.preamble_init_target_power_dbm as i16;
        }
        self.total_ra_attempts += 1;

        let ra_rnti = ro.calculate_ra_rnti();
        let msg1 = Msg1Transmission {
            preamble_index: dedicated_preamble,
            ra_rnti,
            tx_power_dbm: self.current_tx_power_dbm,
            ssb_index: selected_ssb,
            ro,
            transmission_counter: self.preamble_trans_counter,
        };

        self.active_msg1 = Some(Msg1PreambleState {
            info: msg1.clone(),
            window_remaining_slots: self.config.ra_response_window_slots,
        });
        self.state = RachState::Msg1Transmitted;

        msg1
    }

    /// Process received Msg2 MAC RAR.
    pub fn handle_msg2_rar(
        &mut self,
        rar_msg: &Msg2RarMessage,
    ) -> Result<Option<MacRarPayload>, RachFailureReason> {
        let active = match &self.active_msg1 {
            Some(a) => a,
            None => return Err(RachFailureReason::RarTimeout),
        };

        let target_rapid = active.info.preamble_index;

        // Check if any RAR matches our preamble index (RAPID)
        let mut matched_rar = None;
        for rar in &rar_msg.rar_payloads {
            if rar.rapid == target_rapid {
                matched_rar = Some(rar.clone());
                break;
            }
        }

        if let Some(rar) = matched_rar {
            self.state = RachState::Msg2Received;
            self.active_msg1 = None;

            // If CFRA: procedure is successfully completed immediately!
            if self.rach_type == RachType::Cfra4Step {
                let assigned_c_rnti = self.c_rnti.unwrap_or(rar.tc_rnti);
                self.c_rnti = Some(assigned_c_rnti);
                self.state = RachState::Completed {
                    c_rnti: assigned_c_rnti,
                    ta: rar.timing_advance,
                };
                self.preamble_trans_counter = 0;
                self.power_ramping_counter = 0;
                self.successful_ra_count += 1;
            }

            return Ok(Some(rar));
        }

        // Check Backoff Indicator if present
        if let Some(bi) = rar_msg.backoff_indicator {
            let delay_ms = bi_to_delay_ms(bi);
            self.backoff_slots_remaining = delay_ms / 1; // slots approx
        }

        Ok(None)
    }

    /// Transmit Msg3 using UL grant received in RAR (4-step CBRA).
    pub fn transmit_msg3(
        &mut self,
        rar: &MacRarPayload,
        msg3_payload: Vec<u8>,
        current_slot: u64,
    ) -> Result<Msg3Transmission, RachFailureReason> {
        if self.state != RachState::Msg2Received {
            return Err(RachFailureReason::ContentionResolutionFailed);
        }

        let msg3 = Msg3Transmission {
            tc_rnti: rar.tc_rnti,
            payload: msg3_payload,
            ul_grant: rar.ul_grant,
            tx_slot: current_slot,
        };

        self.active_msg3 = Some(msg3.clone());
        self.contention_timer_remaining = Some(self.config.ra_contention_resolution_timer_slots);
        self.state = RachState::Msg3Transmitted;

        Ok(msg3)
    }

    /// Process Msg4 Contention Resolution.
    pub fn handle_msg4_contention_resolution(
        &mut self,
        msg4: &Msg4ContentionResolution,
        rar_ta: u16,
    ) -> Result<bool, RachFailureReason> {
        let msg3 = match &self.active_msg3 {
            Some(m) => m,
            None => return Err(RachFailureReason::ContentionResolutionFailed),
        };

        // Validate 48-bit CCCH SDU echo match (first 6 bytes of Msg3 payload)
        if msg3.payload.len() >= 6 && msg3.payload[0..6] == msg4.contention_resolution_id {
            // Contention Resolution Success!
            let final_c_rnti = msg3.tc_rnti;
            self.c_rnti = Some(final_c_rnti);
            self.state = RachState::Completed {
                c_rnti: final_c_rnti,
                ta: rar_ta,
            };
            self.preamble_trans_counter = 0;
            self.power_ramping_counter = 0;
            self.active_msg3 = None;
            self.contention_timer_remaining = None;
            self.successful_ra_count += 1;
            Ok(true)
        } else {
            // Contention Resolution Mismatch / Collision!
            self.retry_or_fail(RachFailureReason::ContentionResolutionFailed)
        }
    }

    /// Trigger retransmission or declare failure when timer expires or collision occurs.
    pub fn retry_or_fail(&mut self, reason: RachFailureReason) -> Result<bool, RachFailureReason> {
        self.preamble_trans_counter += 1;
        self.active_msg1 = None;
        self.active_msg3 = None;
        self.contention_timer_remaining = None;

        if self.preamble_trans_counter > self.config.preamble_trans_max {
            self.state = RachState::Failed(RachFailureReason::MaxPreambleReached);
            return Err(RachFailureReason::MaxPreambleReached);
        }

        // Power ramping
        self.power_ramping_counter += 1;
        self.current_tx_power_dbm += self.config.power_ramping_step_db as i16;
        self.state = RachState::Idle;

        Err(reason)
    }

    /// Slot timer tick: decrements response window and contention resolution timers.
    pub fn tick_slot(&mut self) -> Option<RachFailureReason> {
        // Tick response window
        if let Some(active) = &mut self.active_msg1 {
            if active.window_remaining_slots > 0 {
                active.window_remaining_slots -= 1;
                if active.window_remaining_slots == 0 {
                    return match self.retry_or_fail(RachFailureReason::RarTimeout) {
                        Ok(_) => None,
                        Err(fail) => Some(fail),
                    };
                }
            }
        }

        // Tick contention resolution timer
        if let Some(timer) = &mut self.contention_timer_remaining {
            if *timer > 0 {
                *timer -= 1;
                if *timer == 0 {
                    return match self.retry_or_fail(RachFailureReason::ContentionResolutionFailed) {
                        Ok(_) => None,
                        Err(fail) => Some(fail),
                    };
                }
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // 2-Step RACH Procedure (Rel-16/Rel-17 Type-2 CBRA)
    // -----------------------------------------------------------------------

    /// Initiate 2-step RACH MsgA transmission.
    pub fn initiate_2step_msga(
        &mut self,
        preamble_index: u8,
        msg_a_payload: Vec<u8>,
        selected_ssb: u8,
        ro: PrachOccasion,
    ) -> MsgATransmission {
        self.rach_type = RachType::Cbra2Step;
        self.current_ssb = selected_ssb;
        self.preamble_trans_counter = 1;
        self.total_ra_attempts += 1;

        let msga = MsgATransmission {
            preamble_index,
            msg_a_pusch_payload: msg_a_payload,
            ssb_index: selected_ssb,
            ro,
        };
        self.state = RachState::Msg1Transmitted;
        msga
    }

    /// Process MsgB response for 2-step RACH.
    pub fn handle_msgb_response(
        &mut self,
        msgb: &MsgBResponse,
        expected_echo: &[u8; 6],
    ) -> Result<bool, RachFailureReason> {
        match msgb {
            MsgBResponse::SuccessRar {
                c_rnti,
                timing_advance,
                contention_resolution_id,
            } => {
                if contention_resolution_id == expected_echo {
                    self.c_rnti = Some(*c_rnti);
                    self.state = RachState::Completed {
                        c_rnti: *c_rnti,
                        ta: *timing_advance,
                    };
                    self.successful_ra_count += 1;
                    Ok(true)
                } else {
                    self.state = RachState::Failed(RachFailureReason::ContentionResolutionFailed);
                    Err(RachFailureReason::ContentionResolutionFailed)
                }
            }
            MsgBResponse::FallbackRar {
                rapid: _,
                timing_advance: _,
                ul_grant: _,
                tc_rnti: _,
            } => {
                // Fallback to Msg3 transmission in 4-step
                self.rach_type = RachType::Cbra4Step;
                self.state = RachState::Msg2Received;
                Ok(false)
            }
        }
    }
}
