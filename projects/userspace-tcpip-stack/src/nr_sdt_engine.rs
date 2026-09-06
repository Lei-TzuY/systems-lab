//! 3GPP Rel-17 5G NR Small Data Transmission (SDT) in RRC_INACTIVE Engine
//!
//! Conforms to:
//! - 3GPP TS 38.300 §18: Small Data Transmission in RRC_INACTIVE
//! - 3GPP TS 38.321 §5.27: Small Data Transmission procedure
//! - 3GPP TS 38.331: `sdt-Config-r17`, `RRCResumeRequest1`, `RRCRelease` with `suspendConfig`
//! - 3GPP TS 38.304: Idle and Inactive mode procedures
//!
//! Pure standard Rust (`std`/`core` only), zero external dependencies.

use std::fmt;

/// Logical Channel IDs (LCID) for MAC Subheaders in SDT (TS 38.321 §6.2.1).
pub const MAC_LCID_CCCH_SDT: u8 = 0x00;
pub const MAC_LCID_DTCH_MIN: u8 = 0x01;
pub const MAC_LCID_DTCH_MAX: u8 = 0x20;
pub const MAC_LCID_SHORT_BSR: u8 = 0x3D;

/// Type of Small Data Transmission selected for transmission (TS 38.321 §5.27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdtType {
    /// SDT not triggered; criteria not met (fallback to legacy full RRC Connection Resume).
    None,
    /// 4-step Random Access based SDT (RA-SDT Msg3).
    RaSdt4Step,
    /// 2-step Random Access based SDT (RA-SDT MsgA).
    RaSdt2Step,
    /// Configured Grant based SDT (CG-SDT without RACH preamble).
    CgSdt,
}

impl fmt::Display for SdtType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SdtType::None => write!(f, "NONE"),
            SdtType::RaSdt4Step => write!(f, "RA-SDT-4Step"),
            SdtType::RaSdt2Step => write!(f, "RA-SDT-2Step"),
            SdtType::CgSdt => write!(f, "CG-SDT"),
        }
    }
}

/// Operational state of the SDT state machine (TS 38.321 §5.27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdtProcedureState {
    /// UE is in RRC_INACTIVE standby mode.
    InactiveStandby,
    /// SDT triggered by uplink buffer; transmission initiated.
    Initiated { sdt_type: SdtType },
    /// Initial SDT MAC PDU transmitted; monitoring PDCCH for gNB response.
    AwaitingResponse {
        sdt_type: SdtType,
        transmitted_bytes: usize,
    },
    /// Subsequent data transfer in progress under dynamic grants; inactivity timer running.
    SubsequentData {
        remaining_inactivity_ms: u32,
        total_transferred_bytes: usize,
    },
    /// SDT successfully concluded via RRCRelease with suspendConfig; UE remains in RRC_INACTIVE.
    TerminatedSuccess { total_transferred_bytes: usize },
    /// gNB ordered fallback to full RRC_CONNECTED via RRCResume.
    FallbackToConnected { total_transferred_bytes: usize },
    /// gNB congestion backoff via RRCReject.
    Rejected { backoff_ms: u32 },
}

/// Configuration parameters for Small Data Transmission (TS 38.331 `sdt-Config-r17`).
#[derive(Debug, Clone, PartialEq)]
pub struct SdtConfig {
    /// Maximum pending uplink buffer volume in bytes to trigger SDT (e.g. 1024 bytes).
    pub data_volume_threshold_bytes: usize,
    /// Minimum serving cell RSRP threshold in dBm to permit SDT (e.g. -105.0 dBm).
    pub rsrp_threshold_dbm: f32,
    /// Inactivity timer in milliseconds for subsequent data transfers (e.g. 160 ms).
    pub inactivity_timer_ms: u32,
    /// Timing Alignment (TA) validity timer in milliseconds for CG-SDT (e.g. 2560 ms).
    pub cg_ta_timer_ms: u32,
    /// Whether Configured Grant SDT resources are configured by the network.
    pub cg_configured: bool,
    /// Whether 2-step RACH SDT is supported in this cell.
    pub support_2step_ra: bool,
    /// Radio Bearer IDs allowed for SDT (e.g. DRB 1).
    pub allowed_drb_ids: Vec<u8>,
}

