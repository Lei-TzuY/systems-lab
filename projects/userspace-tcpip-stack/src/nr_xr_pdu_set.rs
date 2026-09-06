//! 3GPP Release 18 (5G-Advanced) Extended Reality (XR) & PDU Set Protocol Engine.
//!
//! Conforms to:
//! - 3GPP TS 23.501 Rel-18 §5.37: Support for PDU Set based QoS and Handling in 5GS.
//! - 3GPP TS 38.300 Rel-18 §16.14: Extended Reality (XR) enhancements in NG-RAN.
//! - 3GPP TS 38.323 Rel-18 §5.17: PDCP PDU Set marking and cascading discard.
//! - 3GPP TR 38.835 / TR 26.928: Study on XR evaluations, traffic models, and QoE metrics.
//!
//! Features:
//! 1. Multi-modal traffic representations: Video (I/P/B frames), 6DoF Pose, Spatial Audio, Haptic.
//! 2. Rel-18 PDCP/SDAP PDU Set header compact binary encoding and decoding (PSIE).
//! 3. PDU Set Delay Budget (PSDB) monitoring with microsecond-precision age validation.
//! 4. Cascading discard engine: drops dependent/subsequent packets when a PDU Set is rendered unrecoverable.
//! 5. Multi-modal priority multiplexing scheduler with pose preemption (< 5 ms MTP latency).
//! 6. XR traffic burst generator for 60 Hz, 90 Hz, and 120 Hz display cadences.
//! 7. Quality-of-Experience (QoE) metrics: Frame Success Rate (FSR), PSER, Goodput ratio, MTP latency.
//!
//! Pure standard Rust with zero external dependencies.

use std::collections::{HashMap, VecDeque};
use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Standard display refresh cadences in Hertz.
pub const XR_REFRESH_RATE_60_HZ: u32 = 60;
pub const XR_REFRESH_RATE_90_HZ: u32 = 90;
pub const XR_REFRESH_RATE_120_HZ: u32 = 120;

/// Standard frame intervals in microseconds.
pub const XR_FRAME_INTERVAL_60HZ_US: u64 = 16_667;
pub const XR_FRAME_INTERVAL_90HZ_US: u64 = 11_111;
pub const XR_FRAME_INTERVAL_120HZ_US: u64 = 8_333;

/// Default PDU Set Delay Budgets (PSDB) in microseconds.
pub const DEFAULT_PSDB_6DOF_POSE_US: u64 = 4_000; // 4 ms budget for 6DoF pose
pub const DEFAULT_PSDB_SPATIAL_AUDIO_US: u64 = 15_000; // 15 ms budget for audio
pub const DEFAULT_PSDB_VIDEO_IFRAME_US: u64 = 15_000; // 15 ms budget for I-Frame
pub const DEFAULT_PSDB_VIDEO_PFRAME_US: u64 = 10_000; // 10 ms budget for P-Frame
pub const DEFAULT_PSDB_HAPTIC_US: u64 = 5_000; // 5 ms budget for haptic

/// Maximum PDU payload size in bytes (Ethernet MTU standard).
pub const XR_DEFAULT_PDU_MTU_BYTES: usize = 1_400;

/// Rel-18 PSIE Header Size in bytes.
pub const PDU_SET_HEADER_SIZE_BYTES: usize = 15;

/// Bitmask flags for PDU Set Header Byte 0.
pub const PDU_SET_FLAG_PRESENT: u8 = 0x80;
pub const PDU_SET_FLAG_END_OF_SET: u8 = 0x40;
pub const PDU_SET_FLAG_IMPORTANCE_MASK: u8 = 0x0F;

// ---------------------------------------------------------------------------
// Modality & Traffic Types
// ---------------------------------------------------------------------------

/// Video frame type under video compression standard (H.264/H.265/AV1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoFrameType {
    /// Intra-coded keyframe (self-contained, essential anchor).
    IFrame,
    /// Predicted frame (depends on preceding reference frames).
    PFrame,
    /// Bi-directional predicted frame.
    BFrame,
}

