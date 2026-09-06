//! 3GPP Rel-17 5G NR Sidelink (SL) & Cellular V2X (C-V2X) PC5 Protocol Engine.
//!
//! Compliant with 3GPP TS 38.321 Rel-17 §5.22 ("SL-SCH Data transfer"),
//! TS 38.214 §8.1.4 ("Resource allocation in sidelink"), TS 38.212 §8.3/§8.4,
//! and TS 38.215 §5.1.30 (CBR and CR).
//!
//! Provides the core direct communication and vehicular networking capabilities:
//! - Two-stage Sidelink Control Information (SCI format 1-A and format 2-A).
//! - Mode 2 Autonomous Sensing Window and Selection Window resource selection.
//! - SL-RSRP collision exclusion with 3 dB iterative threshold backoff.
//! - Channel Busy Ratio (CBR) and Channel Occupancy Ratio (CR) congestion control.
//! - Distance-based Groupcast Option 1 and Unicast PSFCH HARQ feedback.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Configuration & Sidelink Bandwidth Part (BWP)
// ---------------------------------------------------------------------------

/// Sidelink Bandwidth Part (SL-BWP) configuration (TS 38.331 `BWP-SidelinkDedicated`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidelinkBandwidthPart {
    /// Number of sub-channels in the resource pool (e.g. 10..27).
    pub num_subchannels: u8,
    /// Number of PRBs per sub-channel (e.g. 10, 15, 20, 25, 50).
    pub subchannel_size_prb: u8,
    /// Starting PRB index in carrier grid.
    pub start_rb: u16,
    /// PSFCH slot allocation period in slots (0 = no PSFCH, 1, 2, or 4 slots).
    pub psfch_period_slots: u8,
    /// UE processing time T_proc,0 in slots (typically 1..4 slots).
    pub min_proc_time_tproc0: u16,
}

// ---------------------------------------------------------------------------
// Two-Stage Sidelink Control Information (SCI)
// ---------------------------------------------------------------------------

/// 1st-Stage Sidelink Control Information transmitted on PSCCH (TS 38.212 §8.3.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciFormat1A {
    /// Priority indicator (0..7, where 0 is highest priority).
    pub priority: u8,
    /// Frequency resource assignment (sub-channel index or RIV).
    pub freq_resource_assign: u16,
    /// Time resource assignment for retransmissions.
    pub time_resource_assign: u8,
    /// Resource reservation period in milliseconds (0, 100, 200..1000 ms).
    pub reservation_period_ms: u16,
    /// Modulation and Coding Scheme (0..31).
    pub mcs: u8,
    /// 2nd-stage SCI format indicator (0 = format 2-A, 1 = format 2-B).
    pub stage2_format: u8,
}

/// Sidelink transmission cast type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidelinkCastType {
    /// Broadcast to all peer UEs (no HARQ feedback).
    Broadcast,
    /// Groupcast Option 1: Negative-only HARQ feedback if distance <= range requirement.
    GroupcastOption1Distance,
    /// Groupcast Option 2: Conventional ACK/NACK reporting.
    GroupcastOption2,
    /// Unicast point-to-point transmission (ACK/NACK reporting).
    Unicast,
}

/// 2nd-Stage Sidelink Control Information transmitted on PSSCH (TS 38.212 §8.4.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SciFormat2A {
    /// Source Layer-2 Identity (24 bits).
    pub source_l2_id: u32,
    /// Destination Layer-2 Identity (24 bits).
    pub dest_l2_id: u32,
    /// HARQ Process Identifier (0..15).
    pub harq_process_id: u8,
    /// New Data Indicator.
    pub ndi: bool,
    /// Redundancy Version (0..3).
    pub rv: u8,
    /// Sidelink communication cast type.
    pub cast_type: SidelinkCastType,
    /// HARQ feedback enabled flag.
    pub harq_feedback_enabled: bool,
    /// Minimum communication range requirement in meters (for Groupcast Option 1).
    pub comm_range_requirement_m: Option<u32>,
}