impl Default for SdtConfig {
    fn default() -> Self {
        Self {
            data_volume_threshold_bytes: 1024,
            rsrp_threshold_dbm: -105.0,
            inactivity_timer_ms: 160,
            cg_ta_timer_ms: 2560,
            cg_configured: true,
            support_2step_ra: true,
            allowed_drb_ids: vec![1],
        }
    }
}

/// Multiplexed MAC PDU containing CCCH (RRCResumeRequest1) and DTCH (user data) (TS 38.321 §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdtMacPdu {
    /// CCCH SDU containing encoded RRCResumeRequest1 (e.g. I-RNTI + ShortMAC-I).
    pub ccch_sdu: Vec<u8>,
    /// DTCH SDU containing user plane data payload.
    pub dtch_sdu: Vec<u8>,
    /// Optional Buffer Status Report (remaining buffer size in bytes).
    pub remaining_bsr_bytes: Option<u32>,
}

impl SdtMacPdu {
    /// Create a new SDT MAC PDU.
    pub fn new(ccch_sdu: Vec<u8>, dtch_sdu: Vec<u8>, remaining_bsr_bytes: Option<u32>) -> Self {
        Self {
            ccch_sdu,
            dtch_sdu,
            remaining_bsr_bytes,
        }
    }

    /// Serialize into binary wire format:
    /// Format:
    /// - [CCCH Subheader (1B: LCID 0x00)][CCCH Len (2B)][CCCH Data]
    /// - [DTCH Subheader (1B: LCID 0x01)][DTCH Len (2B)][DTCH Data]
    /// - Optional [BSR Subheader (1B: LCID 0x3D)][BSR Value (4B)]
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // 1. CCCH SDU
        buf.push(MAC_LCID_CCCH_SDT);
        let ccch_len = self.ccch_sdu.len() as u16;
        buf.extend_from_slice(&ccch_len.to_be_bytes());
        buf.extend_from_slice(&self.ccch_sdu);

        // 2. DTCH SDU
        buf.push(MAC_LCID_DTCH_MIN);
        let dtch_len = self.dtch_sdu.len() as u16;
        buf.extend_from_slice(&dtch_len.to_be_bytes());
        buf.extend_from_slice(&self.dtch_sdu);

        // 3. Optional BSR
        if let Some(bsr) = self.remaining_bsr_bytes {
            buf.push(MAC_LCID_SHORT_BSR);
            buf.extend_from_slice(&bsr.to_be_bytes());
        }

        buf
    }

    /// Parse an SDT MAC PDU from binary wire bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 6 {
            return Err("Payload too short for SDT MAC PDU");
        }

        let mut offset = 0;
        let mut ccch_sdu = Vec::new();
        let mut dtch_sdu = Vec::new();
        let mut remaining_bsr_bytes = None;

        while offset < bytes.len() {
            let lcid = bytes[offset];
            offset += 1;

            match lcid {
                MAC_LCID_CCCH_SDT => {
                    if offset + 2 > bytes.len() {
                        return Err("Malformed CCCH length");
                    }
                    let len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                    offset += 2;
                    if offset + len > bytes.len() {
                        return Err("CCCH payload truncated");
                    }
                    ccch_sdu = bytes[offset..offset + len].to_vec();
                    offset += len;
                }
                MAC_LCID_DTCH_MIN..=MAC_LCID_DTCH_MAX => {
                    if offset + 2 > bytes.len() {
                        return Err("Malformed DTCH length");
                    }
                    let len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                    offset += 2;
                    if offset + len > bytes.len() {
                        return Err("DTCH payload truncated");
                    }
                    dtch_sdu = bytes[offset..offset + len].to_vec();
                    offset += len;
                }
                MAC_LCID_SHORT_BSR => {
                    if offset + 4 > bytes.len() {
                        return Err("BSR field truncated");
                    }
                    let bsr = u32::from_be_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                    ]);
                    remaining_bsr_bytes = Some(bsr);
                    offset += 4;
                }
                _ => return Err("Unsupported LCID in SDT MAC PDU"),
            }
        }

        Ok(Self {
            ccch_sdu,
            dtch_sdu,
            remaining_bsr_bytes,
        })
    }
}