/// XR traffic modality category.
#[derive(Debug, Clone, PartialEq)]
pub enum XrModality {
    /// Video frame stream with specific frame type.
    VideoFrame {
        frame_type: VideoFrameType,
        width: u32,
        height: u32,
    },
    /// 6-Degrees-of-Freedom (6DoF) Head-Mounted Display (HMD) / Controller pose.
    SixDofPose {
        seq: u32,
        x: f32,
        y: f32,
        z: f32,
        yaw: f32,
        pitch: f32,
        roll: f32,
    },
    /// Spatial 3D ambisonics audio stream.
    SpatialAudio { channels: u8, sample_rate_hz: u32 },
    /// Tactile/haptic actuator sensory packet.
    HapticFeedback {
        actuator_id: u8,
        intensity: u8,
        frequency_hz: u16,
    },
}

/// Modality discriminant for indexing and table lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XrModalityType {
    VideoIFrame,
    VideoPFrame,
    VideoBFrame,
    SixDofPose,
    SpatialAudio,
    HapticFeedback,
}

impl XrModality {
    /// Get the corresponding modality type discriminant.
    pub fn modality_type(&self) -> XrModalityType {
        match self {
            XrModality::VideoFrame { frame_type, .. } => match frame_type {
                VideoFrameType::IFrame => XrModalityType::VideoIFrame,
                VideoFrameType::PFrame => XrModalityType::VideoPFrame,
                VideoFrameType::BFrame => XrModalityType::VideoBFrame,
            },
            XrModality::SixDofPose { .. } => XrModalityType::SixDofPose,
            XrModality::SpatialAudio { .. } => XrModalityType::SpatialAudio,
            XrModality::HapticFeedback { .. } => XrModalityType::HapticFeedback,
        }
    }

    /// Return recommended default priority level (0 = highest priority).
    pub fn default_priority(&self) -> u8 {
        match self {
            XrModality::SixDofPose { .. } => 0, // Ultra-low latency tracking
            XrModality::HapticFeedback { .. } => 1, // Real-time sensory response
            XrModality::SpatialAudio { .. } => 2, // Continuous speech/audio
            XrModality::VideoFrame { frame_type, .. } => match frame_type {
                VideoFrameType::IFrame => 3, // Keyframe slice
                VideoFrameType::PFrame => 4, // Predicted frame slice
                VideoFrameType::BFrame => 5, // Bi-predictive frame slice
            },
        }
    }

    /// Return default PDU Set Importance (0–7, where 7 is highest).
    pub fn default_importance(&self) -> u8 {
        match self {
            XrModality::SixDofPose { .. } => 7,
            XrModality::VideoFrame { frame_type, .. } => match frame_type {
                VideoFrameType::IFrame => 6,
                VideoFrameType::PFrame => 4,
                VideoFrameType::BFrame => 2,
            },
            XrModality::HapticFeedback { .. } => 5,
            XrModality::SpatialAudio { .. } => 4,
        }
    }
}

// ---------------------------------------------------------------------------
// PDU Set Header & Codec
// ---------------------------------------------------------------------------

/// 3GPP Rel-18 PDU Set Header Information Element (PSIE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PduSetHeader {
    /// Whether PDU Set Information is present.
    pub pdu_set_present: bool,
    /// Whether this packet is the End of PDU Set (EOP).
    pub end_of_pdu_set: bool,
    /// Importance level of the PDU Set (0 to 7).
    pub importance: u8,
    /// PDU Set Sequence Number (PSSN, 0..=65535).
    pub pssn: u16,
    /// Sequence number of this PDU within the PDU Set (0..=255).
    pub psn: u8,
    /// Total number of PDUs composing this PDU Set (1..=255).
    pub pdu_set_size: u8,
    /// Microsecond timestamp when this PDU Set was generated at source application.
    pub generation_ts_us: u64,
    /// Payload length in bytes.
    pub payload_size_bytes: usize,
}

impl PduSetHeader {
    /// Create a new PduSetHeader with parameters.
    pub fn new(
        pssn: u16,
        psn: u8,
        pdu_set_size: u8,
        end_of_pdu_set: bool,
        importance: u8,
        generation_ts_us: u64,
        payload_size_bytes: usize,
    ) -> Self {
        Self {
            pdu_set_present: true,
            end_of_pdu_set,
            importance: importance.min(7),
            pssn,
            psn,
            pdu_set_size,
            generation_ts_us,
            payload_size_bytes,
        }
    }
}

/// Binary serializer and deserializer for Rel-18 PDU Set headers.
pub struct PduSetBinaryCodec;

