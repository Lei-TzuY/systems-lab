//! Stream Control Transmission Protocol (SCTP - RFC 4960).
//!
//! Multi-streaming and multi-homed reliable transport protocol operating over IP Protocol 132.

use std::fmt;

pub const IP_PROTO_SCTP: u8 = 132;
pub const SCTP_COMMON_HEADER_LEN: usize = 12;

// SCTP Chunk Types
pub const SCTP_CHUNK_DATA: u8 = 0;
pub const SCTP_CHUNK_INIT: u8 = 1;
pub const SCTP_CHUNK_INIT_ACK: u8 = 2;
pub const SCTP_CHUNK_SACK: u8 = 3;
pub const SCTP_CHUNK_HEARTBEAT: u8 = 4;
pub const SCTP_CHUNK_HEARTBEAT_ACK: u8 = 5;
pub const SCTP_CHUNK_ABORT: u8 = 6;
pub const SCTP_CHUNK_SHUTDOWN: u8 = 7;
pub const SCTP_CHUNK_SHUTDOWN_ACK: u8 = 8;
pub const SCTP_CHUNK_COOKIE_ECHO: u8 = 10;
pub const SCTP_CHUNK_COOKIE_ACK: u8 = 11;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SctpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub verification_tag: u32,
    pub checksum: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SctpChunk {
    pub chunk_type: u8,
    pub flags: u8,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SctpPacket {
    pub header: SctpHeader,
    pub chunks: Vec<SctpChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SctpError {
    PacketTooShort(usize),
    InvalidChunkLength,
}

impl fmt::Display for SctpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SctpError::PacketTooShort(l) => write!(f, "SCTP packet too short ({} bytes)", l),
            SctpError::InvalidChunkLength => write!(f, "Invalid SCTP chunk length field"),
        }
    }
}

impl std::error::Error for SctpError {}

impl SctpPacket {
    pub fn build_init(
        src_p: u16,
        dst_p: u16,
        init_tag: u32,
        a_rwnd: u32,
        num_out: u16,
        num_in: u16,
        isn: u32,
    ) -> Self {
        let mut val = Vec::new();
        val.extend_from_slice(&init_tag.to_be_bytes());
        val.extend_from_slice(&a_rwnd.to_be_bytes());
        val.extend_from_slice(&num_out.to_be_bytes());
        val.extend_from_slice(&num_in.to_be_bytes());
        val.extend_from_slice(&isn.to_be_bytes());

        let chunk = SctpChunk {
            chunk_type: SCTP_CHUNK_INIT,
            flags: 0,
            value: val,
        };

        SctpPacket {
            header: SctpHeader {
                src_port: src_p,
                dst_port: dst_p,
                verification_tag: 0, // Verification tag is 0 in INIT
                checksum: 0,
            },
            chunks: vec![chunk],
        }
    }

    pub fn build_data(
        src_p: u16,
        dst_p: u16,
        v_tag: u32,
        tsn: u32,
        stream_id: u16,
        stream_seq: u16,
        payload_proto: u32,
        user_data: &[u8],
    ) -> Self {
        let mut val = Vec::new();
        val.extend_from_slice(&tsn.to_be_bytes());
        val.extend_from_slice(&stream_id.to_be_bytes());
        val.extend_from_slice(&stream_seq.to_be_bytes());
        val.extend_from_slice(&payload_proto.to_be_bytes());
        val.extend_from_slice(user_data);

        let chunk = SctpChunk {
            chunk_type: SCTP_CHUNK_DATA,
            flags: 0x03, // Complete segment (Unordered=0, Begin=1, End=1)
            value: val,
        };

        SctpPacket {
            header: SctpHeader {
                src_port: src_p,
                dst_port: dst_p,
                verification_tag: v_tag,
                checksum: 0,
            },
            chunks: vec![chunk],
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.header.src_port.to_be_bytes());
        buf.extend_from_slice(&self.header.dst_port.to_be_bytes());
        buf.extend_from_slice(&self.header.verification_tag.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // Checksum placeholder

        for ch in &self.chunks {
            buf.push(ch.chunk_type);
            buf.push(ch.flags);
            let len = (4 + ch.value.len()) as u16;
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(&ch.value);

            // 4-byte alignment padding
            let pad_len = (4 - (ch.value.len() % 4)) % 4;
            for _ in 0..pad_len {
                buf.push(0x00);
            }
        }

        // Calculate Adler-32 Checksum for demonstration
        let chk = calculate_adler32(&buf);
        buf[8..12].copy_from_slice(&chk.to_be_bytes());

        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, SctpError> {
        if data.len() < SCTP_COMMON_HEADER_LEN {
            return Err(SctpError::PacketTooShort(data.len()));
        }

        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let verification_tag = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let checksum = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let header = SctpHeader {
            src_port,
            dst_port,
            verification_tag,
            checksum,
        };

        let mut chunks = Vec::new();
        let mut offset = SCTP_COMMON_HEADER_LEN;

        while offset + 4 <= data.len() {
            let chunk_type = data[offset];
            let flags = data[offset + 1];
            let chunk_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            if chunk_len < 4 || offset + chunk_len > data.len() {
                return Err(SctpError::InvalidChunkLength);
            }

            let value = data[offset + 4..offset + chunk_len].to_vec();
            chunks.push(SctpChunk {
                chunk_type,
                flags,
                value,
            });

            // Advance with 4-byte padding
            let padded_len = (chunk_len + 3) & !3;
            offset += padded_len;
        }

        Ok(SctpPacket { header, chunks })
    }
}

fn calculate_adler32(data: &[u8]) -> u32 {
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sctp_init_and_data_chunk_roundtrip() {
        let init = SctpPacket::build_init(5000, 2905, 0x12345678, 65535, 10, 10, 1000);
        let raw_init = init.serialize();

        assert!(raw_init.len() >= SCTP_COMMON_HEADER_LEN);
        let parsed_init = SctpPacket::parse(&raw_init).unwrap();
        assert_eq!(parsed_init.header.src_port, 5000);
        assert_eq!(parsed_init.chunks.len(), 1);
        assert_eq!(parsed_init.chunks[0].chunk_type, SCTP_CHUNK_INIT);

        let data = SctpPacket::build_data(5000, 2905, 0x12345678, 1, 0, 0, 0, b"Hello SCTP Stream");
        let raw_data = data.serialize();
        let parsed_data = SctpPacket::parse(&raw_data).unwrap();
        assert_eq!(parsed_data.chunks[0].chunk_type, SCTP_CHUNK_DATA);
        assert!(parsed_data.chunks[0].value.ends_with(b"Hello SCTP Stream"));
    }
}
