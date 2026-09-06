//! 3GPP Rel-17 TS 23.501 §5.27.1 / TS 24.519 / IEEE 802.1AS 5G-TSN Time-Synchronization Service Function (TSCTF).
//!
//! Implements 5G-TSN time synchronization and working clock management:
//! - Multi-Domain Clock Support: Universal 5G Time (Domain 0) and multiple TSN Working Clocks (Domain 1..255).
//! - 5G System (5GS) time to TSN Working Clock translation with affine rate-ratio compensation.
//! - 3GPP Reference Time Information generation and distribution (SIB9 / dedicated RRC signaling).
//! - Time Error (TE) budget allocation and audit across DS-TT, 5G Uu radio interface, 5GC, and NW-TT.
//! - IEEE 802.1AS / 1588-2019 PTP residence time calculation and 16-bit fractional `correctionField` update.
//! - Rel-17 UE-to-UE time synchronization over 5GS without leaving the cellular network.

use std::collections::HashMap;
use std::fmt;

use crate::ptp::PtpHeader;

/// 3GPP TS 22.104 standard time error budget for industrial automation (900 ns).
pub const DEFAULT_INDUSTRIAL_TSN_BUDGET_NS: f64 = 900.0;

/// 3GPP TS 22.104 strict motion control time error budget (250 ns).
pub const STRICT_MOTION_CONTROL_BUDGET_NS: f64 = 250.0;

/// Nanoseconds per second constant.
pub const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

/// Errors raised during 5G-TSN time synchronization and clock conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TsctfError {
    DomainNotFound(u8),
    SessionAlreadyExists(u8),
    InvalidTimeConversion(&'static str),
    TimeErrorBudgetExceeded { total_ns: u64, max_budget_ns: u64 },
    InvalidPtpMessage(&'static str),
    DsTtNotFound(u32),
    NwTtNotFound(u32),
    NegativeResidenceTime { ingress_ns: u64, egress_ns: u64 },
}

impl fmt::Display for TsctfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsctfError::DomainNotFound(d) => write!(f, "TSN Clock Domain {} not found in TSCTF", d),
            TsctfError::SessionAlreadyExists(d) => {
                write!(f, "TSN Clock Domain {} already registered", d)
            }
            TsctfError::InvalidTimeConversion(msg) => write!(f, "Invalid time conversion: {}", msg),
            TsctfError::TimeErrorBudgetExceeded {
                total_ns,
                max_budget_ns,
            } => write!(
                f,
                "Time error {} ns exceeds 5GS budget {} ns",
                total_ns, max_budget_ns
            ),
            TsctfError::InvalidPtpMessage(msg) => write!(f, "Invalid PTP message: {}", msg),
            TsctfError::DsTtNotFound(port) => write!(f, "DS-TT port {} not found", port),
            TsctfError::NwTtNotFound(port) => write!(f, "NW-TT port {} not found", port),
            TsctfError::NegativeResidenceTime {
                ingress_ns,
                egress_ns,
            } => write!(
                f,
                "Egress timestamp {} ns precedes ingress timestamp {} ns",
                egress_ns, ingress_ns
            ),
        }
    }
}

impl std::error::Error for TsctfError {}

/// Synchronization flow direction between external TSN network and 5GS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDirection {
    /// External TSN Grandmaster is on the network side (connected to NW-TT).
    DownlinkFromNwTt,
    /// External TSN Grandmaster is on the device side (connected to DS-TT).
    UplinkFromDsTt,
    /// Direct UE-to-UE synchronization over 5GS without external GM.
    UeToUeDirect,
}

/// Type of time synchronization clock domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClockDomainType {
    /// Universal 5G Internal Time (Domain 0).
    Universal5GTime,
    /// IEEE 802.1AS TSN Working Clock domain for industrial cell / deterministic application.
    TsnWorkingClock,
}

/// 3GPP TS 38.331 ReferenceTimeInfo broadcasted in SIB9 or unicast via dedicated RRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceTimeInfo {
    /// System Frame Number (0..1023).
    pub sfn: u16,
    /// Subframe Number (0..9).
    pub subframe: u8,
    /// Slot Number within subframe (0..31 depending on numerology).
    pub slot: u8,
    /// 5G Universal Time epoch in whole seconds.
    pub time_seconds: u64,
    /// Fractional seconds in nanoseconds (0..999,999,999).
    pub time_nanoseconds: u32,
    /// Synchronization uncertainty bound in nanoseconds.
    pub uncertainty_ns: u32,
}