impl PduSetBinaryCodec {
    /// Encode a PduSetHeader into a 15-byte wire representation.
    pub fn encode_header(header: &PduSetHeader) -> [u8; PDU_SET_HEADER_SIZE_BYTES] {
        let mut buf = [0u8; PDU_SET_HEADER_SIZE_BYTES];

        let mut flags = 0u8;
        if header.pdu_set_present {
            flags |= PDU_SET_FLAG_PRESENT;
        }
        if header.end_of_pdu_set {
            flags |= PDU_SET_FLAG_END_OF_SET;
        }
        flags |= header.importance & PDU_SET_FLAG_IMPORTANCE_MASK;
        buf[0] = flags;

        // Byte 1..2: PSSN (u16 BE)
        let pssn_bytes = header.pssn.to_be_bytes();
        buf[1] = pssn_bytes[0];
        buf[2] = pssn_bytes[1];

        // Byte 3: PSN (u8)
        buf[3] = header.psn;

        // Byte 4: PDU Set Size (u8)
        buf[4] = header.pdu_set_size;

        // Byte 5..12: Generation Timestamp (u64 BE)
        let ts_bytes = header.generation_ts_us.to_be_bytes();
        buf[5..13].copy_from_slice(&ts_bytes);

        // Byte 13..14: Payload length (u16 BE)
        let len = (header.payload_size_bytes as u16).to_be_bytes();
        buf[13] = len[0];
        buf[14] = len[1];

        buf
    }

    /// Decode a PduSetHeader from wire bytes.
    pub fn decode_header(buf: &[u8]) -> Result<PduSetHeader, XrError> {
        if buf.len() < PDU_SET_HEADER_SIZE_BYTES {
            return Err(XrError::BufferTooShort {
                needed: PDU_SET_HEADER_SIZE_BYTES,
                provided: buf.len(),
            });
        }

        let flags = buf[0];
        let pdu_set_present = (flags & PDU_SET_FLAG_PRESENT) != 0;
        let end_of_pdu_set = (flags & PDU_SET_FLAG_END_OF_SET) != 0;
        let importance = flags & PDU_SET_FLAG_IMPORTANCE_MASK;

        let pssn = u16::from_be_bytes([buf[1], buf[2]]);
        let psn = buf[3];
        let pdu_set_size = buf[4];

        let mut ts_bytes = [0u8; 8];
        ts_bytes.copy_from_slice(&buf[5..13]);
        let generation_ts_us = u64::from_be_bytes(ts_bytes);

        let payload_size_bytes = u16::from_be_bytes([buf[13], buf[14]]) as usize;

        Ok(PduSetHeader {
            pdu_set_present,
            end_of_pdu_set,
            importance,
            pssn,
            psn,
            pdu_set_size,
            generation_ts_us,
            payload_size_bytes,
        })
    }
}

// ---------------------------------------------------------------------------
// PDU Set Packet
// ---------------------------------------------------------------------------

/// Complete XR packet encapsulation with PDU Set header and application payload.
#[derive(Debug, Clone, PartialEq)]
pub struct PduSetPacket {
    pub header: PduSetHeader,
    pub modality: XrModality,
    pub payload: Vec<u8>,
}

impl PduSetPacket {
    /// Create a new PduSetPacket.
    pub fn new(header: PduSetHeader, modality: XrModality, payload: Vec<u8>) -> Self {
        let mut h = header;
        h.payload_size_bytes = payload.len();
        Self {
            header: h,
            modality,
            payload,
        }
    }

    /// Serialize full packet into wire format (15-byte header + payload).
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PDU_SET_HEADER_SIZE_BYTES + self.payload.len());
        let hdr_bytes = PduSetBinaryCodec::encode_header(&self.header);
        out.extend_from_slice(&hdr_bytes);
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parse full packet from wire format.
    pub fn deserialize(buf: &[u8], modality: XrModality) -> Result<Self, XrError> {
        let header = PduSetBinaryCodec::decode_header(buf)?;
        let payload_start = PDU_SET_HEADER_SIZE_BYTES;
        let payload_end = payload_start + header.payload_size_bytes;

        if buf.len() < payload_end {
            return Err(XrError::BufferTooShort {
                needed: payload_end,
                provided: buf.len(),
            });
        }

        let payload = buf[payload_start..payload_end].to_vec();
        Ok(Self {
            header,
            modality,
            payload,
        })
    }
}

// ---------------------------------------------------------------------------
// PDU Set Delay Budget (PSDB)
// ---------------------------------------------------------------------------