/// Network response action sent by gNB in response to an SDT transmission (TS 38.331).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdtResponseAction {
    /// RRCRelease with suspendConfig: SDT complete; UE stays in RRC_INACTIVE.
    RrcReleaseWithSuspend,
    /// RRCResume: Fallback to full RRC_CONNECTED.
    RrcResume,
    /// RRCReject: Network congestion; backoff requested.
    RrcReject { wait_time_sec: u16 },
    /// Dynamic Grant: Network provides additional UL grant for subsequent data.
    DynamicGrant { granted_bytes: usize },
}

/// Performance and energy saving metrics comparing SDT with legacy RRC Resume.
#[derive(Debug, Clone, PartialEq)]
pub struct SdtPerformanceMetrics {
    /// Total bytes of user data successfully transferred via SDT.
    pub user_data_bytes_transferred: usize,
    /// Total control signaling overhead bytes used in SDT.
    pub sdt_signaling_overhead_bytes: usize,
    /// Estimated control signaling overhead bytes that would have been used by legacy RRC Resume.
    pub legacy_signaling_overhead_bytes: usize,
    /// Percentage reduction in control signaling overhead.
    pub signaling_reduction_percentage: f32,
    /// Estimated radio energy consumed in mJ.
    pub estimated_energy_consumed_mj: f32,
    /// Estimated radio energy saved compared to legacy procedure in mJ.
    pub estimated_energy_saved_mj: f32,
}

/// Primary 3GPP Rel-17 Small Data Transmission (SDT) Engine.
#[derive(Debug, Clone)]
pub struct SdtEngine {
    /// SDT configuration.
    pub config: SdtConfig,
    /// Current state machine state.
    pub state: SdtProcedureState,
    /// Inactive Timing Alignment timer remaining in ms.
    pub ta_timer_remaining_ms: u32,
    /// Suspended AS Security context token (simulated ShortMAC-I seed).
    pub short_mac_i_token: u16,
    /// Total user data bytes successfully transferred in this session.
    pub total_user_bytes_transferred: usize,
    /// Total control signaling bytes exchanged in this session.
    pub total_signaling_bytes_used: usize,
}

impl SdtEngine {
    /// Create a new SDT Engine with the given configuration.
    pub fn new(config: SdtConfig) -> Self {
        Self {
            config,
            state: SdtProcedureState::InactiveStandby,
            ta_timer_remaining_ms: 0,
            short_mac_i_token: 0xCAFE,
            total_user_bytes_transferred: 0,
            total_signaling_bytes_used: 0,
        }
    }

    /// Start or refresh Timing Alignment for Configured Grant SDT.
    pub fn update_timing_alignment(&mut self) {
        self.ta_timer_remaining_ms = self.config.cg_ta_timer_ms;
    }

    /// Check if Timing Alignment is currently valid for CG-SDT.
    pub fn is_ta_valid(&self) -> bool {
        self.ta_timer_remaining_ms > 0
    }

    /// Evaluate SDT trigger criteria per TS 38.321 §5.27.1.
    ///
    /// Determines whether SDT should be initiated and which transmission mode to use:
    /// - `CgSdt`: If CG configured, TA is valid, and buffer <= threshold and RSRP >= threshold.
    /// - `RaSdt2Step`: If CG not available, 2-step RA supported, and criteria met.
    /// - `RaSdt4Step`: If CG not available, standard 4-step RA.
    /// - `None`: If buffer exceeds threshold or RSRP is too low (fallback to legacy RRC Resume).
    pub fn evaluate_sdt_criteria(
        &self,
        buffer_size_bytes: usize,
        serving_rsrp_dbm: f32,
    ) -> SdtType {
        // Criteria 1: Buffer size check
        if buffer_size_bytes == 0 || buffer_size_bytes > self.config.data_volume_threshold_bytes {
            return SdtType::None;
        }

        // Criteria 2: RSRP threshold check
        if serving_rsrp_dbm < self.config.rsrp_threshold_dbm {
            return SdtType::None;
        }

        // Selection between CG-SDT and RA-SDT
        if self.config.cg_configured && self.is_ta_valid() {
            SdtType::CgSdt
        } else if self.config.support_2step_ra {
            SdtType::RaSdt2Step
        } else {
            SdtType::RaSdt4Step
        }
    }