impl ReferenceTimeInfo {
    pub fn new(
        sfn: u16,
        subframe: u8,
        slot: u8,
        time_seconds: u64,
        time_nanoseconds: u32,
        uncertainty_ns: u32,
    ) -> Self {
        Self {
            sfn,
            subframe,
            slot,
            time_seconds,
            time_nanoseconds,
            uncertainty_ns,
        }
    }

    /// Converts into a continuous 64-bit nanosecond timestamp.
    pub fn to_5g_epoch_ns(&self) -> u64 {
        self.time_seconds * NANOSECONDS_PER_SECOND + (self.time_nanoseconds as u64)
    }

    /// Extrapolates current 5G system time based on elapsed duration since reference.
    pub fn extrapolate_current_5g_ns(&self, elapsed_ns: u64) -> u64 {
        self.to_5g_epoch_ns().saturating_add(elapsed_ns)
    }
}

/// Mathematical model for converting between 5G internal time and TSN Working Clock time.
///
/// Implements affine transformation with rate-ratio drift tracking:
/// $$T_{working}(t) = T_{ref,working} + (1.0 + \text{rate\_ratio\_ppm} \times 10^{-6}) \times (t_{5G} - t_{ref,5G})$$
#[derive(Debug, Clone, PartialEq)]
pub struct WorkingClockModel {
    /// Reference calibration timestamp in 5G system time (ns).
    pub ref_5g_ns: u64,
    /// Corresponding reference timestamp in TSN Working Clock time (ns).
    pub ref_working_ns: u64,
    /// Relative frequency drift rate offset in parts-per-million (ppm).
    pub rate_offset_ppm: f64,
}

impl WorkingClockModel {
    pub fn new(ref_5g_ns: u64, ref_working_ns: u64, rate_offset_ppm: f64) -> Self {
        Self {
            ref_5g_ns,
            ref_working_ns,
            rate_offset_ppm,
        }
    }

    /// Default 1:1 synchronization model with zero frequency offset.
    pub fn aligned(ref_epoch_ns: u64) -> Self {
        Self {
            ref_5g_ns: ref_epoch_ns,
            ref_working_ns: ref_epoch_ns,
            rate_offset_ppm: 0.0,
        }
    }

    /// Converts 5G internal system time into TSN Working Clock time.
    pub fn convert_5g_to_working(&self, t_5g_ns: u64) -> u64 {
        if t_5g_ns >= self.ref_5g_ns {
            let delta_5g = (t_5g_ns - self.ref_5g_ns) as i128;
            let drift_ns = if self.rate_offset_ppm == 0.0 {
                0i128
            } else {
                ((delta_5g as f64) * (self.rate_offset_ppm * 1e-6)).round() as i128
            };
            let delta_working = delta_5g + drift_ns;
            (self.ref_working_ns as i128 + delta_working).max(0) as u64
        } else {
            let delta_5g = (self.ref_5g_ns - t_5g_ns) as i128;
            let drift_ns = if self.rate_offset_ppm == 0.0 {
                0i128
            } else {
                ((delta_5g as f64) * (self.rate_offset_ppm * 1e-6)).round() as i128
            };
            let delta_working = delta_5g + drift_ns;
            (self.ref_working_ns as i128 - delta_working).max(0) as u64
        }
    }

    /// Converts TSN Working Clock time into 5G internal system time.
    pub fn convert_working_to_5g(&self, t_working_ns: u64) -> u64 {
        if t_working_ns >= self.ref_working_ns {
            let delta_working = (t_working_ns - self.ref_working_ns) as i128;
            let drift_ns = if self.rate_offset_ppm == 0.0 {
                0i128
            } else {
                let ppm_scale = self.rate_offset_ppm * 1e-6;
                ((delta_working as f64) * (ppm_scale / (1.0 + ppm_scale))).round() as i128
            };
            let delta_5g = delta_working - drift_ns;
            (self.ref_5g_ns as i128 + delta_5g).max(0) as u64
        } else {
            let delta_working = (self.ref_working_ns - t_working_ns) as i128;
            let drift_ns = if self.rate_offset_ppm == 0.0 {
                0i128
            } else {
                let ppm_scale = self.rate_offset_ppm * 1e-6;
                ((delta_working as f64) * (ppm_scale / (1.0 + ppm_scale))).round() as i128
            };
            let delta_5g = delta_working - drift_ns;
            (self.ref_5g_ns as i128 - delta_5g).max(0) as u64
        }
    }