/// PDU Set Delay Budget configuration and verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PduSetDelayBudget {
    pub psdb_us: u64,
}

impl PduSetDelayBudget {
    /// Create a new PSDB with duration in microseconds.
    pub fn new(psdb_us: u64) -> Self {
        Self { psdb_us }
    }

    /// Check whether a packet has exceeded its delay budget.
    pub fn is_expired(&self, generation_ts_us: u64, current_ts_us: u64) -> bool {
        if current_ts_us < generation_ts_us {
            return false; // Clock skew or future packet
        }
        (current_ts_us - generation_ts_us) > self.psdb_us
    }

    /// Calculate remaining delay budget in microseconds (can be negative if expired).
    pub fn remaining_budget_us(&self, generation_ts_us: u64, current_ts_us: u64) -> i64 {
        let elapsed = if current_ts_us >= generation_ts_us {
            current_ts_us - generation_ts_us
        } else {
            0
        };
        self.psdb_us as i64 - elapsed as i64
    }
}

// ---------------------------------------------------------------------------
// Cascading Discard Engine
// ---------------------------------------------------------------------------

/// Reason why a PDU or PDU Set was discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardReason {
    /// Exceeded PDU Set Delay Budget (PSDB timeout).
    DelayBudgetExpired,
    /// Upstream critical slice or frame was lost (cascading dependency drop).
    CascadingDependencyLost,
    /// Invalid packet or sequence discontinuity.
    CorruptedPdu,
    /// Buffer queue overflow under heavy congestion.
    QueueOverflow,
}

/// Action determined by the discard manager for an incoming PDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PduHandlingAction {
    /// Deliver or forward PDU to next layer.
    Deliver,
    /// Discard this single PDU.
    DiscardSingle { reason: DiscardReason },
    /// Discard this PDU and propagate cascading discard to all other PDUs in this PSSN.
    TriggerCascadingDiscard { pssn: u16, reason: DiscardReason },
}

/// State tracking for a single active PDU Set.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ActivePduSetState {
    pssn: u16,
    total_pdus: u8,
    received_mask: Vec<bool>,
    is_discarded: bool,
    discard_reason: Option<DiscardReason>,
    modality_type: XrModalityType,
    generation_ts_us: u64,
}

/// Cascading discard engine enforcing PDU Set atomicity.
#[derive(Debug, Clone)]
pub struct CascadingDiscardManager {
    active_sets: HashMap<u16, ActivePduSetState>,
    discarded_pssns: VecDeque<(u16, u64)>, // (pssn, discard_timestamp)
    max_history_size: usize,
    pub total_accepted_pdus: u64,
    pub total_discarded_pdus: u64,
    pub total_cascading_drops: u64,
}

impl CascadingDiscardManager {
    /// Create a new CascadingDiscardManager with max history window.
    pub fn new(max_history_size: usize) -> Self {
        Self {
            active_sets: HashMap::new(),
            discarded_pssns: VecDeque::new(),
            max_history_size,
            total_accepted_pdus: 0,
            total_discarded_pdus: 0,
            total_cascading_drops: 0,
        }
    }

    /// Check if a PSSN is marked as discarded.
    pub fn is_pssn_discarded(&self, pssn: u16) -> bool {
        if let Some(set) = self.active_sets.get(&pssn) {
            if set.is_discarded {
                return true;
            }
        }
        self.discarded_pssns.iter().any(|(p, _)| *p == pssn)
    }

    /// Process an incoming PDU and determine handling action.
    pub fn process_pdu(
        &mut self,
        packet: &PduSetPacket,
        current_ts_us: u64,
        budget: &PduSetDelayBudget,
    ) -> PduHandlingAction {
        let pssn = packet.header.pssn;
        let psn = packet.header.psn;
        let set_size = packet.header.pdu_set_size;

        // 1. Check if the PSSN has already been marked as discarded
        if self.is_pssn_discarded(pssn) {
            self.total_discarded_pdus += 1;
            self.total_cascading_drops += 1;
            return PduHandlingAction::DiscardSingle {
                reason: DiscardReason::CascadingDependencyLost,
            };
        }

        // 2. Check PSDB expiration
        if budget.is_expired(packet.header.generation_ts_us, current_ts_us) {
            self.mark_pssn_discarded(pssn, DiscardReason::DelayBudgetExpired, current_ts_us);
            self.total_discarded_pdus += 1;
            return PduHandlingAction::TriggerCascadingDiscard {
                pssn,
                reason: DiscardReason::DelayBudgetExpired,
            };
        }

        // 3. Track in active set state
        let state = self
            .active_sets
            .entry(pssn)
            .or_insert_with(|| ActivePduSetState {
                pssn,
                total_pdus: set_size,
                received_mask: vec![false; set_size as usize],
                is_discarded: false,
                discard_reason: None,
                modality_type: packet.modality.modality_type(),
                generation_ts_us: packet.header.generation_ts_us,
            });

        if (psn as usize) < state.received_mask.len() {
            state.received_mask[psn as usize] = true;
        }

        self.total_accepted_pdus += 1;
        PduHandlingAction::Deliver
    }

