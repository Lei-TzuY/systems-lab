//! 3GPP Rel-17 5G NR Sidelink (SL) Discontinuous Reception (DRX) & Inter-UE Coordination (IUC) Engine.
//!
//! Compliant with:
//! - 3GPP TS 38.321 Rel-17 Section 5.28 ("SL DRX")
//! - 3GPP TS 38.331 Rel-17 (`SL-DRX-ConfigGC-BC`, `SL-DRX-ConfigUC`, and `SL-DRX-QoS-Mapping`)
//! - 3GPP TS 38.214 Rel-17 Section 8.1.4 ("Sidelink Resource Allocation & Inter-UE Coordination")
//! - 3GPP TS 38.212 Rel-17 Section 8.4 (SCI Format 2-C for Coordination Feedback)
//!
//! Implements:
//! 1. Multi-session Sidelink DRX (Unicast, Groupcast, and Broadcast) with configurable cycles,
//!    start offsets, `onDuration`, `inactivity`, and per-HARQ process RTT / retransmission timers.
//! 2. Sidelink Active Time arbitration across all sessions and transmission queues.
//! 3. Partial Sensing coordination for power-constrained UEs (e.g. Pedestrians/VRUs).
//! 4. Rel-17 Inter-UE Coordination (IUC):
//!    - Scheme 1: Preferred and Non-preferred candidate resource recommendation & exclusion.
//!    - Scheme 2: Conflict & collision detection with proactive alert generation.
//! 5. Battery conservation, duty-cycle tracking, and performance telemetry.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ---------------------------------------------------------------------------
// Enums & Basic Types
// ---------------------------------------------------------------------------

/// Sidelink communication cast type for DRX profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlDrxCastType {
    /// Dedicated point-to-point communication with a specific peer UE.
    Unicast,
    /// Point-to-multipoint communication for a vehicular or sensor cluster.
    Groupcast,
    /// Direct omnidirectional broadcast for safety or emergency beacons.
    Broadcast,
}

