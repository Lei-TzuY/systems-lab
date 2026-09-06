//! IEEE 802.1AS Generalized Precision Time Protocol (gPTP / Time-Sensitive Networking).
//!
//! Sub-nanosecond deterministic peer-to-peer clock synchronization for AVB & TSN over EtherType 0x88F7.

use crate::ethernet::MacAddress;
use std::fmt;

pub const ETHERTYPE_GPTP: u16 = 0x88F7;
pub const GPTP_MULTICAST_MAC: MacAddress = MacAddress([0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E]);

// gPTP Message Types
pub const GPTP_MSG_SYNC: u8 = 0x00;
pub const GPTP_MSG_PDELAY_REQ: u8 = 0x02;
pub const GPTP_MSG_PDELAY_RESP: u8 = 0x03;
pub const GPTP_MSG_FOLLOW_UP: u8 = 0x08;
pub const GPTP_MSG_PDELAY_RESP_FOLLOW_UP: u8 = 0x0A;
pub const GPTP_MSG_ANNOUNCE: u8 = 0x0B;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GptpTimestamp {
    pub seconds: u64, // 48-bit
    pub nanoseconds: u32,
}

impl GptpTimestamp {
    pub fn new(seconds: u64, nanoseconds: u32) -> Self {
        GptpTimestamp {
            seconds: seconds & 0x0000_FFFF_FFFF_FFFF,
            nanoseconds,
        }
    }

    pub fn to_nanos(&self) -> u64 {
        self.seconds * 1_000_000_000 + self.nanoseconds as u64
    }

