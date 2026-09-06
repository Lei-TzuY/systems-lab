//! Next-Generation Modern Transport: QUIC Binary Packet Framing (RFC 9000).
//!
//! Handles RFC 9000 Variable-Length Integer (VINT) 2-bit prefix encoding,
//! QUIC Long Header packets (Initial, Handshake, 0-RTT, Retry), and Short Header (1-RTT) multiplexed framing.

use std::fmt;

pub const QUIC_VERSION_1: u32 = 0x0000_0001;

// Long Header Packet Types
pub const QUIC_PKT_INITIAL: u8 = 0x0;
pub const QUIC_PKT_0RTT: u8 = 0x1;
pub const QUIC_PKT_HANDSHAKE: u8 = 0x2;
pub const QUIC_PKT_RETRY: u8 = 0x3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicPacket {
    Long {
        packet_type: u8,
        version: u32,
        dcid: Vec<u8>,
        scid: Vec<u8>,
        payload: Vec<u8>,
    },
    Short {
        spin_bit: bool,
        dcid: Vec<u8>,
        packet_number: u32,
        payload: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuicError {
    PacketTooShort(usize),
    InvalidVint,
    BufferOverflow,
}

impl fmt::Display for QuicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuicError::PacketTooShort(len) => write!(f, "QUIC packet too short ({} bytes)", len),
            QuicError::InvalidVint => {
                write!(f, "Invalid QUIC Variable-Length Integer (VINT) encoding")
            }
            QuicError::BufferOverflow => write!(f, "QUIC payload length exceeds available buffer"),
        }
    }
}

impl std::error::Error for QuicError {}

// --- Variable-Length Integer (VINT - RFC 9000 Section 16) ---

pub fn encode_vint(val: u64) -> Vec<u8> {
    if val <= 63 {
        vec![val as u8]
    } else if val <= 16383 {
        let v = 0x4000u16 | (val as u16);
        v.to_be_bytes().to_vec()
    } else if val <= 1073741823 {
        let v = 0x8000_0000u32 | (val as u32);
        v.to_be_bytes().to_vec()
    } else {
        let v = 0xC000_0000_0000_0000u64 | val;
        v.to_be_bytes().to_vec()
    }
}

pub fn decode_vint(data: &[u8]) -> Result<(u64, usize), QuicError> {
    if data.is_empty() {
        return Err(QuicError::PacketTooShort(0));
    }

    let prefix = data[0] >> 6;
    match prefix {
        0 => Ok((data[0] as u64, 1)),
        1 => {
            if data.len() < 2 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let raw = u16::from_be_bytes([data[0], data[1]]) & 0x3FFF;
            Ok((raw as u64, 2))
        }
        2 => {
            if data.len() < 4 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let raw = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) & 0x3FFF_FFFF;
            Ok((raw as u64, 4))
        }
        3 => {
            if data.len() < 8 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let raw = u64::from_be_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) & 0x3FFF_FFFF_FFFF_FFFF;
            Ok((raw, 8))
        }
        _ => Err(QuicError::InvalidVint),
    }
}

