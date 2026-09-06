//! 3GPP Rel-17 5G NR Non-Terrestrial Networks (NTN) HARQ & Autonomous TA Tracking Engine.
//!
//! Compliant with:
//! - 3GPP TS 38.300 Rel-17 Section 16.14 ("Non-Terrestrial Networks Support")
//! - 3GPP TS 38.321 Rel-17 Section 5.4.3 ("HARQ in NTN") and Section 12 ("NTN Operation")
//! - 3GPP TS 38.331 Rel-17 (`SIB19`, `ntn-Config-r17`, `nrofHARQ-ProcessesForPDSCH-r17`)
//! - 3GPP TS 38.214 Rel-17 Section 5.1 & Section 6.1 ($K_{offset}$ cell-specific delay offset)
//! - 3GPP TS 38.213 Rel-17 Section 4.2 (Autonomous UE Timing Advance & Doppler Drift Tracking)
//!
//! Solves:
//! 1. Buffer stalling over high-delay satellite links (LEO 25..50 ms, GEO 540 ms RTT) via
//!    extended 32-process HARQ and per-process HARQ feedback disabling (blind repetitions).
//! 2. Continuous time-varying Doppler shift and Timing Advance (TA) drift at up to 40 µs/s
//!    caused by ~7.5 km/s LEO orbital velocity through autonomous UE GNSS-assisted tracking.
//! 3. Cell-specific $K_{offset}$ and $K_{mac}$ slot scheduling alignment for UL grants and DL PDSCH.

use std::fmt;

pub const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;
pub const MAX_NTN_HARQ_PROCESSES: usize = 32;
pub const STANDARD_TERRESTRIAL_HARQ_PROCESSES: usize = 16;
pub const DEFAULT_TA_STEP_THRESHOLD_US: f64 = 0.52; // ~16 Tc resolution threshold

// ---------------------------------------------------------------------------
// NTN Enums & Errors
// ---------------------------------------------------------------------------

/// Satellite Orbit Class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatelliteOrbitType {
    /// Low Earth Orbit (500 - 1500 km, ~7.5 km/s orbital velocity).
    Leo,
    /// Medium Earth Orbit (7000 - 25000 km).
    Meo,
    /// Geostationary Earth Orbit (35786 km, fixed relative to Earth surface).
    Geo,
}

/// HARQ Process Operational State in NTN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtnHarqProcessState {
    /// Available for new transmission.
    Idle,
    /// Active transmission (blind repetitions in progress).
    Transmitting { repetition_idx: u8 },
    /// Transmission sent, waiting for PUCCH/PDSCH ACK/NACK over long satellite RTT.
    AwaitingAck { rtt_slots_remaining: u32 },
    /// NACK received or blind repetition pending retransmission.
    AwaitingRetransmission,
    /// Successfully acknowledged or completed.
    Completed,
}

/// Errors raised during NTN HARQ scheduling and tracking operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NtnHarqError {
    InvalidProcessId(u8),
    HarqBufferStalled { active_processes: u8 },
    InvalidKOffset { requested: u32, min_allowed: u32 },
    InvalidConfiguration(&'static str),
}