/// Sidelink DRX Configuration Error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidelinkDrxError {
    InvalidCycleConfig { reason: &'static str },
    ProfileAlreadyExists(u8),
    ProfileNotFound(u8),
    InvalidTimerValue(&'static str),
}

impl fmt::Display for SidelinkDrxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SidelinkDrxError::InvalidCycleConfig { reason } => {
                write!(f, "Invalid SL DRX cycle configuration: {}", reason)
            }
            SidelinkDrxError::ProfileAlreadyExists(id) => {
                write!(f, "SL DRX Profile with ID {} already exists", id)
            }
            SidelinkDrxError::ProfileNotFound(id) => {
                write!(f, "SL DRX Profile with ID {} not found", id)
            }
            SidelinkDrxError::InvalidTimerValue(t) => {
                write!(f, "Invalid timer value for {}", t)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration & Profiles per 3GPP TS 38.331
// ---------------------------------------------------------------------------

/// Configuration for a Sidelink DRX profile (TS 38.331 `SL-DRX-Config`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidelinkDrxProfileConfig {
    /// Unique profile identifier (0..15).
    pub profile_id: u8,
    /// Communication cast type.
    pub cast_type: SlDrxCastType,
    /// Target peer Layer-2 ID (for Unicast: 24-bit L2 ID; None for Broadcast/Groupcast).
    pub peer_ue_id: Option<u32>,
    /// Duration of `sl-drx-onDurationTimer` in slots (e.g. 1..1200 slots).
    pub on_duration_slots: u16,
    /// Duration of `sl-drx-InactivityTimer` in slots (restarted on receiving/sending new SCI).
    pub inactivity_slots: u16,
    /// Duration of `sl-drx-HARQ-RTT-Timer1` in slots before retransmission monitoring.
    pub harq_rtt_slots: u16,
    /// Duration of `sl-drx-RetransmissionTimer` in slots.
    pub retransmission_slots: u16,
    /// SL DRX cycle length in slots (e.g. 40, 80, 160, 320, 640, 1280 slots).
    pub cycle_slots: u32,
    /// Cycle start offset in slots (0 <= offset < cycle_slots).
    pub start_offset_slots: u32,
    /// PC5 QoS Identifiers (PQIs) mapped to this DRX configuration.
    pub pqi_list: Vec<u8>,
}

impl SidelinkDrxProfileConfig {
    /// Validates the profile configuration against 3GPP constraints.
    pub fn validate(&self) -> Result<(), SidelinkDrxError> {
        if self.cycle_slots == 0 {
            return Err(SidelinkDrxError::InvalidCycleConfig {
                reason: "cycle_slots cannot be 0",
            });
        }
        if self.start_offset_slots >= self.cycle_slots {
            return Err(SidelinkDrxError::InvalidCycleConfig {
                reason: "start_offset_slots must be less than cycle_slots",
            });
        }
        if self.on_duration_slots == 0 {
            return Err(SidelinkDrxError::InvalidTimerValue("on_duration_slots"));
        }
        if self.cast_type == SlDrxCastType::Unicast && self.peer_ue_id.is_none() {
            return Err(SidelinkDrxError::InvalidCycleConfig {
                reason: "Unicast SL DRX profile must specify peer_ue_id",
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// HARQ State and DRX Session
// ---------------------------------------------------------------------------

/// Per-HARQ process state tracking for Sidelink retransmissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidelinkHarqProcessState {
    pub process_id: u8,
    /// Remaining slots on `sl-drx-HARQ-RTT-Timer`.
    pub rtt_timer_remaining: u16,
    /// Remaining slots on `sl-drx-RetransmissionTimer`.
    pub retrans_timer_remaining: u16,
    /// Whether this process is waiting for ACK/NACK feedback on PSFCH.
    pub awaiting_feedback: bool,
}

/// Dynamic state for an active Sidelink DRX session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidelinkDrxSession {
    pub config: SidelinkDrxProfileConfig,
    /// Active countdown for `sl-drx-onDurationTimer`.
    pub on_duration_remaining: u16,
    /// Active countdown for `sl-drx-InactivityTimer`.
    pub inactivity_remaining: u16,
    /// Per-HARQ process timer tracking (key = harq_process_id).
    pub harq_processes: HashMap<u8, SidelinkHarqProcessState>,
    /// Whether this individual session requires Active Time on the current slot.
    pub is_session_active: bool,
}

impl SidelinkDrxSession {
    pub fn new(config: SidelinkDrxProfileConfig) -> Self {
        Self {
            config,
            on_duration_remaining: 0,
            inactivity_remaining: 0,
            harq_processes: HashMap::new(),
            is_session_active: false,
        }
    }

    /// Evaluates if a new DRX cycle begins on the given slot.
    #[inline]
    pub fn is_cycle_start(&self, slot: u64) -> bool {
        (slot % (self.config.cycle_slots as u64)) == (self.config.start_offset_slots as u64)
    }

    /// Advances the session timers by 1 slot.
    pub fn advance_slot(&mut self, slot: u64) {
        // Check cycle trigger
        if self.is_cycle_start(slot) {
            self.on_duration_remaining = self.config.on_duration_slots;
        } else if self.on_duration_remaining > 0 {
            self.on_duration_remaining -= 1;
        }

        // Advance inactivity timer
        if self.inactivity_remaining > 0 {
            self.inactivity_remaining -= 1;
        }

        // Advance HARQ process timers
        for state in self.harq_processes.values_mut() {
            if state.rtt_timer_remaining > 0 {
                state.rtt_timer_remaining -= 1;
                if state.rtt_timer_remaining == 0 && state.awaiting_feedback {
                    // Start retransmission timer when RTT timer expires
                    state.retrans_timer_remaining = self.config.retransmission_slots;
                }
            } else if state.retrans_timer_remaining > 0 {
                state.retrans_timer_remaining -= 1;
                if state.retrans_timer_remaining == 0 {
                    state.awaiting_feedback = false;
                }
            }
        }

        // Clean up idle HARQ processes
        self.harq_processes.retain(|_, s| {
            s.rtt_timer_remaining > 0 || s.retrans_timer_remaining > 0 || s.awaiting_feedback
        });

        // Determine if this session is active
        self.is_session_active = self.is_active();
    }

    /// Evaluates if this session is currently active (onDuration, inactivity, or retrans timer running).
    #[inline]
    pub fn is_active(&self) -> bool {
        self.on_duration_remaining > 0
            || self.inactivity_remaining > 0
            || self
                .harq_processes
                .values()
                .any(|s| s.retrans_timer_remaining > 0)
    }
}

// ---------------------------------------------------------------------------
// Partial Sensing Coordination (TS 38.214 §8.1.4)
// ---------------------------------------------------------------------------

/// Configuration for Sidelink Partial Sensing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialSensingConfig {
    /// Minimum periodic reservation gap to track in slots (e.g. 100 slots).
    pub periodic_step_slots: u16,
    /// Number of contiguous sensing slots prior to DRX onDuration window (e.g. 2..10 slots).
    pub contiguous_sensing_slots: u16,
    /// Periodic candidate sensing occurrences (e.g. at T - k * P).
    pub periodic_sensing_depth: u8,
}

// ---------------------------------------------------------------------------
// Inter-UE Coordination (IUC) Types (TS 38.214 §8.1.4 & TS 38.212 §8.4)
// ---------------------------------------------------------------------------

/// Time-frequency resource block in Sidelink resource pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceSlotBlock {
    /// Slot index.
    pub slot: u64,
    /// Starting sub-channel index.
    pub subchannel_index: u8,
    /// Number of contiguous sub-channels.
    pub num_subchannels: u8,
    /// Sidelink RSRP measurement in dBm (e.g. -110..-40 dBm).
    pub rsrp_dbm: i16,
    /// Priority level (0..7, where 0 is highest priority).
    pub priority: u8,
}

/// Inter-UE Coordination Scheme per 3GPP Rel-17.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinationSchemeType {
    /// Scheme 1: Preferred resource set recommendation (low interference/collision).
    Scheme1Preferred,
    /// Scheme 1: Non-preferred resource set exclusion (conflicts or heavy interference).
    Scheme1NonPreferred,
    /// Scheme 2: Conflict & collision notification.
    Scheme2ConflictAlert,
}

/// Inter-UE Coordination Message (carried over SCI 2-C / MAC CE / RRC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterUeCoordinationMessage {
    /// Layer-2 ID of the sending UE.
    pub sender_l2_id: u32,
    /// Layer-2 ID of the target UE.
    pub target_l2_id: u32,
    /// Scheme type.
    pub scheme_type: CoordinationSchemeType,
    /// List of resource blocks associated with this coordination report.
    pub resources: Vec<ResourceSlotBlock>,
    /// Slot timestamp when the report was generated.
    pub timestamp_slot: u64,
}

