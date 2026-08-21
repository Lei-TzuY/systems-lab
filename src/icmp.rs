//! Layer 3: Internet Control Message Protocol (ICMP - RFC 792).
//!
//! Handles ICMP Echo Request (Type 8) and Echo Reply (Type 0).

use crate::checksum::{compute_checksum, verify_checksum};
use std::fmt;

pub const ICMP_TYPE_ECHO_REPLY: u8 = 0;
pub const ICMP_TYPE_DEST_UNREACHABLE: u8 = 3;
pub const ICMP_TYPE_ECHO_REQUEST: u8 = 8;
pub const ICMP_TYPE_TIME_EXCEEDED: u8 = 11;

pub const ICMP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpType {
    EchoReply,
    EchoRequest,
    DestinationUnreachable,
    TimeExceeded,
    Other(u8),
}

impl IcmpType {
    pub fn from_u8(val: u8) -> Self {
        match val {
            ICMP_TYPE_ECHO_REPLY => IcmpType::EchoReply,
            ICMP_TYPE_ECHO_REQUEST => IcmpType::EchoRequest,
            ICMP_TYPE_DEST_UNREACHABLE => IcmpType::DestinationUnreachable,
            ICMP_TYPE_TIME_EXCEEDED => IcmpType::TimeExceeded,
            other => IcmpType::Other(other),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            IcmpType::EchoReply => ICMP_TYPE_ECHO_REPLY,
            IcmpType::EchoRequest => ICMP_TYPE_ECHO_REQUEST,
            IcmpType::DestinationUnreachable => ICMP_TYPE_DEST_UNREACHABLE,
            IcmpType::TimeExceeded => ICMP_TYPE_TIME_EXCEEDED,
            IcmpType::Other(val) => *val,
        }
    }
}

impl fmt::Display for IcmpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IcmpType::EchoReply => write!(f, "Echo Reply (0)"),
            IcmpType::EchoRequest => write!(f, "Echo Request (8)"),
            IcmpType::DestinationUnreachable => write!(f, "Destination Unreachable (3)"),
            IcmpType::TimeExceeded => write!(f, "Time Exceeded (11)"),
            IcmpType::Other(val) => write!(f, "ICMP Type ({})", val),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpPacket<'a> {
    pub icmp_type: IcmpType,
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence_number: u16,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcmpError {
    PacketTooShort(usize),
    InvalidChecksum { computed: u16, found: u16 },
}

impl fmt::Display for IcmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IcmpError::PacketTooShort(len) => write!(f, "ICMP packet too short ({} bytes, min 8)", len),
            IcmpError::InvalidChecksum { computed, found } => {
                write!(f, "ICMP checksum mismatch: computed 0x{:04x}, found 0x{:04x}", computed, found)
            }
        }
    }
}

impl std::error::Error for IcmpError {}

impl<'a> IcmpPacket<'a> {
    pub fn parse(data: &'a [u8], check_checksum: bool) -> Result<Self, IcmpError> {
        if data.len() < ICMP_HEADER_LEN {
            return Err(IcmpError::PacketTooShort(data.len()));
        }

        if check_checksum && !verify_checksum(data) {
            let actual = compute_checksum(data);
            let found = u16::from_be_bytes([data[2], data[3]]);
            return Err(IcmpError::InvalidChecksum {
                computed: actual,
                found,
            });
        }

        let icmp_type = IcmpType::from_u8(data[0]);
        let code = data[1];
        let checksum = u16::from_be_bytes([data[2], data[3]]);
        let identifier = u16::from_be_bytes([data[4], data[5]]);
        let sequence_number = u16::from_be_bytes([data[6], data[7]]);
        let payload = &data[ICMP_HEADER_LEN..];

        Ok(IcmpPacket {
            icmp_type,
            code,
            checksum,
            identifier,
            sequence_number,
            payload,
        })
    }

    pub fn serialize(
        icmp_type: u8,
        code: u8,
        identifier: u16,
        sequence_number: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(ICMP_HEADER_LEN + payload.len());
        buf.push(icmp_type);
        buf.push(code);
        buf.extend_from_slice(&[0x00, 0x00]); // Checksum placeholder
        buf.extend_from_slice(&identifier.to_be_bytes());
        buf.extend_from_slice(&sequence_number.to_be_bytes());
        buf.extend_from_slice(payload);

        let csum = compute_checksum(&buf);
        buf[2..4].copy_from_slice(&csum.to_be_bytes());
        buf
    }

    pub fn build_echo_reply(req: &IcmpPacket<'_>) -> Vec<u8> {
        Self::serialize(
            ICMP_TYPE_ECHO_REPLY,
            0,
            req.identifier,
            req.sequence_number,
            req.payload,
        )
    }

    pub fn build_echo_request(identifier: u16, sequence_number: u16, payload: &[u8]) -> Vec<u8> {
        Self::serialize(
            ICMP_TYPE_ECHO_REQUEST,
            0,
            identifier,
            sequence_number,
            payload,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icmp_echo_reply_creation() {
        let ping_payload = b"abcdefghijklmnopqrstuvwabcdefghi";
        let req_raw = IcmpPacket::build_echo_request(0x1234, 1, ping_payload);
        assert_eq!(req_raw.len(), 8 + ping_payload.len());

        let req = IcmpPacket::parse(&req_raw, true).unwrap();
        assert_eq!(req.icmp_type, IcmpType::EchoRequest);
        assert_eq!(req.identifier, 0x1234);
        assert_eq!(req.sequence_number, 1);
        assert_eq!(req.payload, ping_payload);

        let reply_raw = IcmpPacket::build_echo_reply(&req);
        let reply = IcmpPacket::parse(&reply_raw, true).unwrap();
        assert_eq!(reply.icmp_type, IcmpType::EchoReply);
        assert_eq!(reply.identifier, 0x1234);
        assert_eq!(reply.sequence_number, 1);
        assert_eq!(reply.payload, ping_payload);
    }
}
