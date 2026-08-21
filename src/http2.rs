//! Application Layer: HTTP/2 Binary Framing Protocol (RFC 7540).
//!
//! Handles the standard 9-byte HTTP/2 frame header, stream multiplexing (Stream IDs),
//! and frame types (DATA, HEADERS, SETTINGS, PING, GOAWAY, WINDOW_UPDATE).

use std::fmt;

pub const HTTP2_FRAME_HEADER_LEN: usize = 9;

// HTTP/2 Frame Types
pub const HTTP2_FRAME_DATA: u8 = 0x0;
pub const HTTP2_FRAME_HEADERS: u8 = 0x1;
pub const HTTP2_FRAME_PRIORITY: u8 = 0x2;
pub const HTTP2_FRAME_RST_STREAM: u8 = 0x3;
pub const HTTP2_FRAME_SETTINGS: u8 = 0x4;
pub const HTTP2_FRAME_PUSH_PROMISE: u8 = 0x5;
pub const HTTP2_FRAME_PING: u8 = 0x6;
pub const HTTP2_FRAME_GOAWAY: u8 = 0x7;
pub const HTTP2_FRAME_WINDOW_UPDATE: u8 = 0x8;
pub const HTTP2_FRAME_CONTINUATION: u8 = 0x9;

// HTTP/2 Frame Flags
pub const HTTP2_FLAG_END_STREAM: u8 = 0x1;
pub const HTTP2_FLAG_END_HEADERS: u8 = 0x4;
pub const HTTP2_FLAG_ACK: u8 = 0x1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Frame {
    pub length: u32,
    pub frame_type: u8,
    pub flags: u8,
    pub stream_id: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Http2Error {
    FrameTooShort(usize),
    LengthMismatch { header_len: usize, available: usize },
}

impl fmt::Display for Http2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Http2Error::FrameTooShort(len) => write!(f, "HTTP/2 frame too short ({} bytes, min 9)", len),
            Http2Error::LengthMismatch { header_len, available } => {
                write!(f, "HTTP/2 frame length {} exceeds available buffer {}", header_len, available)
            }
        }
    }
}

impl std::error::Error for Http2Error {}

impl Http2Frame {
    pub fn new(frame_type: u8, flags: u8, stream_id: u32, payload: Vec<u8>) -> Self {
        Http2Frame {
            length: payload.len() as u32,
            frame_type,
            flags,
            stream_id: stream_id & 0x7FFF_FFFF,
            payload,
        }
    }

    pub fn parse(data: &[u8]) -> Result<Self, Http2Error> {
        if data.len() < HTTP2_FRAME_HEADER_LEN {
            return Err(Http2Error::FrameTooShort(data.len()));
        }

        let length = (((data[0] as u32) << 16) | ((data[1] as u32) << 8) | (data[2] as u32)) as usize;
        let frame_type = data[3];
        let flags = data[4];
        let stream_id = u32::from_be_bytes([data[5], data[6], data[7], data[8]]) & 0x7FFF_FFFF;

        if data.len() < HTTP2_FRAME_HEADER_LEN + length {
            return Err(Http2Error::LengthMismatch {
                header_len: length,
                available: data.len() - HTTP2_FRAME_HEADER_LEN,
            });
        }

        let payload = data[HTTP2_FRAME_HEADER_LEN..HTTP2_FRAME_HEADER_LEN + length].to_vec();

        Ok(Http2Frame {
            length: length as u32,
            frame_type,
            flags,
            stream_id,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = HTTP2_FRAME_HEADER_LEN + self.payload.len();
        let mut buf = Vec::with_capacity(total_len);

        let len = self.payload.len() as u32;
        buf.push(((len >> 16) & 0xFF) as u8);
        buf.push(((len >> 8) & 0xFF) as u8);
        buf.push((len & 0xFF) as u8);
        buf.push(self.frame_type);
        buf.push(self.flags);
        buf.extend_from_slice(&(self.stream_id & 0x7FFF_FFFF).to_be_bytes());
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Builds an HTTP/2 SETTINGS frame (Stream ID 0)
    pub fn build_settings(ack: bool) -> Self {
        let flags = if ack { HTTP2_FLAG_ACK } else { 0 };
        Http2Frame::new(HTTP2_FRAME_SETTINGS, flags, 0, Vec::new())
    }

    /// Builds an HTTP/2 HEADERS frame
    pub fn build_headers(stream_id: u32, end_stream: bool, end_headers: bool, header_block: &[u8]) -> Self {
        let mut flags = 0;
        if end_stream { flags |= HTTP2_FLAG_END_STREAM; }
        if end_headers { flags |= HTTP2_FLAG_END_HEADERS; }
        Http2Frame::new(HTTP2_FRAME_HEADERS, flags, stream_id, header_block.to_vec())
    }

    /// Builds an HTTP/2 DATA frame
    pub fn build_data(stream_id: u32, end_stream: bool, data: &[u8]) -> Self {
        let flags = if end_stream { HTTP2_FLAG_END_STREAM } else { 0 };
        Http2Frame::new(HTTP2_FRAME_DATA, flags, stream_id, data.to_vec())
    }

    /// Builds an HTTP/2 PING frame (Stream ID 0, 8-byte payload)
    pub fn build_ping(ack: bool, opaque_data: [u8; 8]) -> Self {
        let flags = if ack { HTTP2_FLAG_ACK } else { 0 };
        Http2Frame::new(HTTP2_FRAME_PING, flags, 0, opaque_data.to_vec())
    }

    /// Builds an HTTP/2 GOAWAY frame (Stream ID 0)
    pub fn build_goaway(last_stream_id: u32, error_code: u32) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&(last_stream_id & 0x7FFF_FFFF).to_be_bytes());
        payload.extend_from_slice(&error_code.to_be_bytes());
        Http2Frame::new(HTTP2_FRAME_GOAWAY, 0, 0, payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http2_frame_serialize_and_parse() {
        let frame = Http2Frame::build_headers(1, true, true, b":method GET :path /index.html");
        let raw = frame.serialize();
        let parsed = Http2Frame::parse(&raw).unwrap();

        assert_eq!(parsed.length, frame.payload.len() as u32);
        assert_eq!(parsed.frame_type, HTTP2_FRAME_HEADERS);
        assert_eq!(parsed.flags, HTTP2_FLAG_END_STREAM | HTTP2_FLAG_END_HEADERS);
        assert_eq!(parsed.stream_id, 1);
        assert_eq!(parsed.payload, b":method GET :path /index.html");
    }

    #[test]
    fn test_http2_settings_and_ping_frames() {
        let settings = Http2Frame::build_settings(true);
        assert_eq!(settings.flags, HTTP2_FLAG_ACK);
        assert_eq!(settings.stream_id, 0);

        let ping = Http2Frame::build_ping(false, [1, 2, 3, 4, 5, 6, 7, 8]);
        let parsed_ping = Http2Frame::parse(&ping.serialize()).unwrap();
        assert_eq!(parsed_ping.frame_type, HTTP2_FRAME_PING);
        assert_eq!(parsed_ping.payload, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
