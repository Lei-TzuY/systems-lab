//! 3GPP TS 23.501 §5.8.4 / TS 29.244 / TS 23.502 §4.2.3.3 Release 17 5G UPF Downlink Data Buffering Engine.
//!
//! Implements 5G UPF User-Plane Downlink Data Buffering, Paging Triggering, and FIFO Flushing:
//! - Buffering Action Rule (BAR) execution during UE CM-IDLE state (TS 29.244 Section 5.2.4 & 8.2.14)
//! - Downlink Data Notification (DDN) / Downlink Data Report (DDR) generation for SMF
//! - Paging Policy Indicator (PPI) derivation based on IP DSCP / 5QI / QFI priority
//! - Buffer Quota and Drop Policies: `DropOldest` (head drop), `DropNewest` (tail drop), `PriorityDrop`
//! - Downlink Data Notification Delay (DDND) timer support
//! - FIFO buffer flushing and 5G GTP-U encapsulation (with QFI PDU Session Container) upon wake-up
//! - Buffer purge and telemetry upon Paging timeout
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::collections::HashMap;
use std::net::Ipv4Addr;

// ---------------------------------------------------------------------------
// Constants and Enums
// ---------------------------------------------------------------------------

/// Default buffer capacity per session.
pub const DEFAULT_MAX_BUFFER_PACKETS: usize = 100;
pub const DEFAULT_MAX_BUFFER_BYTES: usize = 128 * 1024; // 128 KB

/// Buffer Overflow Drop Policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferDropPolicy {
    /// Drop the oldest packet at the head of the FIFO queue.
    DropOldest,
    /// Drop the incoming newest packet.
    DropNewest,
    /// Drop the lowest priority QFI packet currently in the buffer.
    PriorityDrop,
}

/// Buffering Action Rule (BAR) Configuration (TS 29.244 Section 8.2.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarConfig {
    pub bar_id: u16,
    /// Delay before emitting Downlink Data Notification (ms). 0 = immediate.
    pub ddn_delay_ms: u64,
    /// Suggested maximum number of packets to buffer.
    pub suggested_packet_count: usize,
    /// Maximum time to hold packets before discarding (ms).
    pub max_hold_time_ms: u64,
}

impl Default for BarConfig {
    fn default() -> Self {
        BarConfig {
            bar_id: 1,
            ddn_delay_ms: 0,
            suggested_packet_count: 10,
            max_hold_time_ms: 30_000, // 30 seconds
        }
    }
}

/// Downlink Buffered Packet Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedDlPacket {
    pub payload: Vec<u8>,
    pub qfi: u8,
    pub dscp: u8,
    pub arrival_time_ms: u64,
    pub packet_size: usize,
}

/// Downlink Data Report (DDR) emitted by UPF to SMF over N4 PFCP (TS 29.244 Section 5.2.2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownlinkDataReport {
    pub seid: u64,
    pub pdr_id: u16,
    pub qfi: u8,
    pub ppi: Option<u8>, // Paging Policy Indicator (1..8)
    pub first_packet_timestamp_ms: u64,
}

/// Encapsulated GTP-U packet dispatched upon buffer flush.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushedGtpPacket {
    pub teid: u32,
    pub gnb_ip: Ipv4Addr,
    pub qfi: u8,
    pub gtpu_packet: Vec<u8>,
}

/// UPF Buffering Error Types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpfBufferingError {
    SessionNotFound(u64),
    BufferFull,
    InvalidPacket,
}

/// Cumulative Buffering Statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BufferingStats {
    pub total_buffered_packets: u64,
    pub total_buffered_bytes: u64,
    pub total_flushed_packets: u64,
    pub total_flushed_bytes: u64,
    pub total_dropped_overflow_packets: u64,
    pub total_dropped_overflow_bytes: u64,
    pub total_purged_packets: u64,
    pub total_purged_bytes: u64,
    pub total_ddn_reports_generated: u64,
}