// ---------------------------------------------------------------------------
// Sensing & Mode 2 Selection Types
// ---------------------------------------------------------------------------

/// Decoded historical sensing reservation entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensingReservationEntry {
    pub slot_received: u64,
    pub subchannel: u8,
    pub sci1: SciFormat1A,
    /// Measured SL-RSRP on PSSCH DMRS in dBm (-140 dBm .. -40 dBm).
    pub sl_rsrp_dbm: i16,
}

/// Candidate single-slot transmission resource $R_{x, y}$.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CandidateResource {
    pub slot: u64,
    pub subchannel: u8,
}

/// PSFCH HARQ Feedback report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsfchFeedback {
    Ack,
    Nack,
}

/// Channel Busy Ratio (CBR) measurement result (TS 38.215 §5.1.30).
#[derive(Debug, Clone, PartialEq)]
pub struct CbrMeasurement {
    pub cbr_ratio: f32,
    pub busy_subchannels: usize,
    pub total_subchannels: usize,
}

/// Channel Occupancy Ratio (CR) evaluation result.
#[derive(Debug, Clone, PartialEq)]
pub struct CrMeasurement {
    pub cr_ratio: f32,
    pub cr_limit: f32,
    pub congestion_mitigated: bool,
}

// ---------------------------------------------------------------------------
// 5G NR Sidelink Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR Sidelink & C-V2X Protocol Engine.
#[derive(Debug)]
pub struct NrSidelinkEngine {
    pub ue_l2_id: u32,
    pub bwp: SidelinkBandwidthPart,
    pub sensing_history: Vec<SensingReservationEntry>,
    pub rssi_history: Vec<i16>,
    pub transmission_history: Vec<(u64, u8)>, // (slot, subchannel count)
    pub default_rsrp_threshold_dbm: i16,
    pub cr_limit_table: HashMap<u8, f32>, // Priority -> CR_limit
}

impl NrSidelinkEngine {
    /// Create a new Sidelink Protocol Engine instance.
    pub fn new(ue_l2_id: u32, bwp: SidelinkBandwidthPart) -> Self {
        let mut cr_limits = HashMap::new();
        // Standard CR limits per priority class (0..7)
        cr_limits.insert(0, 0.80); // Emergency safety / highest priority
        cr_limits.insert(1, 0.60);
        cr_limits.insert(2, 0.50);
        cr_limits.insert(3, 0.40);
        cr_limits.insert(4, 0.30);
        cr_limits.insert(5, 0.20);
        cr_limits.insert(6, 0.15);
        cr_limits.insert(7, 0.10); // Lowest priority background telemetry

        Self {
            ue_l2_id,
            bwp,
            sensing_history: Vec::new(),
            rssi_history: Vec::new(),
            transmission_history: Vec::new(),
            default_rsrp_threshold_dbm: -110,
            cr_limit_table: cr_limits,
        }
    }

    // -----------------------------------------------------------------------
    // SCI Serialization (TS 38.212 §8.3/§8.4)
    // -----------------------------------------------------------------------

    /// Serializes 1st-Stage SCI format 1-A into a compact 6-byte wire payload.
    pub fn encode_sci_1a(sci: &SciFormat1A) -> [u8; 6] {
        let mut bytes = [0u8; 6];
        // Byte 0: [Priority: 3 bits][Stage2: 1 bit][Freq MSBs: 4 bits]
        bytes[0] = ((sci.priority & 0x07) << 5)
            | ((sci.stage2_format & 0x01) << 4)
            | (((sci.freq_resource_assign >> 8) & 0x0F) as u8);
        // Byte 1: [Freq LSBs: 8 bits]
        bytes[1] = (sci.freq_resource_assign & 0xFF) as u8;
        // Byte 2: [Time resource assign: 8 bits]
        bytes[2] = sci.time_resource_assign;
        // Byte 3 & 4: [Reservation period: 16 bits]
        bytes[3] = (sci.reservation_period_ms >> 8) as u8;
        bytes[4] = (sci.reservation_period_ms & 0xFF) as u8;
        // Byte 5: [MCS: 5 bits][Reserved: 3 bits]
        bytes[5] = (sci.mcs & 0x1F) << 3;
        bytes
    }