    /// Initiate an SDT transmission with user data payload.
    /// Returns the multiplexed `SdtMacPdu` ready for transmission over the air.
    pub fn initiate_sdt(
        &mut self,
        user_data: &[u8],
        serving_rsrp_dbm: f32,
    ) -> Result<SdtMacPdu, &'static str> {
        let sdt_type = self.evaluate_sdt_criteria(user_data.len(), serving_rsrp_dbm);
        if sdt_type == SdtType::None {
            return Err("SDT criteria not satisfied; fallback to legacy RRC Resume required");
        }

        self.state = SdtProcedureState::Initiated { sdt_type };

        // Construct CCCH SDU (RRCResumeRequest1)
        // Format: [ResumeCause (1B)][I-RNTI (4B)][ShortMAC-I (2B)]
        let mut ccch_sdu = Vec::new();
        ccch_sdu.push(0x01); // mo-Signalling / mo-Data
        ccch_sdu.extend_from_slice(&0xA1B2C3D4u32.to_be_bytes()); // I-RNTI
        ccch_sdu.extend_from_slice(&self.short_mac_i_token.to_be_bytes()); // ShortMAC-I

        let mac_pdu = SdtMacPdu::new(ccch_sdu, user_data.to_vec(), None);

        self.total_signaling_bytes_used += mac_pdu.ccch_sdu.len() + 6; // CCCH + subheaders
        self.state = SdtProcedureState::AwaitingResponse {
            sdt_type,
            transmitted_bytes: user_data.len(),
        };