impl QuicPacket {
    pub fn parse(data: &[u8]) -> Result<Self, QuicError> {
        if data.is_empty() {
            return Err(QuicError::PacketTooShort(0));
        }

        let first_byte = data[0];
        let is_long_header = (first_byte & 0x80) != 0;

        if is_long_header {
            if data.len() < 7 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let packet_type = (first_byte >> 4) & 0x03;
            let version = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);

            let dcid_len = data[5] as usize;
            if data.len() < 6 + dcid_len + 1 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let dcid = data[6..6 + dcid_len].to_vec();

            let scid_offset = 6 + dcid_len;
            let scid_len = data[scid_offset] as usize;
            if data.len() < scid_offset + 1 + scid_len {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let scid = data[scid_offset + 1..scid_offset + 1 + scid_len].to_vec();

            let payload_offset = scid_offset + 1 + scid_len;
            let payload = data[payload_offset..].to_vec();

            Ok(QuicPacket::Long {
                packet_type,
                version,
                dcid,
                scid,
                payload,
            })
        } else {
            // Short Header (1-RTT)
            if data.len() < 9 {
                return Err(QuicError::PacketTooShort(data.len()));
            }
            let spin_bit = (first_byte & 0x20) != 0;
            // Standard 8-byte DCID for short headers
            let dcid = data[1..9].to_vec();
            let packet_number = if data.len() >= 13 {
                u32::from_be_bytes([data[9], data[10], data[11], data[12]])
            } else {
                0
            };
            let payload = if data.len() > 13 {
                data[13..].to_vec()
            } else {
                Vec::new()
            };

            Ok(QuicPacket::Short {
                spin_bit,
                dcid,
                packet_number,
                payload,
            })
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            QuicPacket::Long {
                packet_type,
                version,
                dcid,
                scid,
                payload,
            } => {
                let first_byte = 0x80 | 0x40 | ((packet_type & 0x03) << 4);
                buf.push(first_byte);
                buf.extend_from_slice(&version.to_be_bytes());

                buf.push(dcid.len() as u8);
                buf.extend_from_slice(dcid);

                buf.push(scid.len() as u8);
                buf.extend_from_slice(scid);

                buf.extend_from_slice(payload);
            }
            QuicPacket::Short {
                spin_bit,
                dcid,
                packet_number,
                payload,
            } => {
                let mut first_byte = 0x40; // Fixed bit
                if *spin_bit {
                    first_byte |= 0x20;
                }
                buf.push(first_byte);
                buf.extend_from_slice(dcid);
                buf.extend_from_slice(&packet_number.to_be_bytes());
                buf.extend_from_slice(payload);
            }
        }

        buf
    }

    pub fn build_initial(dcid: Vec<u8>, scid: Vec<u8>, payload: &[u8]) -> Self {
        QuicPacket::Long {
            packet_type: QUIC_PKT_INITIAL,
            version: QUIC_VERSION_1,
            dcid,
            scid,
            payload: payload.to_vec(),
        }
    }

    pub fn build_1rtt(dcid: Vec<u8>, packet_number: u32, payload: &[u8]) -> Self {
        QuicPacket::Short {
            spin_bit: false,
            dcid,
            packet_number,
            payload: payload.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_vint_encoding_and_decoding() {
        let test_cases: &[(u64, usize)] = &[(25, 1), (15293, 2), (494878333, 4), (15128880994, 8)];

        for &(val, expected_len) in test_cases {
            let encoded = encode_vint(val);
            assert_eq!(encoded.len(), expected_len);
            let (decoded, consumed) = decode_vint(&encoded).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(consumed, expected_len);
        }
    }

    #[test]
    fn test_quic_long_and_short_packets() {
        // 1. Long Header Initial
        let dcid = vec![0x11, 0x22, 0x33, 0x44];
        let scid = vec![0xaa, 0xbb, 0xcc, 0xdd];
        let initial = QuicPacket::build_initial(dcid.clone(), scid.clone(), b"CRYPTO Frame Data");

        let raw_initial = initial.serialize();
        let parsed_initial = QuicPacket::parse(&raw_initial).unwrap();
        if let QuicPacket::Long {
            packet_type,
            dcid: d,
            scid: s,
            payload,
            ..
        } = parsed_initial
        {
            assert_eq!(packet_type, QUIC_PKT_INITIAL);
            assert_eq!(d, dcid);
            assert_eq!(s, scid);
            assert_eq!(payload, b"CRYPTO Frame Data");
        } else {
            panic!("Expected Long Header");
        }

        // 2. Short Header 1-RTT
        let short = QuicPacket::build_1rtt(vec![0x01; 8], 105, b"HTTP/3 Stream Data");
        let raw_short = short.serialize();
        let parsed_short = QuicPacket::parse(&raw_short).unwrap();
        if let QuicPacket::Short {
            packet_number,
            payload,
            ..
        } = parsed_short
        {
            assert_eq!(packet_number, 105);
            assert_eq!(payload, b"HTTP/3 Stream Data");
        } else {
            panic!("Expected Short Header");
        }
    }
}