    /// Updates calibration pair and calculates new drift rate ratio in ppm.
    pub fn update_calibration(&mut self, new_5g_ns: u64, new_working_ns: u64) {
        if new_5g_ns > self.ref_5g_ns && new_working_ns > self.ref_working_ns {
            let delta_5g = (new_5g_ns - self.ref_5g_ns) as f64;
            let delta_working = (new_working_ns - self.ref_working_ns) as f64;
            if delta_5g > 0.0 {
                let ratio = delta_working / delta_5g;
                // rate_offset_ppm = (ratio - 1.0) * 1e6
                self.rate_offset_ppm = (ratio - 1.0) * 1e6;
            }
        }
        self.ref_5g_ns = new_5g_ns;
        self.ref_working_ns = new_working_ns;
    }
}

/// 3GPP TS 22.104 / TS 23.501 §5.27.1.2 Time Error (TE) budget allocation across 5GS.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeErrorBudget {
    /// DS-TT hardware timestamping jitter and error (ns).
    pub ds_tt_error_ns: f64,
    /// 5G Uu radio interface transmission jitter (ns).
    pub uu_radio_jitter_ns: f64,
    /// 5G Core (UPF) internal packet forwarding delay variation (ns).
    pub upf_transit_error_ns: f64,
    /// NW-TT hardware timestamping jitter and error (ns).
    pub nw_tt_error_ns: f64,
    /// Maximum allowed end-to-end 5GS time error budget (ns).
    pub max_budget_ns: f64,
}

impl TimeErrorBudget {
    pub fn new(
        ds_tt_error_ns: f64,
        uu_radio_jitter_ns: f64,
        upf_transit_error_ns: f64,
        nw_tt_error_ns: f64,
        max_budget_ns: f64,
    ) -> Self {
        Self {
            ds_tt_error_ns,
            uu_radio_jitter_ns,
            upf_transit_error_ns,
            nw_tt_error_ns,
            max_budget_ns,
        }
    }

    /// Default industrial profile conforming to TS 22.104 (900 ns).
    pub fn industrial_default() -> Self {
        Self {
            ds_tt_error_ns: 80.0,
            uu_radio_jitter_ns: 400.0,
            upf_transit_error_ns: 200.0,
            nw_tt_error_ns: 80.0,
            max_budget_ns: DEFAULT_INDUSTRIAL_TSN_BUDGET_NS,
        }
    }

    /// Strict motion control profile (250 ns budget).
    pub fn strict_motion_control() -> Self {
        Self {
            ds_tt_error_ns: 30.0,
            uu_radio_jitter_ns: 120.0,
            upf_transit_error_ns: 60.0,
            nw_tt_error_ns: 30.0,
            max_budget_ns: STRICT_MOTION_CONTROL_BUDGET_NS,
        }
    }

    /// Total cumulative time error across all 5GS components.
    pub fn total_time_error_ns(&self) -> f64 {
        self.ds_tt_error_ns
            + self.uu_radio_jitter_ns
            + self.upf_transit_error_ns
            + self.nw_tt_error_ns
    }

    /// Returns true if the cumulative time error is within the configured budget.
    pub fn is_compliant(&self) -> bool {
        self.total_time_error_ns() <= self.max_budget_ns
    }

    /// Validates budget compliance, returning an error if out of specification.
    pub fn audit(&self) -> Result<f64, TsctfError> {
        let total = self.total_time_error_ns();
        if total <= self.max_budget_ns {
            Ok(total)
        } else {
            Err(TsctfError::TimeErrorBudgetExceeded {
                total_ns: total.round() as u64,
                max_budget_ns: self.max_budget_ns.round() as u64,
            })
        }
    }
}

/// Detailed result of IEEE 802.1AS PTP residence time calculation across 5GS virtual bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtpResidenceTimeUpdate {
    pub domain_id: u8,
    pub ingress_port: u32,
    pub egress_port: u32,
    pub t_in_working_ns: u64,
    pub t_out_working_ns: u64,
    pub residence_time_ns: u64,
    pub ingress_port_delay_ns: u64,
    pub egress_port_delay_ns: u64,
    pub total_correction_ns: u64,
    pub original_correction_field: i64,
    pub updated_correction_field: i64,
}

