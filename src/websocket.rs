//! Application Layer: WebSocket Protocol (RFC 6455) Binary Framing.
//!
//! Handles frame headers (FIN, Opcode, Masking), variable payload length (7-bit, 16-bit, 64-bit),
//! 4-byte XOR mask calculation, and Control Frames (Ping, Pong, Close).

use std::fmt;

// WebSocket Opcodes
pub const WS_OPCODE_CONTINUATION: u8 = 0x0;
pub const WS_OPCODE_TEXT: u8 = 0x1;
pub const WS_OPCODE_BINARY: u8 = 0x2;
pub const WS_OPCODE_CLOSE: u8 = 0x8;
pub const WS_OPCODE_PING: u8 = 0x9;
pub const WS_OPCODE_PONG: u8 = 0xA;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSocketFrame {
    pub fin: bool,
    pub opcode: u8,
    pub masked: bool,
    pub mask_key: Option<[u8; 4]>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSocketError {
    FrameTooShort(usize),
    InvalidLength { specified: usize, available: usize },
}

impl fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSocketError::FrameTooShort(len) => write!(f, "WebSocket frame too short ({} bytes, min 2)", len),
            WebSocketError::InvalidLength { specified, available } => {
                write!(f, "WebSocket payload length {} exceeds available buffer {}", specified, available)
            }
        }
    }
}

impl std::error::Error for WebSocketError {}

impl WebSocketFrame {
    pub fn parse(data: &[u8]) -> Result<Self, WebSocketError> {
        if data.len() < 2 {
            return Err(WebSocketError::FrameTooShort(data.len()));
        }

        let fin = (data[0] & 0x80) != 0;
        let opcode = data[0] & 0x0F;

        let masked = (data[1] & 0x80) != 0;
        let initial_len = (data[1] & 0x7F) as usize;

        let mut offset = 2;
        let payload_len = if initial_len == 126 {
            if data.len() < offset + 2 {
                return Err(WebSocketError::FrameTooShort(data.len()));
            }
            let len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            len
        } else if initial_len == 127 {
            if data.len() < offset + 8 {
                return Err(WebSocketError::FrameTooShort(data.len()));
            }
            let len = u64::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            offset += 8;
            len
        } else {
            initial_len
        };

        let mask_key = if masked {
            if data.len() < offset + 4 {
                return Err(WebSocketError::FrameTooShort(data.len()));
            }
            let key = [data[offset], data[offset + 1], data[offset + 2], data[offset + 3]];
            offset += 4;
            Some(key)
        } else {
            None
        };

        if data.len() < offset + payload_len {
            return Err(WebSocketError::InvalidLength {
                specified: payload_len,
                available: data.len() - offset,
            });
        }

        let payload = data[offset..offset + payload_len].to_vec();

        Ok(WebSocketFrame {
            fin,
            opcode,
            masked,
            mask_key,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        let mut b0 = self.opcode & 0x0F;
        if self.fin {
            b0 |= 0x80;
        }
        buf.push(b0);

        let len = self.payload.len();
        let mask_bit = if self.masked { 0x80 } else { 0 };

        if len <= 125 {
            buf.push(mask_bit | (len as u8));
        } else if len <= 65535 {
            buf.push(mask_bit | 126);
            buf.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            buf.push(mask_bit | 127);
            buf.extend_from_slice(&(len as u64).to_be_bytes());
        }

        if let Some(key) = self.mask_key {
            buf.extend_from_slice(&key);
            // Mask the payload
            for (i, &b) in self.payload.iter().enumerate() {
                buf.push(b ^ key[i % 4]);
            }
        } else {
            buf.extend_from_slice(&self.payload);
        }

        buf
    }

    /// Returns unmasked payload data (applies XOR mask if frame is masked)
    pub fn unmasked_payload(&self) -> Vec<u8> {
        if let Some(key) = self.mask_key {
            self.payload
                .iter()
                .enumerate()
                .map(|(i, &b)| b ^ key[i % 4])
                .collect()
        } else {
            self.payload.clone()
        }
    }

    /// Builds a WebSocket Text frame
    pub fn build_text(text: &str, masked: bool, mask_key: Option<[u8; 4]>) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: WS_OPCODE_TEXT,
            masked,
            mask_key,
            payload: text.as_bytes().to_vec(),
        }
    }

    /// Builds a WebSocket Binary frame
    pub fn build_binary(data: &[u8], masked: bool, mask_key: Option<[u8; 4]>) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: WS_OPCODE_BINARY,
            masked,
            mask_key,
            payload: data.to_vec(),
        }
    }

    /// Builds a WebSocket Ping frame
    pub fn build_ping(payload: &[u8]) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: WS_OPCODE_PING,
            masked: false,
            mask_key: None,
            payload: payload.to_vec(),
        }
    }

    /// Builds a WebSocket Pong frame
    pub fn build_pong(payload: &[u8]) -> Self {
        WebSocketFrame {
            fin: true,
            opcode: WS_OPCODE_PONG,
            masked: false,
            mask_key: None,
            payload: payload.to_vec(),
        }
    }

    /// Builds a WebSocket Connection Close frame
    pub fn build_close() -> Self {
        WebSocketFrame {
            fin: true,
            opcode: WS_OPCODE_CLOSE,
            masked: false,
            mask_key: None,
            payload: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_text_frame_unmasked() {
        let msg = "Hello WebSocket!";
        let frame = WebSocketFrame::build_text(msg, false, None);
        let raw = frame.serialize();

        let parsed = WebSocketFrame::parse(&raw).unwrap();
        assert!(parsed.fin);
        assert_eq!(parsed.opcode, WS_OPCODE_TEXT);
        assert!(!parsed.masked);
        assert_eq!(String::from_utf8(parsed.unmasked_payload()).unwrap(), msg);
    }

    #[test]
    fn test_websocket_masked_frame_and_unmasking() {
        let msg = "Secure Masked Payload";
        let mask_key = [0x37, 0xfa, 0x21, 0x3d];
        let frame = WebSocketFrame::build_text(msg, true, Some(mask_key));
        let raw = frame.serialize();

        let parsed = WebSocketFrame::parse(&raw).unwrap();
        assert!(parsed.masked);
        assert_eq!(parsed.mask_key, Some(mask_key));
        assert_eq!(String::from_utf8(parsed.unmasked_payload()).unwrap(), msg);
    }

    #[test]
    fn test_websocket_ping_pong_frames() {
        let ping = WebSocketFrame::build_ping(b"Heartbeat");
        let parsed_ping = WebSocketFrame::parse(&ping.serialize()).unwrap();
        assert_eq!(parsed_ping.opcode, WS_OPCODE_PING);
        assert_eq!(parsed_ping.payload, b"Heartbeat");

        let pong = WebSocketFrame::build_pong(b"Heartbeat");
        let parsed_pong = WebSocketFrame::parse(&pong.serialize()).unwrap();
        assert_eq!(parsed_pong.opcode, WS_OPCODE_PONG);
        assert_eq!(parsed_pong.payload, b"Heartbeat");
    }
}