    /// Mark a PSSN as discarded and record in history.
    pub fn mark_pssn_discarded(&mut self, pssn: u16, reason: DiscardReason, current_ts_us: u64) {
        if let Some(set) = self.active_sets.get_mut(&pssn) {
            set.is_discarded = true;
            set.discard_reason = Some(reason);
        }

        if !self.discarded_pssns.iter().any(|(p, _)| *p == pssn) {
            self.discarded_pssns.push_back((pssn, current_ts_us));
            if self.discarded_pssns.len() > self.max_history_size {
                self.discarded_pssns.pop_front();
            }
        }
    }

    /// Check if all PDUs of a PDU Set have been successfully received.
    pub fn is_pdu_set_complete(&self, pssn: u16) -> bool {
        if let Some(state) = self.active_sets.get(&pssn) {
            if state.is_discarded {
                return false;
            }
            state.received_mask.iter().all(|&r| r)
        } else {
            false
        }
    }

    /// Clean up completed or expired PDU Sets.
    pub fn prune_completed_sets(&mut self) {
        self.active_sets
            .retain(|_, state| !state.is_discarded && !state.received_mask.iter().all(|&r| r));
    }
}

// ---------------------------------------------------------------------------
// Multi-Modal Scheduler
// ---------------------------------------------------------------------------

/// Priority-based scheduler multiplexing 6DoF Pose, Audio, and Video streams.
pub struct XrMultiModalScheduler {
    pose_queue: VecDeque<PduSetPacket>,
    haptic_queue: VecDeque<PduSetPacket>,
    audio_queue: VecDeque<PduSetPacket>,
    video_queue: VecDeque<PduSetPacket>,
    discard_manager: CascadingDiscardManager,
    psdb_table: HashMap<XrModalityType, PduSetDelayBudget>,
    pub max_queue_depth_per_modality: usize,
}

