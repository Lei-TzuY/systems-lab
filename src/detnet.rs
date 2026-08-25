//! DetNet: Deterministic Networking Data Plane & PREF Engine (RFC 8655 / 8938 / 8939 / 8964).
//!
//! Implements DetNet Control Word (d-CW), DetNet-over-UDP (Port 3636) encapsulation,
//! and Packet Replication and Elimination Function (PREF) with sliding window sequence
//! deduplication for zero-loss deterministic industrial and datacenter communication.

use crate::ipv4::Ipv4Address;

/// Default DetNet over UDP Port (RFC 8939 Section 4.1).
pub const DETNET_UDP_PORT: u16 = 3636;

/// 4-byte DetNet Control Word (d-CW) (RFC 8964 Section 4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetNetControlWord {
    /// 16-bit or 28-bit sequence number (represented as u32).
    pub sequence_number: u32,
    /// 6-bit control flags (e.g. OAM bit, S-bit).
    pub flags: u8,
}

impl DetNetControlWord {
    pub fn new(sequence_number: u32) -> Self {
        DetNetControlWord {
            sequence_number,
            flags: 0,
        }
    }

    /// Serializes the 4-byte DetNet Control Word.
    pub fn serialize(&self) -> [u8; 4] {
        let seq16 = (self.sequence_number & 0xFFFF) as u16;
        let mut bytes = [0u8; 4];
        bytes[0] = (self.flags << 2) & 0xFC;
        bytes[1] = 0x00; // Reserved
        bytes[2..4].copy_from_slice(&seq16.to_be_bytes());
        bytes
    }

    /// Parses the 4-byte DetNet Control Word.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let flags = (bytes[0] >> 2) & 0x3F;
        let seq16 = u16::from_be_bytes([bytes[2], bytes[3]]) as u32;
        Some(DetNetControlWord {
            sequence_number: seq16,
            flags,
        })
    }
}

/// A complete DetNet data packet encapsulated in DetNet-over-UDP/IP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetNetPacket {
    pub flow_id: u32,
    pub control_word: DetNetControlWord,
    pub payload: Vec<u8>,
}

impl DetNetPacket {
    pub fn new(flow_id: u32, sequence_number: u32, payload: Vec<u8>) -> Self {
        DetNetPacket {
            flow_id,
            control_word: DetNetControlWord::new(sequence_number),
            payload,
        }
    }

    /// Encodes DetNet packet: [4-byte Flow ID | 4-byte Control Word | Payload].
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + self.payload.len());
        buf.extend_from_slice(&self.flow_id.to_be_bytes());
        buf.extend_from_slice(&self.control_word.serialize());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decodes a DetNet packet from raw payload bytes.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 8 {
            return None;
        }
        let flow_id = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let control_word = DetNetControlWord::parse(&buf[4..8])?;
        let payload = buf[8..].to_vec();
        Some(DetNetPacket {
            flow_id,
            control_word,
            payload,
        })
    }
}

/// DetNet Flow Identifier based on IP 5-tuple or Flow ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DetNetFlowKey {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub flow_id: u32,
}

/// Statistics for a DetNet Elimination node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetNetStats {
    pub packets_received: usize,
    pub packets_forwarded: usize,
    pub duplicates_dropped: usize,
    pub out_of_order_packets: usize,
}

/// Sliding-window Sequence Elimination Filter for a single DetNet flow.
#[derive(Debug, Clone)]
pub struct DetNetEliminationFilter {
    pub window_size: usize,
    pub highest_seq: u32,
    pub initialized: bool,
    pub seen_history: Vec<u32>,
    pub stats: DetNetStats,
}

impl DetNetEliminationFilter {
    pub fn new(window_size: usize) -> Self {
        DetNetEliminationFilter {
            window_size: window_size.clamp(16, 2048),
            highest_seq: 0,
            initialized: false,
            seen_history: Vec::new(),
            stats: DetNetStats::default(),
        }
    }

    /// Evaluates an incoming packet sequence number:
    /// Returns `true` if the packet is ACCEPTED (first arrival),
    /// or `false` if it is a DUPLICATE and should be ELIMINATED.
    pub fn process_sequence(&mut self, seq: u32) -> bool {
        self.stats.packets_received += 1;

        if !self.initialized {
            self.initialized = true;
            self.highest_seq = seq;
            self.seen_history.push(seq);
            self.stats.packets_forwarded += 1;
            return true;
        }

        // Check if sequence is already in the recent history window
        if self.seen_history.contains(&seq) {
            self.stats.duplicates_dropped += 1;
            return false;
        }

        // Check sequence advancement with 16-bit wrap-around comparison
        let diff = (seq.wrapping_sub(self.highest_seq)) & 0xFFFF;
        if diff > 0 && diff < 0x8000 {
            // New forward sequence number
            self.highest_seq = seq;
        } else {
            // Out-of-order or delayed arrival
            self.stats.out_of_order_packets += 1;
        }

        self.seen_history.push(seq);
        if self.seen_history.len() > self.window_size {
            self.seen_history.remove(0);
        }

        self.stats.packets_forwarded += 1;
        true
    }
}

/// DetNet Packet Replication and Elimination Function (PREF) Engine.
#[derive(Debug, Clone)]
pub struct DetNetPrefEngine {
    pub next_tx_seq: u32,
    pub filters: std::collections::HashMap<u32, DetNetEliminationFilter>,
    pub replication_factor: usize,
    pub default_window_size: usize,
}

impl DetNetPrefEngine {
    pub fn new(replication_factor: usize, default_window_size: usize) -> Self {
        DetNetPrefEngine {
            next_tx_seq: 1,
            filters: std::collections::HashMap::new(),
            replication_factor: replication_factor.max(1),
            default_window_size,
        }
    }

    /// Replicates a packet into `replication_factor` redundant copies with matching sequence numbers.
    pub fn replicate(&mut self, flow_id: u32, payload: &[u8]) -> Vec<DetNetPacket> {
        let seq = self.next_tx_seq;
        self.next_tx_seq = (self.next_tx_seq + 1) & 0xFFFF;

        let mut copies = Vec::with_capacity(self.replication_factor);
        for _ in 0..self.replication_factor {
            copies.push(DetNetPacket::new(flow_id, seq, payload.to_vec()));
        }
        copies
    }

    /// Processes an incoming DetNet packet through the flow's elimination filter:
    /// Returns `Some(packet)` if it is the first copy to arrive, or `None` if it is a duplicate.
    pub fn eliminate(&mut self, packet: DetNetPacket) -> Option<DetNetPacket> {
        let filter = self
            .filters
            .entry(packet.flow_id)
            .or_insert_with(|| DetNetEliminationFilter::new(self.default_window_size));

        if filter.process_sequence(packet.control_word.sequence_number) {
            Some(packet)
        } else {
            None
        }
    }

    /// Retrieves statistics for a specific flow.
    pub fn get_flow_stats(&self, flow_id: u32) -> Option<&DetNetStats> {
        self.filters.get(&flow_id).map(|f| &f.stats)
    }
}