        Ok(mac_pdu)
    }

    /// Process network response from gNB (TS 38.331).
    pub fn handle_network_response(
        &mut self,
        response: SdtResponseAction,
    ) -> Result<(), &'static str> {
        match response {
            SdtResponseAction::RrcReleaseWithSuspend => {
                let bytes = match self.state {
                    SdtProcedureState::AwaitingResponse {
                        transmitted_bytes, ..
                    } => transmitted_bytes,
                    SdtProcedureState::SubsequentData {
                        total_transferred_bytes,
                        ..
                    } => total_transferred_bytes,
                    _ => return Err("Unexpected RrcRelease in current state"),
                };
                self.total_user_bytes_transferred += bytes;
                self.total_signaling_bytes_used += 12; // RRCRelease payload
                self.state = SdtProcedureState::TerminatedSuccess {
                    total_transferred_bytes: self.total_user_bytes_transferred,
                };
            }
            SdtResponseAction::RrcResume => {
                let bytes = match self.state {
                    SdtProcedureState::AwaitingResponse {
                        transmitted_bytes, ..
                    } => transmitted_bytes,
                    SdtProcedureState::SubsequentData {
                        total_transferred_bytes,
                        ..
                    } => total_transferred_bytes,
                    _ => 0,
                };
                self.total_user_bytes_transferred += bytes;
                self.state = SdtProcedureState::FallbackToConnected {
                    total_transferred_bytes: self.total_user_bytes_transferred,
                };
            }
            SdtResponseAction::RrcReject { wait_time_sec } => {
                self.state = SdtProcedureState::Rejected {
                    backoff_ms: wait_time_sec as u32 * 1000,
                };
            }
            SdtResponseAction::DynamicGrant { granted_bytes: _ } => {
                // Subsequent data transfer triggered
                let prev_bytes = match self.state {
                    SdtProcedureState::AwaitingResponse {
                        transmitted_bytes, ..
                    } => transmitted_bytes,
                    SdtProcedureState::SubsequentData {
                        total_transferred_bytes,
                        ..
                    } => total_transferred_bytes,
                    _ => return Err("Dynamic grant received in invalid state"),
                };

                self.state = SdtProcedureState::SubsequentData {
                    remaining_inactivity_ms: self.config.inactivity_timer_ms,
                    total_transferred_bytes: prev_bytes,
                };
            }
        }
        Ok(())
    }

    /// Transmit subsequent data under an active subsequent grant.
    pub fn transmit_subsequent_data(&mut self, payload: &[u8]) -> Result<Vec<u8>, &'static str> {
        match &mut self.state {
            SdtProcedureState::SubsequentData {
                remaining_inactivity_ms,
                total_transferred_bytes,
            } => {
                // Reset inactivity timer
                *remaining_inactivity_ms = self.config.inactivity_timer_ms;
                *total_transferred_bytes += payload.len();

                // Generate DTCH packet
                let mut pdu = Vec::new();
                pdu.push(MAC_LCID_DTCH_MIN);
                let len = payload.len() as u16;
                pdu.extend_from_slice(&len.to_be_bytes());
                pdu.extend_from_slice(payload);
                Ok(pdu)
            }
            _ => Err("Cannot transmit subsequent data: not in SubsequentData state"),
        }
    }

    /// Advance elapsed time in milliseconds.
    /// Handles TA timer decrement and SDT inactivity timer expiry.
    pub fn advance_time_ms(&mut self, elapsed_ms: u32) {
        if self.ta_timer_remaining_ms > 0 {
            self.ta_timer_remaining_ms = self.ta_timer_remaining_ms.saturating_sub(elapsed_ms);
        }

        if let SdtProcedureState::SubsequentData {
            remaining_inactivity_ms,
            total_transferred_bytes,
        } = self.state
        {
            if elapsed_ms >= remaining_inactivity_ms {
                // Inactivity timer expired: conclude SDT autonomously
                self.total_user_bytes_transferred += total_transferred_bytes;
                self.state = SdtProcedureState::TerminatedSuccess {
                    total_transferred_bytes: self.total_user_bytes_transferred,
                };
            } else {
                self.state = SdtProcedureState::SubsequentData {
                    remaining_inactivity_ms: remaining_inactivity_ms - elapsed_ms,
                    total_transferred_bytes,
                };
            }
        }
    }

    /// Reset engine back to InactiveStandby mode.
    pub fn reset_to_inactive(&mut self) {
        self.state = SdtProcedureState::InactiveStandby;
    }

    /// Calculate performance and energy savings compared to legacy RRC Resume.
    pub fn compute_performance_metrics(&self) -> SdtPerformanceMetrics {
        // Legacy RRC Resume procedure requires:
        // 1. RRCResumeRequest: 32 bytes
        // 2. RRCResume: 48 bytes
        // 3. RRCResumeComplete: 28 bytes
        // 4. Security setup & RRCRelease: 40 bytes
        // Total legacy control signaling ~ 148 bytes per session.
        let legacy_signaling_bytes = 148;
        let sdt_signaling_bytes = self.total_signaling_bytes_used.max(20);

        let reduction_pct = if legacy_signaling_bytes > sdt_signaling_bytes {
            ((legacy_signaling_bytes - sdt_signaling_bytes) as f32 / legacy_signaling_bytes as f32)
                * 100.0
        } else {
            0.0
        };

        // Energy consumption model:
        // Connected mode full RACH + 3-way RRC handshake takes ~180 ms @ 400 mW = ~72 mJ.
        // SDT single-shot transmission takes ~25 ms @ 350 mW = ~8.75 mJ.
        let energy_consumed_mj = (self.total_user_bytes_transferred as f32 * 0.05) + 8.75;
        let legacy_energy_mj = (self.total_user_bytes_transferred as f32 * 0.05) + 72.0;
        let energy_saved_mj = (legacy_energy_mj - energy_consumed_mj).max(0.0);

        SdtPerformanceMetrics {
            user_data_bytes_transferred: self.total_user_bytes_transferred,
            sdt_signaling_overhead_bytes: sdt_signaling_bytes,
            legacy_signaling_overhead_bytes: legacy_signaling_bytes,
            signaling_reduction_percentage: reduction_pct,
            estimated_energy_consumed_mj: energy_consumed_mj,
            estimated_energy_saved_mj: energy_saved_mj,
        }
    }
}