impl XrMultiModalScheduler {
    /// Create a new multi-modal scheduler.
    pub fn new(max_queue_depth: usize) -> Self {
        let mut psdb_table = HashMap::new();
        psdb_table.insert(
            XrModalityType::SixDofPose,
            PduSetDelayBudget::new(DEFAULT_PSDB_6DOF_POSE_US),
        );
        psdb_table.insert(
            XrModalityType::HapticFeedback,
            PduSetDelayBudget::new(DEFAULT_PSDB_HAPTIC_US),
        );
        psdb_table.insert(
            XrModalityType::SpatialAudio,
            PduSetDelayBudget::new(DEFAULT_PSDB_SPATIAL_AUDIO_US),
        );
        psdb_table.insert(
            XrModalityType::VideoIFrame,
            PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_IFRAME_US),
        );
        psdb_table.insert(
            XrModalityType::VideoPFrame,
            PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_PFRAME_US),
        );
        psdb_table.insert(
            XrModalityType::VideoBFrame,
            PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_PFRAME_US),
        );

        Self {
            pose_queue: VecDeque::with_capacity(max_queue_depth),
            haptic_queue: VecDeque::with_capacity(max_queue_depth),
            audio_queue: VecDeque::with_capacity(max_queue_depth),
            video_queue: VecDeque::with_capacity(max_queue_depth),
            discard_manager: CascadingDiscardManager::new(256),
            psdb_table,
            max_queue_depth_per_modality: max_queue_depth,
        }
    }

    /// Set customized PSDB for a modality.
    pub fn set_psdb(&mut self, modality: XrModalityType, budget_us: u64) {
        self.psdb_table
            .insert(modality, PduSetDelayBudget::new(budget_us));
    }

    /// Access the cascading discard manager.
    pub fn discard_manager(&mut self) -> &mut CascadingDiscardManager {
        &mut self.discard_manager
    }

    /// Enqueue an incoming packet.
    pub fn enqueue(
        &mut self,
        packet: PduSetPacket,
        current_ts_us: u64,
    ) -> Result<PduHandlingAction, XrError> {
        let mod_type = packet.modality.modality_type();
        let budget = self
            .psdb_table
            .get(&mod_type)
            .cloned()
            .unwrap_or(PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_PFRAME_US));

        let action = self
            .discard_manager
            .process_pdu(&packet, current_ts_us, &budget);

        if action != PduHandlingAction::Deliver {
            return Ok(action);
        }

        // Queue according to modality priority
        match &packet.modality {
            XrModality::SixDofPose { .. } => {
                if self.pose_queue.len() >= self.max_queue_depth_per_modality {
                    self.pose_queue.pop_front(); // Evict oldest pose sample (freshness priority)
                }
                self.pose_queue.push_back(packet);
            }
            XrModality::HapticFeedback { .. } => {
                if self.haptic_queue.len() < self.max_queue_depth_per_modality {
                    self.haptic_queue.push_back(packet);
                }
            }
            XrModality::SpatialAudio { .. } => {
                if self.audio_queue.len() < self.max_queue_depth_per_modality {
                    self.audio_queue.push_back(packet);
                }
            }
            XrModality::VideoFrame { .. } => {
                if self.video_queue.len() < self.max_queue_depth_per_modality {
                    self.video_queue.push_back(packet);
                }
            }
        }

        Ok(PduHandlingAction::Deliver)
    }

    /// Schedule and dequeue the highest-priority, non-expired packet.
    pub fn schedule_next(&mut self, current_ts_us: u64) -> Option<PduSetPacket> {
        // Priority order: Pose (0) > Haptic (1) > Audio (2) > Video (3)
        if let Some(pkt) = self.dequeue_valid_packet(0, current_ts_us) {
            return Some(pkt);
        }
        if let Some(pkt) = self.dequeue_valid_packet(1, current_ts_us) {
            return Some(pkt);
        }
        if let Some(pkt) = self.dequeue_valid_packet(2, current_ts_us) {
            return Some(pkt);
        }
        if let Some(pkt) = self.dequeue_valid_packet(3, current_ts_us) {
            return Some(pkt);
        }
        None
    }

    /// Helper to dequeue the head packet from a priority queue, discarding if expired.
    fn dequeue_valid_packet(&mut self, priority: u8, current_ts_us: u64) -> Option<PduSetPacket> {
        let queue = match priority {
            0 => &mut self.pose_queue,
            1 => &mut self.haptic_queue,
            2 => &mut self.audio_queue,
            _ => &mut self.video_queue,
        };

        while let Some(pkt) = queue.pop_front() {
            let mod_type = pkt.modality.modality_type();
            let budget = self
                .psdb_table
                .get(&mod_type)
                .cloned()
                .unwrap_or(PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_PFRAME_US));

            if self.discard_manager.is_pssn_discarded(pkt.header.pssn) {
                self.discard_manager.total_discarded_pdus += 1;
                self.discard_manager.total_cascading_drops += 1;
                continue;
            }

            if budget.is_expired(pkt.header.generation_ts_us, current_ts_us) {
                self.discard_manager.mark_pssn_discarded(
                    pkt.header.pssn,
                    DiscardReason::DelayBudgetExpired,
                    current_ts_us,
                );
                self.discard_manager.total_discarded_pdus += 1;
                continue;
            }

            return Some(pkt);
        }

        None
    }

    /// Schedule a transmission grant of maximum bytes.
    pub fn schedule_grant(
        &mut self,
        max_grant_bytes: usize,
        current_ts_us: u64,
    ) -> Vec<PduSetPacket> {
        let mut granted = Vec::new();
        let mut consumed_bytes = 0;

        while let Some(pkt) = self.schedule_next(current_ts_us) {
            let pkt_len = PDU_SET_HEADER_SIZE_BYTES + pkt.payload.len();
            if consumed_bytes + pkt_len > max_grant_bytes && !granted.is_empty() {
                // Return packet back to the front of its queue
                self.requeue_front(pkt);
                break;
            }
            consumed_bytes += pkt_len;
            granted.push(pkt);
        }

        granted
    }

    fn requeue_front(&mut self, pkt: PduSetPacket) {
        match pkt.modality {
            XrModality::SixDofPose { .. } => self.pose_queue.push_front(pkt),
            XrModality::HapticFeedback { .. } => self.haptic_queue.push_front(pkt),
            XrModality::SpatialAudio { .. } => self.audio_queue.push_front(pkt),
            XrModality::VideoFrame { .. } => self.video_queue.push_front(pkt),
        }
    }

    /// Total packets currently queued across all modalities.
    pub fn total_queued_packets(&self) -> usize {
        self.pose_queue.len()
            + self.haptic_queue.len()
            + self.audio_queue.len()
            + self.video_queue.len()
    }
}

