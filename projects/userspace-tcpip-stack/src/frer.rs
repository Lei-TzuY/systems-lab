//! IEEE 802.1CB Frame Replication and Elimination for Reliability (FRER / TSN - R-TAG 0xF1C1).
//!
//! Provides zero-loss, hitless packet replication over dual independent paths
//! and sequence-based de-duplication elimination filtering.

use crate::ethernet::MacAddress;
use std::collections::HashSet;

pub const ETHERTYPE_RTAG: u16 = 0xF1C1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RTagHeader {
    pub reserved: u16,        // 16 bits: Reserved (0x0000)
    pub sequence_number: u16, // 16 bits: Monotonic sequence number
    pub inner_ethertype: u16, // 16 bits: Encapsulated EtherType (e.g. 0x0800 IPv4)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RTagFrame {
    pub dst_mac: MacAddress,
    pub src_mac: MacAddress,
    pub rtag: RTagHeader,
    pub payload: Vec<u8>,
}

impl RTagFrame {
    pub fn new(
        dst_mac: MacAddress,
        src_mac: MacAddress,
        sequence_number: u16,
        inner_ethertype: u16,
        payload: Vec<u8>,
    ) -> Self {
        RTagFrame {
            dst_mac,
            src_mac,
            rtag: RTagHeader {
                reserved: 0,
                sequence_number,
                inner_ethertype,
            },
            payload,
        }
    }

    /// Serializes an IEEE 802.1CB R-TAG frame (12B MAC + 2B EtherType 0xF1C1 + 6B R-TAG + Payload)
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12 + 2 + 6 + self.payload.len());
        buf.extend_from_slice(&self.dst_mac.0);
        buf.extend_from_slice(&self.src_mac.0);
        buf.extend_from_slice(&ETHERTYPE_RTAG.to_be_bytes());
        buf.extend_from_slice(&self.rtag.reserved.to_be_bytes());
        buf.extend_from_slice(&self.rtag.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.rtag.inner_ethertype.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parses an IEEE 802.1CB R-TAG frame
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 20 {
            return None;
        }

        let mut dst = [0u8; 6];
        dst.copy_from_slice(&data[0..6]);
        let mut src = [0u8; 6];
        src.copy_from_slice(&data[6..12]);

        let etype = u16::from_be_bytes([data[12], data[13]]);
        if etype != ETHERTYPE_RTAG {
            return None;
        }

        let reserved = u16::from_be_bytes([data[14], data[15]]);
        let sequence_number = u16::from_be_bytes([data[16], data[17]]);
        let inner_ethertype = u16::from_be_bytes([data[18], data[19]]);
        let payload = data[20..].to_vec();

        Some(RTagFrame {
            dst_mac: MacAddress(dst),
            src_mac: MacAddress(src),
            rtag: RTagHeader {
                reserved,
                sequence_number,
                inner_ethertype,
            },
            payload,
        })
    }
}

/// FRER Engine managing dual-path replication and receiver sequence de-duplication elimination
#[derive(Debug, Clone, Default)]
pub struct FrerEngine {
    pub tx_seq: u16,
    pub received_sequences: HashSet<u16>,
    pub packets_forwarded: u32,
    pub packets_eliminated_duplicates: u32,
}

impl FrerEngine {
    pub fn new() -> Self {
        FrerEngine {
            tx_seq: 1,
            received_sequences: HashSet::new(),
            packets_forwarded: 0,
            packets_eliminated_duplicates: 0,
        }
    }

    /// Replicates a packet into dual tagged frames for transmission across disjoint paths
    pub fn replicate(
        &mut self,
        dst: MacAddress,
        src: MacAddress,
        inner_etype: u16,
        payload: &[u8],
    ) -> (RTagFrame, RTagFrame) {
        let seq = self.tx_seq;
        self.tx_seq = self.tx_seq.wrapping_add(1);

        let frame_path_a = RTagFrame::new(dst, src, seq, inner_etype, payload.to_vec());
        let frame_path_b = RTagFrame::new(dst, src, seq, inner_etype, payload.to_vec());

        (frame_path_a, frame_path_b)
    }

    /// Processes an incoming R-TAG frame: forwards the first copy, eliminates duplicate copies
    pub fn process_ingress_frame(&mut self, frame: &RTagFrame) -> Option<Vec<u8>> {
        let seq = frame.rtag.sequence_number;
        if self.received_sequences.insert(seq) {
            self.packets_forwarded += 1;
            Some(frame.payload.clone()) // First copy accepted
        } else {
            self.packets_eliminated_duplicates += 1;
            None // Duplicate eliminated
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frer_replication_and_elimination() {
        let mut engine = FrerEngine::new();
        let dst = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        let src = MacAddress([0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB]);

        let (path_a, path_b) =
            engine.replicate(dst, src, 0x0800, b"TSN Time-Critical Mission Control");
        assert_eq!(path_a.rtag.sequence_number, 1);
        assert_eq!(path_b.rtag.sequence_number, 1);

        // First copy arrives via Path A -> Accepted
        let forward_a = engine.process_ingress_frame(&path_a);
        assert!(forward_a.is_some());
        assert_eq!(engine.packets_forwarded, 1);
        assert_eq!(engine.packets_eliminated_duplicates, 0);

        // Second copy arrives via Path B with identical sequence -> Eliminated
        let forward_b = engine.process_ingress_frame(&path_b);
        assert!(forward_b.is_none());
        assert_eq!(engine.packets_forwarded, 1);
        assert_eq!(engine.packets_eliminated_duplicates, 1);
    }
}