impl fmt::Display for NtnHarqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NtnHarqError::InvalidProcessId(id) => {
                write!(f, "Invalid NTN HARQ process ID: {}", id)
            }
            NtnHarqError::HarqBufferStalled { active_processes } => {
                write!(
                    f,
                    "NTN HARQ buffer stalled: all {} processes awaiting round-trip ACK",
                    active_processes
                )
            }
            NtnHarqError::InvalidKOffset {
                requested,
                min_allowed,
            } => {
                write!(
                    f,
                    "Invalid K_offset {} slots, must be at least {} slots",
                    requested, min_allowed
                )
            }
            NtnHarqError::InvalidConfiguration(msg) => {
                write!(f, "Invalid NTN configuration: {}", msg)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SIB19 Configuration (TS 38.331 `SIB19` & `ntn-Config-r17`)
// ---------------------------------------------------------------------------

/// Broadcast System Information Block 19 (SIB19) parameters for 5G NTN.
#[derive(Debug, Clone, PartialEq)]
pub struct NtnSib19Config {
    /// Satellite orbit classification.
    pub orbit_type: SatelliteOrbitType,
    /// Cell-specific scheduling offset K_offset in slots (TS 38.214 Section 5.1/6.1).
    pub k_offset_slots: u32,
    /// Scheduling offset K_mac in slots for MAC CE activation/deactivation delay.
    pub k_mac_slots: u16,
    /// Feeder link propagation delay in milliseconds (Gateway <-> Satellite).
    pub feeder_link_delay_ms: f64,
    /// Satellite ECEF position coordinates in meters [X, Y, Z].
    pub satellite_pos_ecef_m: [f64; 3],
    /// Satellite ECEF velocity vector in meters/second [Vx, Vy, Vz].
    pub satellite_vel_ecef_mps: [f64; 3],
    /// Reference epoch timestamp in seconds.
    pub epoch_time_s: u64,
    /// Downlink carrier frequency in Hz.
    pub carrier_frequency_hz: f64,
    /// Subcarrier spacing in kHz (e.g. 15, 30, 60, 120 kHz).
    pub subcarrier_spacing_khz: u16,
}

impl NtnSib19Config {
    /// Computes slot duration in milliseconds based on subcarrier spacing (TS 38.211).
    #[inline]
    pub fn slot_duration_ms(&self) -> f64 {
        1.0 / (self.subcarrier_spacing_khz as f64 / 15.0)
    }

    /// Computes minimum K_offset slots required for common feeder + service link delay.
    pub fn calculate_min_k_offset(&self, service_delay_ms: f64) -> u32 {
        let total_rtt_ms = 2.0 * (self.feeder_link_delay_ms + service_delay_ms);
        let slot_dur = self.slot_duration_ms();
        (total_rtt_ms / slot_dur).ceil() as u32
    }
}

// ---------------------------------------------------------------------------
// Autonomous Timing Advance & Doppler Drift Tracker (TS 38.213 Section 4.2)
// ---------------------------------------------------------------------------

/// Autonomous UE Timing Advance and Doppler Shift Tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct AutonomousTaTracker {
    /// Ground UE ECEF position in meters [X, Y, Z] (from onboard GNSS).
    pub ue_pos_ecef_m: [f64; 3],
    /// Downlink carrier frequency in Hz.
    pub carrier_frequency_hz: f64,
    /// Current slant range from UE to satellite in kilometers.
    pub current_slant_range_km: f64,
    /// Current radial velocity relative to UE in m/s (positive = moving away).
    pub current_radial_velocity_mps: f64,
    /// Current one-way service link delay in milliseconds.
    pub current_service_delay_ms: f64,
    /// Current full Timing Advance (T_TA) in microseconds (feeder + service link).
    pub current_timing_advance_us: f64,
    /// Current Doppler frequency shift in Hz.
    pub current_doppler_shift_hz: f64,
    /// Accumulated TA drift since last adjustment step in microseconds.
    pub accumulated_ta_drift_us: f64,
    /// Threshold in microseconds to trigger an autonomous TA update event.
    pub step_threshold_us: f64,
    /// Counter of autonomous TA update events performed.
    pub total_ta_adjustments_count: u64,
}

impl AutonomousTaTracker {
    /// Creates a new autonomous TA tracker for a UE at a known GNSS position.
    pub fn new(ue_pos_ecef_m: [f64; 3], carrier_frequency_hz: f64) -> Self {
        Self {
            ue_pos_ecef_m,
            carrier_frequency_hz,
            current_slant_range_km: 0.0,
            current_radial_velocity_mps: 0.0,
            current_service_delay_ms: 0.0,
            current_timing_advance_us: 0.0,
            current_doppler_shift_hz: 0.0,
            accumulated_ta_drift_us: 0.0,
            step_threshold_us: DEFAULT_TA_STEP_THRESHOLD_US,
            total_ta_adjustments_count: 0,
        }
    }

