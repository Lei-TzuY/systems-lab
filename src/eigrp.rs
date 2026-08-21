//! Enhanced Interior Gateway Routing Protocol (EIGRP - RFC 7868).
//!
//! Advanced distance-vector routing protocol operating over IP Protocol 88 (Multicast 224.0.0.10).
//! Features the DUAL (Diffusing Update Algorithm) metric engine and loop-free backup path selection.

use crate::checksum::compute_checksum;
use crate::ipv4::Ipv4Address;
use std::collections::HashMap;
use std::fmt;

pub const IP_PROTO_EIGRP: u8 = 88;
pub const EIGRP_MULTICAST_IP: Ipv4Address = Ipv4Address([224, 0, 0, 10]);
pub const EIGRP_HEADER_LEN: usize = 20;

// EIGRP Opcodes
pub const EIGRP_OPCODE_UPDATE: u8 = 1;
pub const EIGRP_OPCODE_REQUEST: u8 = 2;
pub const EIGRP_OPCODE_QUERY: u8 = 3;
pub const EIGRP_OPCODE_REPLY: u8 = 4;
pub const EIGRP_OPCODE_HELLO: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EigrpHeader {
    pub version: u8,
    pub opcode: u8,
    pub checksum: u16,
    pub flags: u32,
    pub sequence: u32,
    pub ack: u32,
    pub vrid: u16,
    pub as_number: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EigrpMetric {
    pub bandwidth_kbps: u32,
    pub delay_tens_of_us: u32,
    pub reliability: u8,
    pub load: u8,
    pub hop_count: u8,
}

impl EigrpMetric {
    pub fn new(bandwidth_kbps: u32, delay_tens_of_us: u32) -> Self {
        EigrpMetric {
            bandwidth_kbps,
            delay_tens_of_us,
            reliability: 255,
            load: 1,
            hop_count: 1,
        }
    }

    /// Computes classic EIGRP composite metric: 256 * (10^7 / BW_min + Delay_sum)
    pub fn calculate_composite_metric(&self) -> u64 {
        let bw_term = 10_000_000u64 / (self.bandwidth_kbps.max(1) as u64);
        let delay_term = self.delay_tens_of_us as u64;
        256 * (bw_term + delay_term)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EigrpPacket {
    pub header: EigrpHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EigrpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidChecksum,
}

impl fmt::Display for EigrpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EigrpError::PacketTooShort(l) => write!(f, "EIGRP packet too short ({} bytes, min 20)", l),
            EigrpError::InvalidVersion(v) => write!(f, "Invalid EIGRP version: expected 2, found {}", v),
            EigrpError::InvalidChecksum => write!(f, "EIGRP checksum verification failed"),
        }
    }
}

impl std::error::Error for EigrpError {}