// ---------------------------------------------------------------------------
// Helper: Paging Policy Indicator (PPI) Derivation
// ---------------------------------------------------------------------------

/// Derive standard Paging Policy Indicator (PPI) from QFI and IP DSCP (TS 23.501 §5.4.3).
pub fn derive_ppi(qfi: u8, dscp: u8) -> Option<u8> {
    // Priority 1: Mission Critical / Voice (QFI 1 or DSCP Expedited Forwarding EF = 46)
    if qfi == 1 || dscp == 46 {
        return Some(1);
    }
    // Priority 2: IMS Signalling (QFI 5 or DSCP CS5 = 40, CS3 = 24)
    if qfi == 5 || dscp == 40 || dscp == 24 {
        return Some(2);
    }
    // Priority 3: Delay-Critical Interactive / Video (QFI 2 or 3 or DSCP AF41 = 34)
    if qfi == 2 || qfi == 3 || dscp == 34 {
        return Some(3);
    }
    // Default best-effort: No special PPI needed
    None
}

// ---------------------------------------------------------------------------
// Helper: 5G GTP-U Encapsulation with QFI PDU Session Container (TS 38.415)
// ---------------------------------------------------------------------------

/// Encapsulate a user-plane payload into a 5G GTP-U packet with 4-byte Extension Header.
pub fn build_5g_gtpu_packet(teid: u32, qfi: u8, payload: &[u8]) -> Vec<u8> {
    // Standard GTP-U Header (8 bytes) + PDU Session Container Extension Header (4 bytes) + Payload
    let total_ext_len = 4;
    let length_field = (payload.len() + total_ext_len) as u16;

    let mut packet = Vec::with_capacity(12 + payload.len());
    // Byte 0: Version 1, Protocol Type 1, Extension Header Flag E=1 (0x34)
    packet.push(0x34);
    // Byte 1: Message Type 0xFF (G-PDU)
    packet.push(0xFF);
    // Bytes 2..3: Length
    packet.extend_from_slice(&length_field.to_be_bytes());
    // Bytes 4..7: TEID
    packet.extend_from_slice(&teid.to_be_bytes());

    // Extension Header:
    // Byte 8: Sequence Number (0)
    packet.push(0x00);
    // Byte 9: N-PDU Number (0)
    packet.push(0x00);
    // Byte 10: Next Extension Header Type 0x85 (PDU Session Container)
    packet.push(0x85);
    // Byte 11: Extension Header Length in 4-octet units (1)
    packet.push(0x01);

    // PDU Session Container (TS 38.415 Section 5.2):
    // [PDU Type: 0 (DL), QFI: 6 bits]
    let qfi_byte = qfi & 0x3F;
    packet.push(qfi_byte);
    packet.push(0x00); // RQI = 0
    packet.push(0x00); // Spare
    packet.push(0x00); // Next Extension Header = 0 (No more extensions)

    packet.extend_from_slice(payload);
    packet
}

// ---------------------------------------------------------------------------
// Per-Session Buffer Context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SessionBufferContext {
    pub seid: u64,
    pub pdr_id: u16,
    pub bar_config: BarConfig,
    pub max_buffer_packets: usize,
    pub max_buffer_bytes: usize,
    pub current_bytes: usize,
    pub drop_policy: BufferDropPolicy,
    pub ddn_sent: bool,
    pub first_packet_time_ms: Option<u64>,
    pub packets: Vec<BufferedDlPacket>,
}