// ---------------------------------------------------------------------------
// Telemetry & Metrics
// ---------------------------------------------------------------------------

/// Real-time Sidelink DRX power-saving and coordination metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct SidelinkDrxTelemetry {
    pub total_slots: u64,
    pub active_slots: u64,
    pub sleep_slots: u64,
    pub duty_cycle_percent: f64,
    pub power_saving_percent: f64,
    pub num_wakeups: u64,
    pub sci_rx_count: u64,
    pub sci_tx_count: u64,
    pub iuc_sent_count: u64,
    pub iuc_received_count: u64,
    pub collisions_avoided_count: u64,
}

impl Default for SidelinkDrxTelemetry {
    fn default() -> Self {
        Self {
            total_slots: 0,
            active_slots: 0,
            sleep_slots: 0,
            duty_cycle_percent: 0.0,
            power_saving_percent: 100.0,
            num_wakeups: 0,
            sci_rx_count: 0,
            sci_tx_count: 0,
            iuc_sent_count: 0,
            iuc_received_count: 0,
            collisions_avoided_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Main Sidelink DRX & Inter-UE Coordination Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 Sidelink DRX and Inter-UE Coordination Engine.
#[derive(Debug)]
pub struct SidelinkDrxEngine {
    /// Local UE 24-bit Layer-2 ID.
    local_ue_id: u32,
    /// Current radio slot index.
    current_slot: u64,
    /// Active Sidelink DRX sessions (key = profile_id).
    sessions: HashMap<u8, SidelinkDrxSession>,
    /// Optional Partial Sensing configuration.
    partial_sensing_config: Option<PartialSensingConfig>,
    /// Cached preferred resources received from peer UEs (Scheme 1).
    preferred_resources: Vec<ResourceSlotBlock>,
    /// Cached non-preferred resources received from peer UEs (Scheme 1).
    non_preferred_resources: Vec<ResourceSlotBlock>,
    /// Active collision warnings (Scheme 2).
    collision_alerts: Vec<InterUeCoordinationMessage>,
    /// Pending transmission grant counter (keeps UE active if > 0).
    pending_grants: u16,
    /// Previous slot active state to detect wake-up transitions.
    was_active_prev_slot: bool,
    /// Telemetry & analytics.
    telemetry: SidelinkDrxTelemetry,
}

impl SidelinkDrxEngine {
    /// Creates a new Sidelink DRX and Inter-UE Coordination Engine.
    pub fn new(local_ue_id: u32) -> Self {
        Self {
            local_ue_id,
            current_slot: 0,
            sessions: HashMap::new(),
            partial_sensing_config: None,
            preferred_resources: Vec::new(),
            non_preferred_resources: Vec::new(),
            collision_alerts: Vec::new(),
            pending_grants: 0,
            was_active_prev_slot: false,
            telemetry: SidelinkDrxTelemetry::default(),
        }
    }

    /// Returns the local UE's Layer-2 ID.
    #[inline]
    pub fn local_ue_id(&self) -> u32 {
        self.local_ue_id
    }

    /// Returns the current slot index.
    #[inline]
    pub fn current_slot(&self) -> u64 {
        self.current_slot
    }

    /// Registers a new Sidelink DRX profile.
    pub fn add_profile(
        &mut self,
        config: SidelinkDrxProfileConfig,
    ) -> Result<(), SidelinkDrxError> {
        config.validate()?;
        if self.sessions.contains_key(&config.profile_id) {
            return Err(SidelinkDrxError::ProfileAlreadyExists(config.profile_id));
        }
        let profile_id = config.profile_id;
        self.sessions
            .insert(profile_id, SidelinkDrxSession::new(config));
        Ok(())
    }

    /// Removes a Sidelink DRX profile by ID.
    pub fn remove_profile(&mut self, profile_id: u8) -> Result<(), SidelinkDrxError> {
        if self.sessions.remove(&profile_id).is_some() {
            Ok(())
        } else {
            Err(SidelinkDrxError::ProfileNotFound(profile_id))
        }
    }

    /// Returns a reference to a profile session.
    pub fn get_session(&self, profile_id: u8) -> Option<&SidelinkDrxSession> {
        self.sessions.get(&profile_id)
    }

    /// Returns a mutable reference to a profile session.
    pub fn get_session_mut(&mut self, profile_id: u8) -> Option<&mut SidelinkDrxSession> {
        self.sessions.get_mut(&profile_id)
    }

    /// Configures Partial Sensing for battery conservation.
    pub fn configure_partial_sensing(&mut self, config: PartialSensingConfig) {
        self.partial_sensing_config = Some(config);
    }

    /// Sets the number of pending transmission grants or scheduling requests.
    pub fn set_pending_grants(&mut self, grants: u16) {
        self.pending_grants = grants;
    }

    /// Evaluates if the UE is currently in Sidelink Active Time (TS 38.321 §5.28).
    ///
    /// Active Time is true if:
    /// 1. Any session has `sl-drx-onDurationTimer` running.
    /// 2. Any session has `sl-drx-InactivityTimer` running.
    /// 3. Any session has `sl-drx-RetransmissionTimer` running.
    /// 4. Pending transmission grant / SR > 0.
    /// 5. Partial sensing schedule requires RF wake on the current slot.
    pub fn is_in_active_time(&self) -> bool {
        if self.pending_grants > 0 {
            return true;
        }

        let any_session_active = self.sessions.values().any(|s| s.is_active());
        if any_session_active {
            return true;
        }

        if self.is_partial_sensing_slot() {
            return true;
        }

        false
    }

    /// Evaluates if the current slot is scheduled for partial sensing.
    pub fn is_partial_sensing_slot(&self) -> bool {
        let config = match &self.partial_sensing_config {
            Some(cfg) => cfg,
            None => return false,
        };

        // Check each profile's upcoming onDuration
        for session in self.sessions.values() {
            let cycle = session.config.cycle_slots as u64;
            let offset = session.config.start_offset_slots as u64;
            let current = self.current_slot;

            // Distance to next cycle start
            let current_mod = current % cycle;
            let slots_to_next_on_duration = if current_mod <= offset {
                offset - current_mod
            } else {
                cycle + offset - current_mod
            };

            // Contiguous sensing window immediately preceding onDuration
            if slots_to_next_on_duration <= config.contiguous_sensing_slots as u64
                && slots_to_next_on_duration > 0
            {
                return true;
            }

            // Periodic sensing at T - k * P
            let step = config.periodic_step_slots as u64;
            if step > 0 {
                for k in 1..=config.periodic_sensing_depth as u64 {
                    let target_periodic_slot = k * step;
                    if slots_to_next_on_duration == target_periodic_slot {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Advances the engine by 1 radio slot.
    /// Returns `true` if the UE was in Sidelink Active Time during this slot.
    pub fn advance_slot(&mut self) -> bool {
        // Advance all active sessions
        for session in self.sessions.values_mut() {
            session.advance_slot(self.current_slot);
        }

        // Purge expired IUC resources and alerts
        let cur_slot = self.current_slot;
        self.preferred_resources.retain(|r| r.slot >= cur_slot);
        self.non_preferred_resources.retain(|r| r.slot >= cur_slot);
        self.collision_alerts
            .retain(|a| a.timestamp_slot + 160 >= cur_slot);

        let active = self.is_in_active_time();

        // Update telemetry
        self.telemetry.total_slots += 1;
        if active {
            self.telemetry.active_slots += 1;
            if !self.was_active_prev_slot {
                self.telemetry.num_wakeups += 1;
            }
        } else {
            self.telemetry.sleep_slots += 1;
        }

        self.was_active_prev_slot = active;
        self.telemetry.duty_cycle_percent =
            (self.telemetry.active_slots as f64 / self.telemetry.total_slots as f64) * 100.0;
        self.telemetry.power_saving_percent = 100.0 - self.telemetry.duty_cycle_percent;

        self.current_slot += 1;
        active
    }

    /// Advances the engine by multiple slots.
    pub fn advance_slots(&mut self, count: u64) {
        for _ in 0..count {
            self.advance_slot();
        }
    }

    // -----------------------------------------------------------------------
    // Event Handlers (SCI and HARQ)
    // -----------------------------------------------------------------------

    /// Notifies the engine that an SCI was received from a peer UE.
    /// Restarts `sl-drx-InactivityTimer` on matching session(s).
    pub fn notify_sci_received(&mut self, peer_l2_id: u32, is_new_transmission: bool) {
        self.telemetry.sci_rx_count += 1;

        if is_new_transmission {
            for session in self.sessions.values_mut() {
                let matches_peer = match session.config.cast_type {
                    SlDrxCastType::Unicast => session.config.peer_ue_id == Some(peer_l2_id),
                    SlDrxCastType::Groupcast | SlDrxCastType::Broadcast => true,
                };
                if matches_peer {
                    session.inactivity_remaining = session.config.inactivity_slots;
                    session.is_session_active = true;
                }
            }
        }
    }

    /// Notifies the engine that an SCI was transmitted by local UE.
    /// Restarts `sl-drx-InactivityTimer` on matching session(s).
    pub fn notify_sci_transmitted(&mut self, peer_l2_id: u32, is_new_transmission: bool) {
        self.telemetry.sci_tx_count += 1;

        if is_new_transmission {
            for session in self.sessions.values_mut() {
                let matches_peer = match session.config.cast_type {
                    SlDrxCastType::Unicast => session.config.peer_ue_id == Some(peer_l2_id),
                    SlDrxCastType::Groupcast | SlDrxCastType::Broadcast => true,
                };
                if matches_peer {
                    session.inactivity_remaining = session.config.inactivity_slots;
                    session.is_session_active = true;
                }
            }
        }
    }

    /// Starts `sl-drx-HARQ-RTT-Timer` for a given HARQ process after transmission/reception.
    pub fn trigger_harq_rtt(&mut self, profile_id: u8, harq_process_id: u8) {
        if let Some(session) = self.sessions.get_mut(&profile_id) {
            let rtt_duration = session.config.harq_rtt_slots;
            let process_state =
                session
                    .harq_processes
                    .entry(harq_process_id)
                    .or_insert(SidelinkHarqProcessState {
                        process_id: harq_process_id,
                        rtt_timer_remaining: rtt_duration,
                        retrans_timer_remaining: 0,
                        awaiting_feedback: true,
                    });
            process_state.rtt_timer_remaining = rtt_duration;
            process_state.awaiting_feedback = true;
        }
    }

    /// Notifies the engine that an ACK was received, stopping retransmission timer.
    pub fn notify_harq_ack(&mut self, profile_id: u8, harq_process_id: u8) {
        if let Some(session) = self.sessions.get_mut(&profile_id) {
            if let Some(state) = session.harq_processes.get_mut(&harq_process_id) {
                state.awaiting_feedback = false;
                state.retrans_timer_remaining = 0;
            }
            session.is_session_active = session.is_active();
        }
    }

    // -----------------------------------------------------------------------
    // Inter-UE Coordination (IUC) Rel-17 Methods
    // -----------------------------------------------------------------------

    /// Generates a Scheme 1 Inter-UE Coordination message to assist a target peer UE.
    pub fn generate_iuc_scheme1(
        &mut self,
        target_l2_id: u32,
        scheme: CoordinationSchemeType,
        resources: Vec<ResourceSlotBlock>,
    ) -> InterUeCoordinationMessage {
        self.telemetry.iuc_sent_count += 1;
        InterUeCoordinationMessage {
            sender_l2_id: self.local_ue_id,
            target_l2_id,
            scheme_type: scheme,
            resources,
            timestamp_slot: self.current_slot,
        }
    }

    /// Processes an incoming Inter-UE Coordination message received from a peer UE.
    pub fn process_iuc_message(&mut self, msg: InterUeCoordinationMessage) {
        if msg.target_l2_id != self.local_ue_id && msg.target_l2_id != 0xFFFFFF {
            return; // Not addressed to this UE
        }

        self.telemetry.iuc_received_count += 1;

        match msg.scheme_type {
            CoordinationSchemeType::Scheme1Preferred => {
                self.preferred_resources.extend(msg.resources);
            }
            CoordinationSchemeType::Scheme1NonPreferred => {
                self.non_preferred_resources.extend(msg.resources);
            }
            CoordinationSchemeType::Scheme2ConflictAlert => {
                self.collision_alerts.push(msg);
            }
        }
    }

    /// Evaluates if two resource reservations from peer UEs collide.
    /// If collision is detected, generates a Scheme 2 conflict alert.
    pub fn detect_collision_and_alert(
        &mut self,
        _peer_a_id: u32,
        res_a: &ResourceSlotBlock,
        peer_b_id: u32,
        res_b: &ResourceSlotBlock,
    ) -> Option<InterUeCoordinationMessage> {
        // Collide if slot matches and subchannel range overlaps
        if res_a.slot == res_b.slot {
            let a_start = res_a.subchannel_index;
            let a_end = a_start + res_a.num_subchannels;
            let b_start = res_b.subchannel_index;
            let b_end = b_start + res_b.num_subchannels;

            let overlap = !(a_end <= b_start || b_end <= a_start);
            if overlap {
                self.telemetry.collisions_avoided_count += 1;
                self.telemetry.iuc_sent_count += 1;

                let alert_resources = vec![*res_a, *res_b];
                return Some(InterUeCoordinationMessage {
                    sender_l2_id: self.local_ue_id,
                    target_l2_id: peer_b_id,
                    scheme_type: CoordinationSchemeType::Scheme2ConflictAlert,
                    resources: alert_resources,
                    timestamp_slot: self.current_slot,
                });
            }
        }
        None
    }

    /// Filters and ranks Mode 2 candidate resource blocks using received IUC Scheme 1 feedback.
    ///
    /// Excludes non-preferred resources and prioritizes preferred resources.
    pub fn filter_candidate_resources(
        &self,
        candidates: &[ResourceSlotBlock],
        rsrp_threshold_dbm: i16,
    ) -> Vec<ResourceSlotBlock> {
        let mut non_pref_set: HashSet<(u64, u8)> = HashSet::new();
        for r in &self.non_preferred_resources {
            for sub in r.subchannel_index..(r.subchannel_index + r.num_subchannels) {
                non_pref_set.insert((r.slot, sub));
            }
        }

        let mut pref_set: HashSet<(u64, u8)> = HashSet::new();
        for r in &self.preferred_resources {
            for sub in r.subchannel_index..(r.subchannel_index + r.num_subchannels) {
                pref_set.insert((r.slot, sub));
            }
        }

        let mut filtered: Vec<ResourceSlotBlock> = Vec::new();

        for cand in candidates {
            // Exclude if any subchannel intersects with non-preferred set
            let mut is_non_pref = false;
            for sub in cand.subchannel_index..(cand.subchannel_index + cand.num_subchannels) {
                if non_pref_set.contains(&(cand.slot, sub)) {
                    is_non_pref = true;
                    break;
                }
            }

            if is_non_pref && cand.rsrp_dbm > rsrp_threshold_dbm {
                continue; // Exclude due to non-preferred conflict
            }

            filtered.push(*cand);
        }

        // Sort: Preferred resources first, then lowest RSRP
        filtered.sort_by(|a, b| {
            let a_is_pref = pref_set.contains(&(a.slot, a.subchannel_index));
            let b_is_pref = pref_set.contains(&(b.slot, b.subchannel_index));
            match (a_is_pref, b_is_pref) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.rsrp_dbm.cmp(&b.rsrp_dbm),
            }
        });

        filtered
    }

    /// Returns the telemetry metrics.
    #[inline]
    pub fn telemetry(&self) -> &SidelinkDrxTelemetry {
        &self.telemetry
    }

    /// Resets the telemetry metrics.
    pub fn reset_telemetry(&mut self) {
        self.telemetry = SidelinkDrxTelemetry::default();
    }
}