impl EigrpPacket {
    pub fn parse(data: &[u8], verify_csum: bool) -> Result<Self, EigrpError> {
        if data.len() < EIGRP_HEADER_LEN {
            return Err(EigrpError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if version != 2 {
            return Err(EigrpError::InvalidVersion(version));
        }

        if verify_csum && compute_checksum(data) != 0 {
            return Err(EigrpError::InvalidChecksum);
        }

        let opcode = data[1];
        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let flags = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let sequence = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let ack = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let vrid = u16::from_be_bytes([data[16], data[17]]);
        let as_number = u16::from_be_bytes([data[18], data[19]]);

        let payload = data[EIGRP_HEADER_LEN..].to_vec();

        Ok(EigrpPacket {
            header: EigrpHeader {
                version,
                opcode,
                checksum,
                flags,
                sequence,
                ack,
                vrid,
                as_number,
            },
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = EIGRP_HEADER_LEN + self.payload.len();
        let mut buf = vec![0u8; total_len];

        buf[0] = self.header.version;
        buf[1] = self.header.opcode;
        buf[2..4].copy_from_slice(&0u16.to_be_bytes()); // Checksum placeholder
        buf[4..8].copy_from_slice(&self.header.flags.to_be_bytes());
        buf[8..12].copy_from_slice(&self.header.sequence.to_be_bytes());
        buf[12..16].copy_from_slice(&self.header.ack.to_be_bytes());
        buf[16..18].copy_from_slice(&self.header.vrid.to_be_bytes());
        buf[18..20].copy_from_slice(&self.header.as_number.to_be_bytes());
        buf[20..].copy_from_slice(&self.payload);

        let csum = compute_checksum(&buf);
        buf[2] = (csum >> 8) as u8;
        buf[3] = (csum & 0xFF) as u8;

        buf
    }

    pub fn build_hello(as_number: u16) -> Self {
        // EIGRP Parameters TLV: K1=1, K2=0, K3=1, K4=0, K5=0, Hold Time=15s
        let params_tlv = vec![
            0x00, 0x01, // Type 1: Parameters
            0x00, 0x0C, // Length 12
            0x01, 0x00, 0x01, 0x00, 0x00, 0x00, // K-values
            0x00, 0x0F, // Hold Time 15s
        ];

        EigrpPacket {
            header: EigrpHeader {
                version: 2,
                opcode: EIGRP_OPCODE_HELLO,
                checksum: 0,
                flags: 0,
                sequence: 0,
                ack: 0,
                vrid: 0,
                as_number,
            },
            payload: params_tlv,
        }
    }
}

// --- DUAL Topology Table & Path Selection ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EigrpNeighborRoute {
    pub neighbor: Ipv4Address,
    pub reported_distance: u64, // RD / AD (Advertised Distance)
    pub total_metric: u64,      // Computed metric through this neighbor
}

/// DUAL Topology Table
pub struct EigrpTopologyTable {
    // Destination Subnet -> List of candidate neighbor paths
    pub routes: HashMap<Ipv4Address, Vec<EigrpNeighborRoute>>,
}

impl Default for EigrpTopologyTable {
    fn default() -> Self {
        Self::new()
    }
}

impl EigrpTopologyTable {
    pub fn new() -> Self {
        let mut table = EigrpTopologyTable {
            routes: HashMap::new(),
        };

        // Destination: 10.50.0.0/24
        // Path 1 (FastEthernet via R2): RD = 28160, Total Metric = 30720
        // Path 2 (Gigabit via R3): RD = 30720, Total Metric = 33280
        let dest = Ipv4Address::new(10, 50, 0, 0);
        let r2 = Ipv4Address::new(192, 168, 1, 2);
        let r3 = Ipv4Address::new(192, 168, 1, 3);

        table.add_candidate(dest, r2, 28160, 30720);
        table.add_candidate(dest, r3, 30000, 33280);
        table
    }

    pub fn add_candidate(&mut self, dest: Ipv4Address, neighbor: Ipv4Address, rd: u64, total: u64) {
        self.routes.entry(dest).or_default().push(EigrpNeighborRoute {
            neighbor,
            reported_distance: rd,
            total_metric: total,
        });
    }

    /// Evaluates DUAL Feasibility Condition: Successor (primary) and Feasible Successors (backup)
    pub fn compute_dual(&self, dest: Ipv4Address) -> Option<(EigrpNeighborRoute, Vec<EigrpNeighborRoute>, u64)> {
        let candidates = self.routes.get(&dest)?;
        if candidates.is_empty() {
            return None;
        }

        // 1. Find Successor: Lowest total metric
        let mut sorted = candidates.clone();
        sorted.sort_by_key(|c| c.total_metric);
        let successor = sorted[0].clone();
        let feasible_distance = successor.total_metric;

        // 2. Find Feasible Successor: RD < FD (Feasibility Condition)
        let mut feasible_successors = Vec::new();
        for c in &sorted[1..] {
            if c.reported_distance < feasible_distance {
                feasible_successors.push(c.clone());
            }
        }

        Some((successor, feasible_successors, feasible_distance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eigrp_packet_roundtrip() {
        let hello = EigrpPacket::build_hello(100);
        let raw = hello.serialize();
        assert!(raw.len() >= EIGRP_HEADER_LEN);

        let parsed = EigrpPacket::parse(&raw, true).unwrap();
        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.opcode, EIGRP_OPCODE_HELLO);
        assert_eq!(parsed.header.as_number, 100);
    }

    #[test]
    fn test_eigrp_dual_metric_and_feasibility_condition() {
        let metric = EigrpMetric::new(100_000, 100); // 100 Mbps, 100 tens of microseconds
        let comp = metric.calculate_composite_metric();
        assert_eq!(comp, 256 * (100 + 100)); // 51200

        let table = EigrpTopologyTable::new();
        let dest = Ipv4Address::new(10, 50, 0, 0);

        let (successor, feasible_successors, fd) = table.compute_dual(dest).unwrap();
        assert_eq!(successor.neighbor, Ipv4Address::new(192, 168, 1, 2));
        assert_eq!(fd, 30720);

        // R3 has RD=30000 < FD=30720 -> Qualified as Feasible Successor (Loop-Free Backup)!
        assert_eq!(feasible_successors.len(), 1);
        assert_eq!(feasible_successors[0].neighbor, Ipv4Address::new(192, 168, 1, 3));
    }
}
