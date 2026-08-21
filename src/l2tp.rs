//! Layer 2 Tunneling Protocol Version 3 (L2TPv3 - RFC 3931).
//!
//! Point-to-Point Layer 2 Ethernet Pseudowire encapsulation over IP Protocol 115.

use std::fmt;

pub const IP_PROTO_L2TPV3: u8 = 115;
pub const L2TPV3_UDP_PORT: u16 = 1700;
pub const L2TPV3_MIN_HEADER_LEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L2tpv3Packet {
    pub session_id: u32,
    pub cookie: Option<u64>,
    pub inner_frame: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L2tpError {
    PacketTooShort(usize),
    InvalidSessionId,
}

impl fmt::Display for L2tpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            L2tpError::PacketTooShort(l) => write!(f, "L2TPv3 packet too short ({} bytes, min 4)", l),
            L2tpError::InvalidSessionId => write!(f, "Invalid L2TPv3 session ID 0 (control connection reserved)"),
        }
    }
}

impl std::error::Error for L2tpError {}

impl L2tpv3Packet {
    pub fn parse(data: &[u8], has_cookie: bool) -> Result<Self, L2tpError> {
        let min_len = if has_cookie { 12 } else { 4 };
        if data.len() < min_len {
            return Err(L2tpError::PacketTooShort(data.len()));
        }

        let session_id = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if session_id == 0 {
            return Err(L2tpError::InvalidSessionId);
        }

        let (cookie, offset) = if has_cookie {
            let c = u64::from_be_bytes([
                data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
            ]);
            (Some(c), 12)
        } else {
            (None, 4)
        };

        let inner_frame = data[offset..].to_vec();

        Ok(L2tpv3Packet {
            session_id,
            cookie,
            inner_frame,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.session_id.to_be_bytes());
        if let Some(cookie) = self.cookie {
            buf.extend_from_slice(&cookie.to_be_bytes());
        }
        buf.extend_from_slice(&self.inner_frame);
        buf
    }

    pub fn encapsulate(session_id: u32, inner_frame: &[u8], cookie: Option<u64>) -> Vec<u8> {
        let pkt = L2tpv3Packet {
            session_id,
            cookie,
            inner_frame: inner_frame.to_vec(),
        };
        pkt.serialize()
    }

    pub fn decapsulate(data: &[u8], has_cookie: bool) -> Result<(u32, Vec<u8>), L2tpError> {
        let pkt = Self::parse(data, has_cookie)?;
        Ok((pkt.session_id, pkt.inner_frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2tpv3_encapsulate_and_decapsulate() {
        let inner_eth = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x08, 0x00, 0x45, 0x00];
        let encap = L2tpv3Packet::encapsulate(1001, &inner_eth, None);

        assert_eq!(encap.len(), 4 + inner_eth.len());

        let (sid, recovered) = L2tpv3Packet::decapsulate(&encap, false).unwrap();
        assert_eq!(sid, 1001);
        assert_eq!(recovered, inner_eth);
    }

    #[test]
    fn test_l2tpv3_with_64bit_cookie() {
        let inner_eth = b"Ethernet over L2TPv3 Pseudowire";
        let cookie_val = 0xCAFEBABE_DEADBEEF;
        let encap = L2tpv3Packet::encapsulate(2002, inner_eth, Some(cookie_val));

        assert_eq!(encap.len(), 12 + inner_eth.len());

        let parsed = L2tpv3Packet::parse(&encap, true).unwrap();
        assert_eq!(parsed.session_id, 2002);
        assert_eq!(parsed.cookie, Some(cookie_val));
        assert_eq!(parsed.inner_frame, inner_eth);
    }
}