// ---------------------------------------------------------------------------
// XR Traffic Burst Generator
// ---------------------------------------------------------------------------

/// Generator for realistic XR multi-modal traffic bursts.
pub struct XrTrafficGenerator {
    pub refresh_rate_hz: u32,
    pub pdu_mtu_bytes: usize,
    next_pssn: u16,
    pose_seq: u32,
}

impl XrTrafficGenerator {
    /// Create a new traffic generator for a given refresh rate.
    pub fn new(refresh_rate_hz: u32, pdu_mtu_bytes: usize) -> Self {
        Self {
            refresh_rate_hz,
            pdu_mtu_bytes: pdu_mtu_bytes.max(100),
            next_pssn: 1,
            pose_seq: 1,
        }
    }

    /// Generate a 6DoF pose packet.
    pub fn generate_pose(&mut self, generation_ts_us: u64, x: f32, y: f32, z: f32) -> PduSetPacket {
        let pssn = self.next_pssn;
        self.next_pssn = self.next_pssn.wrapping_add(1);

        let modality = XrModality::SixDofPose {
            seq: self.pose_seq,
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        };
        self.pose_seq += 1;

        // Pose payload is 24 bytes (6 * f32)
        let mut payload = Vec::with_capacity(24);
        payload.extend_from_slice(&x.to_be_bytes());
        payload.extend_from_slice(&y.to_be_bytes());
        payload.extend_from_slice(&z.to_be_bytes());
        payload.extend_from_slice(&0.0f32.to_be_bytes());
        payload.extend_from_slice(&0.0f32.to_be_bytes());
        payload.extend_from_slice(&0.0f32.to_be_bytes());

        let header = PduSetHeader::new(
            pssn,
            0,
            1,
            true,
            modality.default_importance(),
            generation_ts_us,
            payload.len(),
        );

        PduSetPacket::new(header, modality, payload)
    }

    /// Generate a video frame sliced into MTU-sized PDU Set packets.
    pub fn generate_video_frame(
        &mut self,
        frame_type: VideoFrameType,
        total_frame_bytes: usize,
        generation_ts_us: u64,
    ) -> Vec<PduSetPacket> {
        let pssn = self.next_pssn;
        self.next_pssn = self.next_pssn.wrapping_add(1);

        let num_pdus = ((total_frame_bytes + self.pdu_mtu_bytes - 1) / self.pdu_mtu_bytes).max(1);
        let num_pdus_u8 = num_pdus.min(255) as u8;

        let modality = XrModality::VideoFrame {
            frame_type,
            width: 3840,
            height: 2160,
        };
        let importance = modality.default_importance();

        let mut packets = Vec::with_capacity(num_pdus);
        let mut remaining_bytes = total_frame_bytes;

        for psn in 0..num_pdus_u8 {
            let slice_size = remaining_bytes.min(self.pdu_mtu_bytes);
            remaining_bytes -= slice_size;
            let is_eop = psn == num_pdus_u8 - 1;

            let header = PduSetHeader::new(
                pssn,
                psn,
                num_pdus_u8,
                is_eop,
                importance,
                generation_ts_us,
                slice_size,
            );

            // Dummy video slice payload with pattern
            let payload = vec![(psn ^ 0xAA) as u8; slice_size];
            packets.push(PduSetPacket::new(header, modality.clone(), payload));
        }

        packets
    }

    /// Generate spatial audio packet.
    pub fn generate_audio(&mut self, generation_ts_us: u64, audio_bytes: usize) -> PduSetPacket {
        let pssn = self.next_pssn;
        self.next_pssn = self.next_pssn.wrapping_add(1);

        let modality = XrModality::SpatialAudio {
            channels: 4,
            sample_rate_hz: 48_000,
        };

        let payload = vec![0x55; audio_bytes];
        let header = PduSetHeader::new(
            pssn,
            0,
            1,
            true,
            modality.default_importance(),
            generation_ts_us,
            payload.len(),
        );

        PduSetPacket::new(header, modality, payload)
    }
}