    /// Updates orbital geometry, computes new slant range, Doppler, and TA drift.
    ///
    /// Returns `Some(ta_adjustment_us)` if accumulated drift exceeds `step_threshold_us`.
    pub fn update_geometry(
        &mut self,
        sat_pos: &[f64; 3],
        sat_vel: &[f64; 3],
        feeder_delay_ms: f64,
    ) -> Option<f64> {
        // Line-of-sight vector from UE to Satellite: D = P_sat - P_ue
        let dx = sat_pos[0] - self.ue_pos_ecef_m[0];
        let dy = sat_pos[1] - self.ue_pos_ecef_m[1];
        let dz = sat_pos[2] - self.ue_pos_ecef_m[2];
        let dist_m = (dx * dx + dy * dy + dz * dz).sqrt();

        let old_ta_us = self.current_timing_advance_us;

        self.current_slant_range_km = dist_m / 1000.0;
        let one_way_sec = dist_m / SPEED_OF_LIGHT_MPS;
        self.current_service_delay_ms = one_way_sec * 1000.0;

        // Total Timing Advance: 2 * (T_service + T_feeder)
        let total_one_way_us = (self.current_service_delay_ms + feeder_delay_ms) * 1000.0;
        let new_ta_us = 2.0 * total_one_way_us;

        // Radial velocity: dot product of satellite velocity and unit LoS vector (d(dist)/dt)
        let unit_x = dx / dist_m;
        let unit_y = dy / dist_m;
        let unit_z = dz / dist_m;
        let v_radial = sat_vel[0] * unit_x + sat_vel[1] * unit_y + sat_vel[2] * unit_z;
        self.current_radial_velocity_mps = v_radial;

        // Doppler frequency shift: f_d = - (v_r / c) * f_c
        self.current_doppler_shift_hz =
            -(v_radial / SPEED_OF_LIGHT_MPS) * self.carrier_frequency_hz;

        if old_ta_us > 0.0 {
            let delta = new_ta_us - old_ta_us;
            self.accumulated_ta_drift_us += delta;
        }
        self.current_timing_advance_us = new_ta_us;

        // Check if accumulated drift exceeds threshold
        if self.accumulated_ta_drift_us.abs() >= self.step_threshold_us {
            let adjustment = self.accumulated_ta_drift_us;
            self.accumulated_ta_drift_us = 0.0;
            self.total_ta_adjustments_count += 1;
            Some(adjustment)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// 32-Process Extended HARQ Pool & Process Structure (TS 38.321 §5.4.3)
// ---------------------------------------------------------------------------

/// Per-process NTN HARQ state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtnHarqProcess {
    pub process_id: u8,
    /// True: normal ACK/NACK reporting; False: blind retransmission, feedback disabled.
    pub is_feedback_enabled: bool,
    /// Number of configured blind retransmissions (0..4) when feedback is disabled.
    pub blind_repetitions_max: u8,
    /// Blind repetitions executed so far.
    pub blind_repetitions_done: u8,
    /// Process state.
    pub state: NtnHarqProcessState,
    /// New Data Indicator.
    pub ndi: bool,
    /// Redundancy Version (0, 2, 3, 1).
    pub rv: u8,
    /// Payload size in bytes.
    pub payload_size_bytes: usize,
    /// Target slot when scheduled transmission occurs.
    pub target_slot: u64,
}

impl NtnHarqProcess {
    pub fn new(process_id: u8) -> Self {
        Self {
            process_id,
            is_feedback_enabled: true,
            blind_repetitions_max: 0,
            blind_repetitions_done: 0,
            state: NtnHarqProcessState::Idle,
            ndi: false,
            rv: 0,
            payload_size_bytes: 0,
            target_slot: 0,
        }
    }

    /// Resets the process back to Idle state.
    pub fn reset(&mut self) {
        self.state = NtnHarqProcessState::Idle;
        self.blind_repetitions_done = 0;
        self.payload_size_bytes = 0;
        self.rv = 0;
    }
}

// ---------------------------------------------------------------------------
// Telemetry & KPI Analytics
// ---------------------------------------------------------------------------

/// Real-time metrics for 5G NTN MAC and HARQ performance.
#[derive(Debug, Clone, PartialEq)]
pub struct NtnHarqTelemetry {
    pub total_scheduled_grants: u64,
    pub total_transmitted_bytes: u64,
    pub stall_slots_count: u64,
    pub stall_slots_avoided_count: u64,
    pub successful_deliveries: u64,
    pub blind_retransmissions_sent: u64,
    pub feedback_acks_received: u64,
    pub feedback_nacks_received: u64,
    pub autonomous_ta_updates_count: u64,
}

impl Default for NtnHarqTelemetry {
    fn default() -> Self {
        Self {
            total_scheduled_grants: 0,
            total_transmitted_bytes: 0,
            stall_slots_count: 0,
            stall_slots_avoided_count: 0,
            successful_deliveries: 0,
            blind_retransmissions_sent: 0,
            feedback_acks_received: 0,
            feedback_nacks_received: 0,
            autonomous_ta_updates_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-Level 5G NTN HARQ Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR NTN HARQ & Autonomous TA Tracking Engine.
#[derive(Debug)]
pub struct NtnHarqEngine {
    pub ue_id: u32,
    pub sib19: NtnSib19Config,
    pub ta_tracker: AutonomousTaTracker,
    pub processes: Vec<NtnHarqProcess>,
    pub current_slot: u64,
    pub telemetry: NtnHarqTelemetry,
}

impl NtnHarqEngine {
    /// Creates a new NTN HARQ engine with up to 32 extended HARQ processes.
    pub fn new(
        ue_id: u32,
        sib19: NtnSib19Config,
        ue_pos_ecef_m: [f64; 3],
        num_processes: u8,
    ) -> Result<Self, NtnHarqError> {
        let count = num_processes.clamp(1, MAX_NTN_HARQ_PROCESSES as u8) as usize;
        let mut processes = Vec::with_capacity(count);
        for id in 0..count {
            processes.push(NtnHarqProcess::new(id as u8));
        }

        let carrier_hz = sib19.carrier_frequency_hz;
        let mut ta_tracker = AutonomousTaTracker::new(ue_pos_ecef_m, carrier_hz);
        ta_tracker.update_geometry(
            &sib19.satellite_pos_ecef_m,
            &sib19.satellite_vel_ecef_mps,
            sib19.feeder_link_delay_ms,
        );

        Ok(Self {
            ue_id,
            sib19,
            ta_tracker,
            processes,
            current_slot: 0,
            telemetry: NtnHarqTelemetry::default(),
        })
    }

    /// Configures HARQ feedback enabling/disabling and blind repetitions per process.
    pub fn configure_harq_feedback(
        &mut self,
        process_id: u8,
        enabled: bool,
        blind_repetitions: u8,
    ) -> Result<(), NtnHarqError> {
        if (process_id as usize) >= self.processes.len() {
            return Err(NtnHarqError::InvalidProcessId(process_id));
        }
        let proc = &mut self.processes[process_id as usize];
        proc.is_feedback_enabled = enabled;
        proc.blind_repetitions_max = blind_repetitions;
        Ok(())
    }

    /// Evaluates whether all configured HARQ processes are occupied and awaiting feedback.
    pub fn is_stalled(&self) -> bool {
        self.processes.iter().all(|p| match p.state {
            NtnHarqProcessState::Idle | NtnHarqProcessState::Completed => false,
            _ => true,
        })
    }

    /// Schedules an Uplink Grant with cell-specific delay offset K_offset (TS 38.214 §6.1).
    ///
    /// The scheduled slot is: target_slot = current_slot + k2_slots + K_offset.
    ///
    /// Returns `(allocated_process_id, scheduled_slot)`.
    pub fn schedule_uplink_grant(
        &mut self,
        k2_slots: u16,
        payload_size: usize,
    ) -> Result<(u8, u64), NtnHarqError> {
        // Find first available idle process
        let available_proc = self.processes.iter().position(|p| match p.state {
            NtnHarqProcessState::Idle | NtnHarqProcessState::Completed => true,
            _ => false,
        });

        let proc_idx = match available_proc {
            Some(idx) => idx,
            None => {
                self.telemetry.stall_slots_count += 1;
                return Err(NtnHarqError::HarqBufferStalled {
                    active_processes: self.processes.len() as u8,
                });
            }
        };

        let scheduled_slot =
            self.current_slot + (k2_slots as u64) + (self.sib19.k_offset_slots as u64);
        let proc = &mut self.processes[proc_idx];
        proc.target_slot = scheduled_slot;
        proc.payload_size_bytes = payload_size;
        proc.ndi = !proc.ndi; // Toggle NDI for new data
        proc.rv = 0;

        if proc.is_feedback_enabled {
            // Normal HARQ: Start in Transmitting, will wait for round-trip ACK
            proc.state = NtnHarqProcessState::Transmitting { repetition_idx: 0 };
        } else {
            // Disabled Feedback: Blind repetitions without waiting for ACK
            proc.blind_repetitions_done = 0;
            proc.state = NtnHarqProcessState::Transmitting { repetition_idx: 0 };
            self.telemetry.stall_slots_avoided_count += 1;
        }

        self.telemetry.total_scheduled_grants += 1;
        self.telemetry.total_transmitted_bytes += payload_size as u64;

        Ok((proc_idx as u8, scheduled_slot))
    }

    /// Handles incoming HARQ ACK or NACK feedback from gNB / peer receiver.
    pub fn notify_harq_feedback(
        &mut self,
        process_id: u8,
        is_ack: bool,
    ) -> Result<(), NtnHarqError> {
        if (process_id as usize) >= self.processes.len() {
            return Err(NtnHarqError::InvalidProcessId(process_id));
        }

        let proc = &mut self.processes[process_id as usize];
        if is_ack {
            self.telemetry.feedback_acks_received += 1;
            self.telemetry.successful_deliveries += 1;
            proc.reset();
        } else {
            self.telemetry.feedback_nacks_received += 1;
            // Cycle RV: 0 -> 2 -> 3 -> 1
            proc.rv = match proc.rv {
                0 => 2,
                2 => 3,
                3 => 1,
                _ => 0,
            };
            proc.state = NtnHarqProcessState::AwaitingRetransmission;
        }

        Ok(())
    }

    /// Advances the engine by 1 radio slot.
    pub fn advance_slot(&mut self) {
        let cur_slot = self.current_slot;
        let rtt_slots = self.sib19.k_offset_slots;

        for proc in &mut self.processes {
            match proc.state {
                NtnHarqProcessState::Transmitting { repetition_idx } => {
                    if cur_slot >= proc.target_slot {
                        if !proc.is_feedback_enabled {
                            // Blind repetition mode
                            if proc.blind_repetitions_done < proc.blind_repetitions_max {
                                proc.blind_repetitions_done += 1;
                                proc.state = NtnHarqProcessState::Transmitting {
                                    repetition_idx: repetition_idx + 1,
                                };
                                proc.target_slot = cur_slot + 1; // Immediate next slot repetition
                            } else {
                                // All blind repetitions completed -> Success without feedback
                                proc.state = NtnHarqProcessState::Completed;
                            }
                        } else {
                            // Feedback enabled -> Transition to AwaitingAck
                            proc.state = NtnHarqProcessState::AwaitingAck {
                                rtt_slots_remaining: rtt_slots,
                            };
                        }
                    }
                }
                NtnHarqProcessState::AwaitingAck {
                    ref mut rtt_slots_remaining,
                } => {
                    if *rtt_slots_remaining > 0 {
                        *rtt_slots_remaining -= 1;
                    }
                }
                _ => {}
            }
        }

        self.current_slot += 1;
    }

    /// Advances the engine by multiple slots.
    pub fn advance_slots(&mut self, count: u64) {
        for _ in 0..count {
            self.advance_slot();
        }
    }

    /// Updates the autonomous TA tracker with orbital progression (simulating satellite motion).
    pub fn update_satellite_orbit(&mut self, elapsed_seconds: f64) -> Option<f64> {
        let mut sat_pos = self.sib19.satellite_pos_ecef_m;
        let sat_vel = self.sib19.satellite_vel_ecef_mps;

        // Simple orbital progression: P(t) = P(0) + V * t
        sat_pos[0] += sat_vel[0] * elapsed_seconds;
        sat_pos[1] += sat_vel[1] * elapsed_seconds;
        sat_pos[2] += sat_vel[2] * elapsed_seconds;

        let adj =
            self.ta_tracker
                .update_geometry(&sat_pos, &sat_vel, self.sib19.feeder_link_delay_ms);
        if adj.is_some() {
            self.telemetry.autonomous_ta_updates_count += 1;
        }
        adj
    }

    /// Returns the telemetry metrics.
    #[inline]
    pub fn telemetry(&self) -> &NtnHarqTelemetry {
        &self.telemetry
    }
}