    pub fn serialize(&self) -> [u8; 10] {
        let mut buf = [0u8; 10];
        let sec_bytes = self.seconds.to_be_bytes();
        buf[0..6].copy_from_slice(&sec_bytes[2..8]); // 48-bit seconds
        buf[6..10].copy_from_slice(&self.nanoseconds.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 10 {
            return None;
        }
        let seconds =
            u64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]);
        let nanoseconds = u32::from_be_bytes([data[6], data[7], data[8], data[9]]);
        Some(GptpTimestamp {
            seconds,
            nanoseconds,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptpHeader {
    pub transport_specific: u8, // 1 = 802.1AS
    pub message_type: u8,
    pub version_ptp: u8, // 2
    pub message_length: u16,
    pub domain_number: u8, // 0
    pub flags: u16,
    pub correction_field_ns_scaled: i64, // 64-bit nanoseconds * 2^16
    pub clock_identity: [u8; 8],
    pub source_port_id: u16,
    pub sequence_id: u16,
    pub log_message_interval: i8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GptpPacket {
    pub header: GptpHeader,
    pub origin_timestamp: Option<GptpTimestamp>,
    pub requesting_port_identity: Option<([u8; 8], u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GptpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidLength,
}

impl fmt::Display for GptpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GptpError::PacketTooShort(l) => write!(f, "gPTP packet too short ({} bytes)", l),
            GptpError::InvalidVersion(v) => write!(f, "Unsupported gPTP version: {}", v),
            GptpError::InvalidLength => write!(f, "Invalid gPTP length"),
        }
    }
}

impl std::error::Error for GptpError {}

impl GptpPacket {
    pub fn build_pdelay_req(
        clock_id: [u8; 8],
        port_id: u16,
        seq_id: u16,
        t1: GptpTimestamp,
    ) -> Self {
        let header = GptpHeader {
            transport_specific: 1, // IEEE 802.1AS
            message_type: GPTP_MSG_PDELAY_REQ,
            version_ptp: 2,
            message_length: 54,
            domain_number: 0,
            flags: 0x0000,
            correction_field_ns_scaled: 0,
            clock_identity: clock_id,
            source_port_id: port_id,
            sequence_id: seq_id,
            log_message_interval: 0,
        };

        GptpPacket {
            header,
            origin_timestamp: Some(t1),
            requesting_port_identity: None,
        }
    }

    pub fn build_pdelay_resp(
        clock_id: [u8; 8],
        port_id: u16,
        req_clock_id: [u8; 8],
        req_port_id: u16,
        seq_id: u16,
        t2: GptpTimestamp,
    ) -> Self {
        let header = GptpHeader {
            transport_specific: 1,
            message_type: GPTP_MSG_PDELAY_RESP,
            version_ptp: 2,
            message_length: 54,
            domain_number: 0,
            flags: 0x0200, // Two-Step flag set
            correction_field_ns_scaled: 0,
            clock_identity: clock_id,
            source_port_id: port_id,
            sequence_id: seq_id,
            log_message_interval: 0x7F,
        };

        GptpPacket {
            header,
            origin_timestamp: Some(t2),
            requesting_port_identity: Some((req_clock_id, req_port_id)),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let b0 = (self.header.transport_specific << 4) | (self.header.message_type & 0x0F);
        buf.push(b0);
        buf.push(self.header.version_ptp & 0x0F);
        buf.extend_from_slice(&self.header.message_length.to_be_bytes());
        buf.push(self.header.domain_number);
        buf.push(1); // Minor version (802.1AS-2020)
        buf.extend_from_slice(&self.header.flags.to_be_bytes());
        buf.extend_from_slice(&self.header.correction_field_ns_scaled.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // Reserved
        buf.extend_from_slice(&self.header.clock_identity);
        buf.extend_from_slice(&self.header.source_port_id.to_be_bytes());
        buf.extend_from_slice(&self.header.sequence_id.to_be_bytes());
        buf.push(0); // Control field
        buf.push(self.header.log_message_interval as u8);

        // Body (34..54)
        if let Some(ts) = self.origin_timestamp {
            buf.extend_from_slice(&ts.serialize());
        } else {
            buf.extend_from_slice(&[0u8; 10]);
        }

        if let Some((req_cid, req_pid)) = self.requesting_port_identity {
            buf.extend_from_slice(&req_cid);
            buf.extend_from_slice(&req_pid.to_be_bytes());
        } else {
            buf.extend_from_slice(&[0u8; 10]);
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, GptpError> {
        if data.len() < 34 {
            return Err(GptpError::PacketTooShort(data.len()));
        }

        let transport_specific = data[0] >> 4;
        let message_type = data[0] & 0x0F;
        let version_ptp = data[1] & 0x0F;
        if version_ptp != 2 {
            return Err(GptpError::InvalidVersion(version_ptp));
        }

        let message_length = u16::from_be_bytes([data[2], data[3]]);
        let domain_number = data[4];
        let flags = u16::from_be_bytes([data[6], data[7]]);
        let correction_field_ns_scaled = i64::from_be_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);

        let mut clock_identity = [0u8; 8];
        clock_identity.copy_from_slice(&data[20..28]);
        let source_port_id = u16::from_be_bytes([data[28], data[29]]);
        let sequence_id = u16::from_be_bytes([data[30], data[31]]);
        let log_message_interval = data[33] as i8;

        let origin_timestamp = if data.len() >= 44 {
            GptpTimestamp::parse(&data[34..44])
        } else {
            None
        };

        let requesting_port_identity = if data.len() >= 54 {
            let mut req_cid = [0u8; 8];
            req_cid.copy_from_slice(&data[44..52]);
            let req_pid = u16::from_be_bytes([data[52], data[53]]);
            Some((req_cid, req_pid))
        } else {
            None
        };

        Ok(GptpPacket {
            header: GptpHeader {
                transport_specific,
                message_type,
                version_ptp,
                message_length,
                domain_number,
                flags,
                correction_field_ns_scaled,
                clock_identity,
                source_port_id,
                sequence_id,
                log_message_interval,
            },
            origin_timestamp,
            requesting_port_identity,
        })
    }
}

/// Calculate IEEE 802.1AS Peer Propagation Delay (nanoseconds)
///
/// T_pdelay = ((t4 - t1) - (t3 - t2)) / 2
pub fn calculate_gptp_peer_delay(
    t1: GptpTimestamp,
    t2: GptpTimestamp,
    t3: GptpTimestamp,
    t4: GptpTimestamp,
) -> i64 {
    let d_41 = t4.to_nanos() as i64 - t1.to_nanos() as i64;
    let d_32 = t3.to_nanos() as i64 - t2.to_nanos() as i64;
    (d_41 - d_32) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gptp_pdelay_mechanism() {
        let clock_a = [0x00, 0x80, 0xC2, 0xFF, 0xFE, 0x00, 0x00, 0x01];
        let clock_b = [0x00, 0x80, 0xC2, 0xFF, 0xFE, 0x00, 0x00, 0x02];

        let t1 = GptpTimestamp::new(1700000000, 100_000_000);
        let t2 = GptpTimestamp::new(1700000000, 100_000_050); // +50 ns wire delay
        let t3 = GptpTimestamp::new(1700000000, 100_010_000); // 10 us responder turn-around
        let t4 = GptpTimestamp::new(1700000000, 100_010_050); // +50 ns return delay

        let pdelay_req = GptpPacket::build_pdelay_req(clock_a, 1, 101, t1);
        let raw_req = pdelay_req.serialize();
        assert_eq!(raw_req.len(), 54);

        let parsed_req = GptpPacket::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.header.transport_specific, 1);
        assert_eq!(parsed_req.header.message_type, GPTP_MSG_PDELAY_REQ);
        assert_eq!(parsed_req.header.clock_identity, clock_a);

        let pdelay_resp = GptpPacket::build_pdelay_resp(clock_b, 2, clock_a, 1, 101, t2);
        let raw_resp = pdelay_resp.serialize();
        let parsed_resp = GptpPacket::parse(&raw_resp).unwrap();
        assert_eq!(parsed_resp.header.message_type, GPTP_MSG_PDELAY_RESP);

        let pdelay = calculate_gptp_peer_delay(t1, t2, t3, t4);
        assert_eq!(pdelay, 50); // 50 nanoseconds one-way link propagation delay
        assert_eq!(ETHERTYPE_GPTP, 0x88F7);
    }
}