    /// Parses 1st-Stage SCI format 1-A from a 6-byte wire payload.
    pub fn decode_sci_1a(bytes: &[u8; 6]) -> Result<SciFormat1A, &'static str> {
        let priority = (bytes[0] >> 5) & 0x07;
        let stage2_format = (bytes[0] >> 4) & 0x01;
        let freq_msb = (bytes[0] & 0x0F) as u16;
        let freq_lsb = bytes[1] as u16;
        let freq_resource_assign = (freq_msb << 8) | freq_lsb;
        let time_resource_assign = bytes[2];
        let reservation_period_ms = ((bytes[3] as u16) << 8) | (bytes[4] as u16);
        let mcs = (bytes[5] >> 3) & 0x1F;

        Ok(SciFormat1A {
            priority,
            freq_resource_assign,
            time_resource_assign,
            reservation_period_ms,
            mcs,
            stage2_format,
        })
    }

    /// Serializes 2nd-Stage SCI format 2-A into an 8-byte wire payload.
    pub fn encode_sci_2a(sci: &SciFormat2A) -> [u8; 8] {
        let mut bytes = [0u8; 8];
        // Bytes 0..2: Source Layer-2 ID (24 bits)
        bytes[0] = ((sci.source_l2_id >> 16) & 0xFF) as u8;
        bytes[1] = ((sci.source_l2_id >> 8) & 0xFF) as u8;
        bytes[2] = (sci.source_l2_id & 0xFF) as u8;

        // Bytes 3..5: Destination Layer-2 ID (24 bits)
        bytes[3] = ((sci.dest_l2_id >> 16) & 0xFF) as u8;
        bytes[4] = ((sci.dest_l2_id >> 8) & 0xFF) as u8;
        bytes[5] = (sci.dest_l2_id & 0xFF) as u8;

        // Byte 6: [HARQ Process ID: 4 bits][RV: 2 bits][NDI: 1 bit][Feedback Enabled: 1 bit]
        let ndi_bit = if sci.ndi { 1u8 } else { 0u8 };
        let fb_bit = if sci.harq_feedback_enabled { 1u8 } else { 0u8 };
        bytes[6] =
            ((sci.harq_process_id & 0x0F) << 4) | ((sci.rv & 0x03) << 2) | (ndi_bit << 1) | fb_bit;

        // Byte 7: [Cast Type: 2 bits][Range requirement code: 6 bits]
        let cast_bits = match sci.cast_type {
            SidelinkCastType::Broadcast => 0x00,
            SidelinkCastType::GroupcastOption1Distance => 0x01,
            SidelinkCastType::GroupcastOption2 => 0x02,
            SidelinkCastType::Unicast => 0x03,
        };
        let range_code = sci
            .comm_range_requirement_m
            .map(|r| (r / 10).min(63) as u8)
            .unwrap_or(0);
        bytes[7] = (cast_bits << 6) | (range_code & 0x3F);

        bytes
    }

    /// Parses 2nd-Stage SCI format 2-A from an 8-byte wire payload.
    pub fn decode_sci_2a(bytes: &[u8; 8]) -> Result<SciFormat2A, &'static str> {
        let source_l2_id = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
        let dest_l2_id = ((bytes[3] as u32) << 16) | ((bytes[4] as u32) << 8) | (bytes[5] as u32);
        let harq_process_id = (bytes[6] >> 4) & 0x0F;
        let rv = (bytes[6] >> 2) & 0x03;
        let ndi = ((bytes[6] >> 1) & 0x01) != 0;
        let harq_feedback_enabled = (bytes[6] & 0x01) != 0;

        let cast_type = match (bytes[7] >> 6) & 0x03 {
            0x00 => SidelinkCastType::Broadcast,
            0x01 => SidelinkCastType::GroupcastOption1Distance,
            0x02 => SidelinkCastType::GroupcastOption2,
            _ => SidelinkCastType::Unicast,
        };
        let range_code = (bytes[7] & 0x3F) as u32;
        let comm_range_requirement_m = if range_code > 0 {
            Some(range_code * 10)
        } else {
            None
        };

        Ok(SciFormat2A {
            source_l2_id,
            dest_l2_id,
            harq_process_id,
            ndi,
            rv,
            cast_type,
            harq_feedback_enabled,
            comm_range_requirement_m,
        })
    }

    // -----------------------------------------------------------------------
    // Mode 2 Autonomous Sensing & Collision Exclusion (TS 38.214 §8.1.4)
    // -----------------------------------------------------------------------

    /// Record a decoded SCI format 1-A with its measured SL-RSRP into sensing history.
    pub fn record_sensing_reservation(
        &mut self,
        slot_received: u64,
        subchannel: u8,
        sci1: SciFormat1A,
        sl_rsrp_dbm: i16,
    ) {
        self.sensing_history.push(SensingReservationEntry {
            slot_received,
            subchannel,
            sci1,
            sl_rsrp_dbm,
        });

        // Retain only the last 1100 slots of sensing history (maximum sensing window)
        if self.sensing_history.len() > 2000 {
            self.sensing_history.remove(0);
        }
    }

    /// Selects an optimal candidate single-slot resource in Mode 2 selection window.
    ///
    /// Implements:
    /// 1. Selection window initialization: $[n + T_1, n + T_2]$.
    /// 2. Collision exclusion based on historical SCI reservations and SL-RSRP threshold.
    /// 3. Dynamic 3 dB threshold backoff if available candidate resources fall below 20%.
    pub fn select_mode2_resource(
        &self,
        current_slot: u64,
        t1_slots: u16,
        t2_slots: u16,
        my_priority: u8,
    ) -> Option<CandidateResource> {
        let start_slot = current_slot + t1_slots as u64;
        let end_slot = current_slot + t2_slots as u64;

        if end_slot <= start_slot || self.bwp.num_subchannels == 0 {
            return None;
        }

        // Total candidate single-slot resources in selection window
        let mut all_candidates = Vec::new();
        for s in start_slot..=end_slot {
            for subch in 0..self.bwp.num_subchannels {
                all_candidates.push(CandidateResource {
                    slot: s,
                    subchannel: subch,
                });
            }
        }

        let total_count = all_candidates.len();
        if total_count == 0 {
            return None;
        }

        let min_required = (total_count as f64 * 0.20).ceil() as usize;

        let mut current_thresh = self.default_rsrp_threshold_dbm;

        loop {
            let mut eligible: Vec<CandidateResource> = all_candidates
                .iter()
                .filter(|cand| {
                    // Check if candidate collides with any past reservation meeting RSRP threshold
                    for entry in &self.sensing_history {
                        if entry.sl_rsrp_dbm < current_thresh {
                            continue; // Quality low enough to ignore interference
                        }

                        // Priority-based adjustment: higher priority peer gets stricter protection
                        if entry.sci1.priority > my_priority
                            && entry.sl_rsrp_dbm < (current_thresh + 3)
                        {
                            continue;
                        }

                        // Check periodic reservation overlap
                        let period = entry.sci1.reservation_period_ms as u64;
                        if period > 0 {
                            let delta = cand.slot.saturating_sub(entry.slot_received);
                            if delta > 0
                                && delta % period == 0
                                && cand.subchannel == entry.subchannel
                            {
                                return false; // Colliding periodic resource excluded!
                            }
                        } else if cand.slot == entry.slot_received
                            && cand.subchannel == entry.subchannel
                        {
                            return false;
                        }
                    }
                    true
                })
                .cloned()
                .collect();

            if eligible.len() >= min_required || current_thresh >= -60 {
                if eligible.is_empty() {
                    eligible = all_candidates; // Fallback
                }
                // Deterministic pseudorandom pick based on current slot
                let idx = (current_slot as usize) % eligible.len();
                return Some(eligible[idx]);
            }

            // Available resources < 20%: increase threshold by 3 dB and re-evaluate!
            current_thresh += 3;
        }
    }

    // -----------------------------------------------------------------------
    // Congestion Control: CBR and CR (TS 38.215 §5.1.30)
    // -----------------------------------------------------------------------

    /// Records S-RSSI measurement for CBR calculation.
    pub fn record_s_rssi(&mut self, rssi_dbm: i16) {
        self.rssi_history.push(rssi_dbm);
        if self.rssi_history.len() > 100 {
            self.rssi_history.remove(0);
        }
    }

    /// Record a transmission for CR calculation.
    pub fn record_transmission(&mut self, slot: u64, subchannels_used: u8) {
        self.transmission_history.push((slot, subchannels_used));
        if self.transmission_history.len() > 1000 {
            self.transmission_history.remove(0);
        }
    }

    /// Evaluates Channel Busy Ratio (CBR) over the past 100 slots.
    pub fn evaluate_cbr(&self, rssi_thresh_dbm: i16) -> CbrMeasurement {
        if self.rssi_history.is_empty() {
            return CbrMeasurement {
                cbr_ratio: 0.0,
                busy_subchannels: 0,
                total_subchannels: 0,
            };
        }

        let busy = self
            .rssi_history
            .iter()
            .filter(|&&r| r >= rssi_thresh_dbm)
            .count();
        let total = self.rssi_history.len();
        let ratio = busy as f32 / total as f32;

        CbrMeasurement {
            cbr_ratio: ratio,
            busy_subchannels: busy,
            total_subchannels: total,
        }
    }

    /// Evaluates Channel Occupancy Ratio (CR) against configured CR_limit.
    pub fn evaluate_cr(
        &self,
        current_slot: u64,
        my_priority: u8,
        intended_subchannels: usize,
    ) -> CrMeasurement {
        let window_slots = 1000;
        let start_window = current_slot.saturating_sub(500);
        let end_window = current_slot + 500;

        let past_used: usize = self
            .transmission_history
            .iter()
            .filter(|(s, _)| *s >= start_window && *s <= end_window)
            .map(|(_, count)| *count as usize)
            .sum();

        let total_possible = (window_slots as usize) * (self.bwp.num_subchannels as usize);
        let total_used = past_used + intended_subchannels;

        let cr_ratio = if total_possible == 0 {
            0.0
        } else {
            total_used as f32 / total_possible as f32
        };

        let cr_limit = *self.cr_limit_table.get(&my_priority).unwrap_or(&0.50);
        let congestion_mitigated = cr_ratio > cr_limit;

        CrMeasurement {
            cr_ratio,
            cr_limit,
            congestion_mitigated,
        }
    }

    // -----------------------------------------------------------------------
    // PSFCH HARQ Feedback (TS 38.213 §16.5)
    // -----------------------------------------------------------------------

    /// Evaluates PSFCH HARQ feedback generation according to Cast Type and Communication Range.
    pub fn evaluate_psfch_feedback(
        &self,
        cast_type: SidelinkCastType,
        decode_success: bool,
        distance_to_tx_m: Option<u32>,
        range_requirement_m: Option<u32>,
    ) -> Option<PsfchFeedback> {
        match cast_type {
            SidelinkCastType::Broadcast => None, // No HARQ on broadcast
            SidelinkCastType::Unicast | SidelinkCastType::GroupcastOption2 => {
                if decode_success {
                    Some(PsfchFeedback::Ack)
                } else {
                    Some(PsfchFeedback::Nack)
                }
            }
            SidelinkCastType::GroupcastOption1Distance => {
                // Option 1: Negative-only reporting if decode failed AND within communication range!
                if !decode_success {
                    if let (Some(dist), Some(req)) = (distance_to_tx_m, range_requirement_m) {
                        if dist <= req {
                            Some(PsfchFeedback::Nack)
                        } else {
                            None // Beyond range: suppress NACK
                        }
                    } else {
                        Some(PsfchFeedback::Nack)
                    }
                } else {
                    None // No ACK transmitted in Option 1
                }
            }
        }
    }
}
