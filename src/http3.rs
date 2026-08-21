//! Hypertext Transfer Protocol Version 3 (HTTP/3 - RFC 9114) & QPACK (RFC 9204).
//!
//! Binary multiplexed application protocol framing designed to run natively over QUIC streams.

use crate::quic::{decode_vint, encode_vint, QuicError};
use std::fmt;

pub const HTTP3_FRAME_DATA: u64 = 0x00;
pub const HTTP3_FRAME_HEADERS: u64 = 0x01;
pub const HTTP3_FRAME_CANCEL_PUSH: u64 = 0x03;
pub const HTTP3_FRAME_SETTINGS: u64 = 0x04;
pub const HTTP3_FRAME_PUSH_PROMISE: u64 = 0x05;
pub const HTTP3_FRAME_GOAWAY: u64 = 0x07;

pub const HTTP3_SETTING_QPACK_MAX_TABLE_CAPACITY: u64 = 0x01;
pub const HTTP3_SETTING_MAX_FIELD_SECTION_SIZE: u64 = 0x06;
pub const HTTP3_SETTING_QPACK_BLOCKED_STREAMS: u64 = 0x07;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http3Frame {
    pub frame_type: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Http3Error {
    FrameTooShort,
    InvalidVint(QuicError),
    PayloadLengthMismatch,
}

impl fmt::Display for Http3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Http3Error::FrameTooShort => write!(f, "HTTP/3 frame too short"),
            Http3Error::InvalidVint(e) => write!(f, "HTTP/3 VINT decode failed: {}", e),
            Http3Error::PayloadLengthMismatch => write!(f, "HTTP/3 payload length mismatch"),
        }
    }
}

impl std::error::Error for Http3Error {}

impl Http3Frame {
    pub fn parse(data: &[u8]) -> Result<(Self, usize), Http3Error> {
        let mut offset = 0;

        let (frame_type, type_len) = decode_vint(&data[offset..]).map_err(Http3Error::InvalidVint)?;
        offset += type_len;

        let (length, len_len) = decode_vint(&data[offset..]).map_err(Http3Error::InvalidVint)?;
        offset += len_len;

        let payload_len = length as usize;
        if data.len() < offset + payload_len {
            return Err(Http3Error::PayloadLengthMismatch);
        }

        let payload = data[offset..offset + payload_len].to_vec();
        offset += payload_len;

        Ok((Http3Frame { frame_type, payload }, offset))
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_vint(self.frame_type));
        buf.extend_from_slice(&encode_vint(self.payload.len() as u64));
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn build_data(data: &[u8]) -> Self {
        Http3Frame {
            frame_type: HTTP3_FRAME_DATA,
            payload: data.to_vec(),
        }
    }

    pub fn build_headers(headers: &[(&str, &str)]) -> Self {
        // QPACK Representation (Simulated Literal with Name Reference)
        let mut payload = Vec::new();
        for &(k, v) in headers {
            let k_bytes = k.as_bytes();
            let v_bytes = v.as_bytes();
            payload.push(0x00); // Literal Header Field without Name Reference
            payload.extend_from_slice(&encode_vint(k_bytes.len() as u64));
            payload.extend_from_slice(k_bytes);
            payload.extend_from_slice(&encode_vint(v_bytes.len() as u64));
            payload.extend_from_slice(v_bytes);
        }

        Http3Frame {
            frame_type: HTTP3_FRAME_HEADERS,
            payload,
        }
    }

    pub fn build_settings(settings: &[(u64, u64)]) -> Self {
        let mut payload = Vec::new();
        for &(id, val) in settings {
            payload.extend_from_slice(&encode_vint(id));
            payload.extend_from_slice(&encode_vint(val));
        }

        Http3Frame {
            frame_type: HTTP3_FRAME_SETTINGS,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http3_settings_frame() {
        let settings = vec![
            (HTTP3_SETTING_QPACK_MAX_TABLE_CAPACITY, 4096),
            (HTTP3_SETTING_MAX_FIELD_SECTION_SIZE, 65536),
        ];

        let frame = Http3Frame::build_settings(&settings);
        let raw = frame.serialize();

        let (parsed, len) = Http3Frame::parse(&raw).unwrap();
        assert_eq!(len, raw.len());
        assert_eq!(parsed.frame_type, HTTP3_FRAME_SETTINGS);
    }

    #[test]
    fn test_http3_headers_and_data_frames() {
        let headers = vec![
            (":method", "GET"),
            (":path", "/api/v1/status"),
            (":scheme", "https"),
        ];

        let hdr_frame = Http3Frame::build_headers(&headers);
        let hdr_raw = hdr_frame.serialize();
        let (parsed_hdr, _) = Http3Frame::parse(&hdr_raw).unwrap();
        assert_eq!(parsed_hdr.frame_type, HTTP3_FRAME_HEADERS);

        let data_frame = Http3Frame::build_data(b"{\"status\": \"ok\", \"protocol\": \"HTTP/3\"}");
        let data_raw = data_frame.serialize();
        let (parsed_data, _) = Http3Frame::parse(&data_raw).unwrap();
        assert_eq!(parsed_data.frame_type, HTTP3_FRAME_DATA);
        assert_eq!(parsed_data.payload, b"{\"status\": \"ok\", \"protocol\": \"HTTP/3\"}");
    }
}