impl PtpResidenceTimeUpdate {
    /// Applies the residence time correction directly to a PTP packet header.
    ///
    /// IEEE 802.1AS / IEEE 1588-2019:
    /// `correctionField` is a 64-bit integer representing nanoseconds multiplied by $2^{16}$ (shifted left by 16 bits).
    pub fn apply_to_ptp_header(&self, ptp_header: &mut PtpHeader) {
        ptp_header.correction_field = self.updated_correction_field;
    }
}

/// Performance telemetry for a Rel-17 direct UE-to-UE synchronization exchange.
#[derive(Debug, Clone, PartialEq)]
pub struct UeToUeSyncReport {
    pub domain_id: u8,
    pub source_ds_tt: u32,
    pub target_ds_tt: u32,
    pub source_working_time_ns: u64,
    pub target_working_time_ns: u64,
    pub transit_delay_5g_ns: u64,
    pub estimated_sync_error_ns: f64,
    pub within_rel17_sla: bool,
}

/// Time-Synchronization Session managing a single TSN Clock Domain in TSCTF.
#[derive(Debug, Clone)]
pub struct TsctfSession {
    pub domain_id: u8,
    pub domain_type: ClockDomainType,
    pub direction: SyncDirection,
    pub working_clock: WorkingClockModel,
    pub budget: TimeErrorBudget,
    pub nw_tt_port: u32,
    pub connected_ds_tts: Vec<u32>,
    pub sync_sequence_counter: u16,
}

impl TsctfSession {
    pub fn new(
        domain_id: u8,
        domain_type: ClockDomainType,
        direction: SyncDirection,
        working_clock: WorkingClockModel,
        budget: TimeErrorBudget,
        nw_tt_port: u32,
    ) -> Self {
        Self {
            domain_id,
            domain_type,
            direction,
            working_clock,
            budget,
            nw_tt_port,
            connected_ds_tts: Vec::new(),
            sync_sequence_counter: 0,
        }
    }

    pub fn add_ds_tt(&mut self, ds_tt_port: u32) {
        if !self.connected_ds_tts.contains(&ds_tt_port) {
            self.connected_ds_tts.push(ds_tt_port);
        }
    }

    /// Processes an incoming PTP Sync / Follow_Up message across the 5GS virtual bridge,
    /// computing residence time in Working Clock domain and updating the PTP `correctionField`.
    pub fn process_ptp_forward(
        &mut self,
        ingress_port: u32,
        egress_port: u32,
        t_in_5g_ns: u64,
        t_out_5g_ns: u64,
        ingress_port_delay_ns: u64,
        egress_port_delay_ns: u64,
        ptp_header: &mut PtpHeader,
    ) -> Result<PtpResidenceTimeUpdate, TsctfError> {
        if t_out_5g_ns < t_in_5g_ns {
            return Err(TsctfError::NegativeResidenceTime {
                ingress_ns: t_in_5g_ns,
                egress_ns: t_out_5g_ns,
            });
        }

        // Convert ingress and egress timestamps to the TSN Working Clock domain
        let t_in_working_ns = self.working_clock.convert_5g_to_working(t_in_5g_ns);
        let t_out_working_ns = self.working_clock.convert_5g_to_working(t_out_5g_ns);

        let residence_time_ns = t_out_working_ns.saturating_sub(t_in_working_ns);
        let total_correction_ns = residence_time_ns + ingress_port_delay_ns + egress_port_delay_ns;

        let original_correction_field = ptp_header.correction_field;
        // IEEE 802.1AS: correctionField += (correction_ns << 16)
        let delta_scaled = (total_correction_ns as i64) << 16;
        let updated_correction_field = original_correction_field.saturating_add(delta_scaled);

        ptp_header.correction_field = updated_correction_field;
        ptp_header.domain_number = self.domain_id;
        self.sync_sequence_counter = self.sync_sequence_counter.wrapping_add(1);

        Ok(PtpResidenceTimeUpdate {
            domain_id: self.domain_id,
            ingress_port,
            egress_port,
            t_in_working_ns,
            t_out_working_ns,
            residence_time_ns,
            ingress_port_delay_ns,
            egress_port_delay_ns,
            total_correction_ns,
            original_correction_field,
            updated_correction_field,
        })
    }
}

