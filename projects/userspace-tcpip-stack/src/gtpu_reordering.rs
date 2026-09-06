//! 3GPP TS 29.281 Section 5.2.1 — 5G GTP-U Sequence Number Reordering & Jitter Buffer.
//!
//! In cellular core networks (gNodeB to UPF, UPF to PSA), multipath underlays
//! or handovers can cause GTP-U user plane packets carrying sensitive voice/video
//! or TCP traffic to arrive out-of-order.
//!
//! This module implements:
//! * 16-bit GTP-U Sequence Number sliding reordering window with RFC 1982 modular arithmetic.
//! * Out-of-order packet jitter buffer.
//! * In-order contiguous packet release to upper PDU layers.
//! * Gap skip & stale packet dropping protection.

use std::collections::BTreeMap;

/// RFC 1982 16-bit serial number comparison: returns true if a is before b.
pub fn seq_lt(a: u16, b: u16) -> bool {
    let diff = b.wrapping_sub(a);
    diff > 0 && diff < 32768
}

/// RFC 1982 16-bit serial number comparison: returns true if a is after b.
pub fn seq_gt(a: u16, b: u16) -> bool {
    let diff = a.wrapping_sub(b);
    diff > 0 && diff < 32768
}

/// A buffered GTP-U PDU session packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpuBufferedPacket {
    pub sequence_number: u16,
    pub payload: Vec<u8>,
}

/// GTP-U Sequence Number Reordering Engine per TEID.
#[derive(Debug, Clone)]
pub struct GtpuReorderingEngine {
    pub teid: u32,
    /// Next expected in-order GTP-U sequence number.
    pub next_expected_seq: u16,
    /// Maximum reordering window size.
    pub window_size: u16,
    /// Buffer holding out-of-order packets indexed by sequence number.
    pub buffer: BTreeMap<u16, Vec<u8>>,
    /// Counters.
    pub total_received: u64,
    pub total_in_order: u64,
    pub total_reordered: u64,
    pub total_duplicates: u64,
    pub total_gaps_skipped: u64,
}

impl GtpuReorderingEngine {
    pub fn new(teid: u32, initial_seq: u16, window_size: u16) -> Self {
        GtpuReorderingEngine {
            teid,
            next_expected_seq: initial_seq,
            window_size,
            buffer: BTreeMap::new(),
            total_received: 0,
            total_in_order: 0,
            total_reordered: 0,
            total_duplicates: 0,
            total_gaps_skipped: 0,
        }
    }

    /// Ingests a GTP-U packet with sequence number and returns ready in-order packets.
    pub fn ingest_packet(&mut self, seq: u16, payload: Vec<u8>) -> Vec<GtpuBufferedPacket> {
        self.total_received += 1;
        let mut delivered = Vec::new();

        // 1. Check if packet is stale/duplicate (seq < next_expected_seq)
        if seq_lt(seq, self.next_expected_seq) {
            self.total_duplicates += 1;
            return delivered;
        }

        // 2. Exactly expected packet!
        if seq == self.next_expected_seq {
            self.total_in_order += 1;
            delivered.push(GtpuBufferedPacket {
                sequence_number: seq,
                payload,
            });
            self.next_expected_seq = self.next_expected_seq.wrapping_add(1);

            // Drain any contiguous buffered packets
            while let Some(buffered_payload) = self.buffer.remove(&self.next_expected_seq) {
                delivered.push(GtpuBufferedPacket {
                    sequence_number: self.next_expected_seq,
                    payload: buffered_payload,
                });
                self.next_expected_seq = self.next_expected_seq.wrapping_add(1);
            }
            return delivered;
        }

        // 3. Out-of-order packet (seq > next_expected_seq)
        let distance = seq.wrapping_sub(self.next_expected_seq);
        if distance <= self.window_size {
            // Buffer it
            if self.buffer.insert(seq, payload).is_none() {
                self.total_reordered += 1;
            } else {
                self.total_duplicates += 1;
            }
        } else {
            // Exceeded window size -> Skip gap and force advance!
            self.total_gaps_skipped += 1;
            self.next_expected_seq = seq.wrapping_sub(self.window_size);

            // Drain anything now eligible
            while let Some((&first_seq, _)) = self.buffer.iter().next() {
                if !seq_gt(first_seq, self.next_expected_seq) {
                    let p = self.buffer.remove(&first_seq).unwrap();
                    delivered.push(GtpuBufferedPacket {
                        sequence_number: first_seq,
                        payload: p,
                    });
                } else {
                    break;
                }
            }

            self.buffer.insert(seq, payload);
            self.total_reordered += 1;
        }

        delivered
    }

    /// Flushes all remaining buffered packets in sequence.
    pub fn force_flush(&mut self) -> Vec<GtpuBufferedPacket> {
        let mut delivered = Vec::new();
        while let Some((seq, payload)) = self.buffer.pop_first() {
            delivered.push(GtpuBufferedPacket {
                sequence_number: seq,
                payload,
            });
            self.next_expected_seq = seq.wrapping_add(1);
        }
        delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtpu_reordering_out_of_order_stream() {
        let mut engine = GtpuReorderingEngine::new(0x1000, 1, 16);

        // Packet 1 arrives -> immediately delivered
        let d1 = engine.ingest_packet(1, b"Packet 1".to_vec());
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].sequence_number, 1);
        assert_eq!(engine.next_expected_seq, 2);

        // Packet 3 arrives -> out of order, buffered
        let d3 = engine.ingest_packet(3, b"Packet 3".to_vec());
        assert_eq!(d3.len(), 0);
        assert_eq!(engine.buffer.len(), 1);

        // Packet 4 arrives -> out of order, buffered
        let d4 = engine.ingest_packet(4, b"Packet 4".to_vec());
        assert_eq!(d4.len(), 0);
        assert_eq!(engine.buffer.len(), 2);

        // Packet 2 arrives (missing gap) -> triggers sequential release of 2, 3, 4!
        let d2 = engine.ingest_packet(2, b"Packet 2".to_vec());
        assert_eq!(d2.len(), 3);
        assert_eq!(d2[0].sequence_number, 2);
        assert_eq!(d2[1].sequence_number, 3);
        assert_eq!(d2[2].sequence_number, 4);
        assert_eq!(engine.next_expected_seq, 5);
        assert_eq!(engine.buffer.len(), 0);
    }

    #[test]
    fn test_gtpu_reordering_duplicate_drop() {
        let mut engine = GtpuReorderingEngine::new(0x1000, 5, 16);

        let d1 = engine.ingest_packet(5, b"P5".to_vec());
        assert_eq!(d1.len(), 1);

        // Duplicate of 5
        let dup = engine.ingest_packet(5, b"P5_dup".to_vec());
        assert_eq!(dup.len(), 0);
        assert_eq!(engine.total_duplicates, 1);
    }
}