impl SessionBufferContext {
    pub fn new(
        seid: u64,
        pdr_id: u16,
        bar_config: BarConfig,
        max_buffer_packets: usize,
        max_buffer_bytes: usize,
        drop_policy: BufferDropPolicy,
    ) -> Self {
        SessionBufferContext {
            seid,
            pdr_id,
            bar_config,
            max_buffer_packets,
            max_buffer_bytes,
            current_bytes: 0,
            drop_policy,
            ddn_sent: false,
            first_packet_time_ms: None,
            packets: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Top-Level 5G UPF Downlink Buffering Engine
// ---------------------------------------------------------------------------

pub struct UpfBufferingEngine {
    pub sessions: HashMap<u64, SessionBufferContext>,
    pub stats: BufferingStats,
}

impl UpfBufferingEngine {
    /// Create a new UPF Buffering Engine.
    pub fn new() -> Self {
        UpfBufferingEngine {
            sessions: HashMap::new(),
            stats: BufferingStats::default(),
        }
    }

    /// Install or update buffering rules for a PFCP Session.
    pub fn configure_session_buffer(
        &mut self,
        seid: u64,
        pdr_id: u16,
        bar_config: BarConfig,
        max_buffer_packets: usize,
        max_buffer_bytes: usize,
        drop_policy: BufferDropPolicy,
    ) {
        let ctx = SessionBufferContext::new(
            seid,
            pdr_id,
            bar_config,
            max_buffer_packets,
            max_buffer_bytes,
            drop_policy,
        );
        self.sessions.insert(seid, ctx);
    }

    /// Ingest a downlink packet arriving for an idle/buffered session.
    /// Returns `Some(DownlinkDataReport)` if a new DDN should be dispatched immediately.
    pub fn buffer_downlink_packet(
        &mut self,
        seid: u64,
        payload: Vec<u8>,
        qfi: u8,
        dscp: u8,
        current_time_ms: u64,
    ) -> Result<Option<DownlinkDataReport>, UpfBufferingError> {
        let session = self
            .sessions
            .get_mut(&seid)
            .ok_or(UpfBufferingError::SessionNotFound(seid))?;

        let packet_size = payload.len();

        // Check buffer limits
        let exceeds_packets = session.packets.len() >= session.max_buffer_packets;
        let exceeds_bytes = session.current_bytes + packet_size > session.max_buffer_bytes;

        if exceeds_packets || exceeds_bytes {
            match session.drop_policy {
                BufferDropPolicy::DropNewest => {
                    self.stats.total_dropped_overflow_packets += 1;
                    self.stats.total_dropped_overflow_bytes += packet_size as u64;
                    return Ok(None);
                }
                BufferDropPolicy::DropOldest => {
                    if !session.packets.is_empty() {
                        let dropped = session.packets.remove(0);
                        session.current_bytes =
                            session.current_bytes.saturating_sub(dropped.packet_size);
                        self.stats.total_dropped_overflow_packets += 1;
                        self.stats.total_dropped_overflow_bytes += dropped.packet_size as u64;
                    }
                }
                BufferDropPolicy::PriorityDrop => {
                    // Find packet with highest QFI number (lowest priority, since QFI 1 is highest priority)
                    let mut lowest_prio_idx = None;
                    let mut highest_qfi = qfi; // Only drop if existing packet has worse priority

                    for (idx, p) in session.packets.iter().enumerate() {
                        if p.qfi > highest_qfi {
                            highest_qfi = p.qfi;
                            lowest_prio_idx = Some(idx);
                        }
                    }

                    if let Some(idx) = lowest_prio_idx {
                        let dropped = session.packets.remove(idx);
                        session.current_bytes =
                            session.current_bytes.saturating_sub(dropped.packet_size);
                        self.stats.total_dropped_overflow_packets += 1;
                        self.stats.total_dropped_overflow_bytes += dropped.packet_size as u64;
                    } else if exceeds_packets || exceeds_bytes {
                        // Incoming packet has lower/equal priority, drop it
                        self.stats.total_dropped_overflow_packets += 1;
                        self.stats.total_dropped_overflow_bytes += packet_size as u64;
                        return Ok(None);
                    }
                }
            }
        }

        // Store packet in FIFO
        session.current_bytes += packet_size;
        session.packets.push(BufferedDlPacket {
            payload,
            qfi,
            dscp,
            arrival_time_ms: current_time_ms,
            packet_size,
        });

        self.stats.total_buffered_packets += 1;
        self.stats.total_buffered_bytes += packet_size as u64;

        if session.first_packet_time_ms.is_none() {
            session.first_packet_time_ms = Some(current_time_ms);
        }

        // Check if DDN report should be emitted
        if !session.ddn_sent {
            // If DDND delay is 0, emit immediately; otherwise wait for timer tick
            if session.bar_config.ddn_delay_ms == 0 {
                session.ddn_sent = true;
                let ppi = derive_ppi(qfi, dscp);
                self.stats.total_ddn_reports_generated += 1;

                Ok(Some(DownlinkDataReport {
                    seid,
                    pdr_id: session.pdr_id,
                    qfi,
                    ppi,
                    first_packet_timestamp_ms: current_time_ms,
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Periodic tick to check and emit delayed DDN reports whose DDND timer has expired.
    pub fn check_delayed_ddn(&mut self, current_time_ms: u64) -> Vec<DownlinkDataReport> {
        let mut reports = Vec::new();
        for session in self.sessions.values_mut() {
            if !session.ddn_sent && !session.packets.is_empty() {
                if let Some(first_time) = session.first_packet_time_ms {
                    if current_time_ms.saturating_sub(first_time) >= session.bar_config.ddn_delay_ms
                    {
                        session.ddn_sent = true;
                        let first_pkt = &session.packets[0];
                        let ppi = derive_ppi(first_pkt.qfi, first_pkt.dscp);

                        reports.push(DownlinkDataReport {
                            seid: session.seid,
                            pdr_id: session.pdr_id,
                            qfi: first_pkt.qfi,
                            ppi,
                            first_packet_timestamp_ms: first_time,
                        });
                    }
                }
            }
        }
        self.stats.total_ddn_reports_generated += reports.len() as u64;
        reports
    }

    /// Flush all buffered packets upon UE wake-up and gNodeB tunnel establishment.
    /// Wraps packets in GTP-U and returns them in chronological order.
    pub fn flush_buffer(
        &mut self,
        seid: u64,
        gnb_teid: u32,
        gnb_ip: Ipv4Addr,
    ) -> Result<Vec<FlushedGtpPacket>, UpfBufferingError> {
        let session = self
            .sessions
            .get_mut(&seid)
            .ok_or(UpfBufferingError::SessionNotFound(seid))?;

        let mut flushed = Vec::with_capacity(session.packets.len());
        for pkt in session.packets.drain(..) {
            let gtpu = build_5g_gtpu_packet(gnb_teid, pkt.qfi, &pkt.payload);
            self.stats.total_flushed_packets += 1;
            self.stats.total_flushed_bytes += pkt.packet_size as u64;

            flushed.push(FlushedGtpPacket {
                teid: gnb_teid,
                gnb_ip,
                qfi: pkt.qfi,
                gtpu_packet: gtpu,
            });
        }

        session.current_bytes = 0;
        session.ddn_sent = false;
        session.first_packet_time_ms = None;

        Ok(flushed)
    }

    /// Purge buffered packets (e.g. upon Paging timeout or session release).
    /// Returns `(purged_packets_count, purged_bytes)`.
    pub fn purge_buffer(&mut self, seid: u64) -> Result<(usize, usize), UpfBufferingError> {
        let session = self
            .sessions
            .get_mut(&seid)
            .ok_or(UpfBufferingError::SessionNotFound(seid))?;

        let count = session.packets.len();
        let bytes = session.current_bytes;

        session.packets.clear();
        session.current_bytes = 0;
        session.ddn_sent = false;
        session.first_packet_time_ms = None;

        self.stats.total_purged_packets += count as u64;
        self.stats.total_purged_bytes += bytes as u64;

        Ok((count, bytes))
    }

    /// Get current buffer metrics for a session: `(packet_count, byte_count)`.
    pub fn get_session_stats(&self, seid: u64) -> Option<(usize, usize)> {
        self.sessions
            .get(&seid)
            .map(|s| (s.packets.len(), s.current_bytes))
    }
}