/// Central 5G-TSN Time-Synchronization Service Function (TSCTF Engine).
#[derive(Debug, Clone)]
pub struct TsctfEngine {
    sessions: HashMap<u8, TsctfSession>,
    latest_ref_time: Option<ReferenceTimeInfo>,
}

impl TsctfEngine {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            latest_ref_time: None,
        }
    }

    /// Registers a new TSN Clock Domain session.
    pub fn register_session(&mut self, session: TsctfSession) -> Result<(), TsctfError> {
        if self.sessions.contains_key(&session.domain_id) {
            return Err(TsctfError::SessionAlreadyExists(session.domain_id));
        }
        self.sessions.insert(session.domain_id, session);
        Ok(())
    }

    /// Retrieves an active session by domain ID.
    pub fn get_session(&self, domain_id: u8) -> Option<&TsctfSession> {
        self.sessions.get(&domain_id)
    }

    /// Retrieves a mutable reference to an active session by domain ID.
    pub fn get_session_mut(&mut self, domain_id: u8) -> Option<&mut TsctfSession> {
        self.sessions.get_mut(&domain_id)
    }

    /// Sets the current 3GPP SIB9 / RRC Reference Time Information.
    pub fn update_reference_time(&mut self, ref_time: ReferenceTimeInfo) {
        self.latest_ref_time = Some(ref_time);
    }

    pub fn get_reference_time(&self) -> Option<ReferenceTimeInfo> {
        self.latest_ref_time
    }

    /// Simulates SIB9 broadcast distribution to a connected DS-TT.
    /// Reconstructs the TSN Working Clock time at the device side.
    pub fn distribute_working_clock_to_ds_tt(
        &self,
        domain_id: u8,
        ds_tt_port: u32,
        elapsed_5g_ns: u64,
    ) -> Result<u64, TsctfError> {
        let session = self
            .sessions
            .get(&domain_id)
            .ok_or(TsctfError::DomainNotFound(domain_id))?;

        if !session.connected_ds_tts.contains(&ds_tt_port) {
            return Err(TsctfError::DsTtNotFound(ds_tt_port));
        }

        let ref_info = self
            .latest_ref_time
            .ok_or(TsctfError::InvalidTimeConversion(
                "No ReferenceTimeInfo available",
            ))?;

        let current_5g_time_ns = ref_info.extrapolate_current_5g_ns(elapsed_5g_ns);
        let reconstructed_working_ns = session
            .working_clock
            .convert_5g_to_working(current_5g_time_ns);

        Ok(reconstructed_working_ns)
    }

    /// Executes a Rel-17 direct UE-to-UE time synchronization between two DS-TTs over 5GS.
    pub fn perform_ue_to_ue_sync(
        &self,
        domain_id: u8,
        source_ds_tt: u32,
        target_ds_tt: u32,
        source_working_time_ns: u64,
        transit_delay_5g_ns: u64,
    ) -> Result<UeToUeSyncReport, TsctfError> {
        let session = self
            .sessions
            .get(&domain_id)
            .ok_or(TsctfError::DomainNotFound(domain_id))?;

        if !session.connected_ds_tts.contains(&source_ds_tt) {
            return Err(TsctfError::DsTtNotFound(source_ds_tt));
        }
        if !session.connected_ds_tts.contains(&target_ds_tt) {
            return Err(TsctfError::DsTtNotFound(target_ds_tt));
        }

        // Convert source working clock to 5G time
        let source_5g_time_ns = session
            .working_clock
            .convert_working_to_5g(source_working_time_ns);
        // Add 5G transit delay
        let arrival_5g_time_ns = source_5g_time_ns.saturating_add(transit_delay_5g_ns);
        // Convert back to target working clock time
        let target_working_time_ns = session
            .working_clock
            .convert_5g_to_working(arrival_5g_time_ns);

        // Estimate end-to-end sync error based on session budget
        let estimated_sync_error_ns = session.budget.total_time_error_ns();
        let within_rel17_sla = estimated_sync_error_ns <= session.budget.max_budget_ns;

        Ok(UeToUeSyncReport {
            domain_id,
            source_ds_tt,
            target_ds_tt,
            source_working_time_ns,
            target_working_time_ns,
            transit_delay_5g_ns,
            estimated_sync_error_ns,
            within_rel17_sla,
        })
    }
}