// ---------------------------------------------------------------------------
// Quality of Experience (QoE) Metrics Tracker
// ---------------------------------------------------------------------------

/// Rel-18 XR QoE & Performance Metrics Tracker.
#[derive(Debug, Clone, Default)]
pub struct XrQoeTracker {
    pub total_frames_sent: u64,
    pub total_frames_completed_on_time: u64,
    pub total_frames_dropped_or_late: u64,
    pub total_bytes_transmitted: u64,
    pub total_goodput_bytes: u64,
    pub total_cascading_saved_bytes: u64,
    pub total_mtp_latency_sum_us: u64,
    pub mtp_latency_sample_count: u64,
}

impl XrQoeTracker {
    /// Create a new QoE tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successfully completed on-time PDU Set (Frame).
    pub fn record_frame_success(&mut self, total_frame_bytes: usize, latency_us: u64) {
        self.total_frames_sent += 1;
        self.total_frames_completed_on_time += 1;
        self.total_bytes_transmitted += total_frame_bytes as u64;
        self.total_goodput_bytes += total_frame_bytes as u64;
        self.total_mtp_latency_sum_us += latency_us;
        self.mtp_latency_sample_count += 1;
    }

    /// Record a failed/incomplete/expired frame.
    pub fn record_frame_failure(
        &mut self,
        transmitted_bytes_before_drop: usize,
        saved_bytes_by_cascading_discard: usize,
    ) {
        self.total_frames_sent += 1;
        self.total_frames_dropped_or_late += 1;
        self.total_bytes_transmitted += transmitted_bytes_before_drop as u64;
        self.total_cascading_saved_bytes += saved_bytes_by_cascading_discard as u64;
    }

    /// Calculate Frame Success Rate (FSR) in percentage (0.0 to 100.0).
    pub fn frame_success_rate(&self) -> f64 {
        if self.total_frames_sent == 0 {
            return 100.0;
        }
        (self.total_frames_completed_on_time as f64 / self.total_frames_sent as f64) * 100.0
    }

    /// Calculate PDU Set Error Rate (PSER) (0.0 to 1.0).
    pub fn pdu_set_error_rate(&self) -> f64 {
        if self.total_frames_sent == 0 {
            return 0.0;
        }
        self.total_frames_dropped_or_late as f64 / self.total_frames_sent as f64
    }

    /// Calculate Goodput ratio in percentage (Goodput / Total Transmitted Bytes).
    pub fn goodput_ratio(&self) -> f64 {
        if self.total_bytes_transmitted == 0 {
            return 100.0;
        }
        (self.total_goodput_bytes as f64 / self.total_bytes_transmitted as f64) * 100.0
    }

    /// Average Motion-to-Photon (MTP) latency in milliseconds.
    pub fn average_mtp_latency_ms(&self) -> f64 {
        if self.mtp_latency_sample_count == 0 {
            return 0.0;
        }
        (self.total_mtp_latency_sum_us as f64 / self.mtp_latency_sample_count as f64) / 1000.0
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors encountered in XR PDU Set processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrError {
    BufferTooShort { needed: usize, provided: usize },
    InvalidHeaderFlags(u8),
    PduSetSizeMismatch { expected: u8, got: u8 },
    QueueFull { capacity: usize },
    UnknownModality,
}

impl fmt::Display for XrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XrError::BufferTooShort { needed, provided } => {
                write!(
                    f,
                    "XR PDU Set buffer too short: needed {} bytes, got {}",
                    needed, provided
                )
            }
            XrError::InvalidHeaderFlags(flags) => {
                write!(f, "Invalid PDU Set header flags: 0x{:02X}", flags)
            }
            XrError::PduSetSizeMismatch { expected, got } => {
                write!(
                    f,
                    "PDU Set size mismatch: expected {}, got {}",
                    expected, got
                )
            }
            XrError::QueueFull { capacity } => {
                write!(f, "XR priority queue full: capacity {}", capacity)
            }
            XrError::UnknownModality => write!(f, "Unknown or unsupported XR modality"),
        }
    }
}

impl std::error::Error for XrError {}
