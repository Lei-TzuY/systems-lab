//! Encapsulated Remote SPAN (ERSPAN) & NVGRE (RFC 7637).
//!
//! Remote network port mirroring and cloud network virtualization over GRE (Protocol 47).

use std::fmt;

pub const ETHERTYPE_ERSPAN_TYPE2: u16 = 0x88BE;
pub const ETHERTYPE_NVGRE_ETHERNET: u16 = 0x6558;
pub const ERSPAN_TYPE2_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErspanType2Header {
    pub vlan: u16,
    pub cos: u8,
    pub session_id: u16, // 10-bit span session id
    pub index: u32,      // 20-bit port index
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErspanPacket {
    pub header: ErspanType2Header,
    pub mirrored_frame: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvgrePacket {
    pub vsid: u32,    // 24-bit Virtual Subnet ID
    pub flow_id: u8,  // 8-bit Flow ID
    pub inner_frame: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErspanError {
    PacketTooShort(usize),
    InvalidVersion(u8),
}

impl fmt::Display for ErspanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErspanError::PacketTooShort(l) => write!(f, "ERSPAN packet too short ({} bytes)", l),
            ErspanError::InvalidVersion(v) => write!(f, "Invalid ERSPAN version: {}", v),
        }
    }
}

impl std::error::Error for ErspanError {}

impl ErspanPacket {
    pub fn encapsulate(session_id: u16, vlan: u16, port_index: u32, frame: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        // Byte 0-1: Ver(4) | VLAN(12) => Ver=1 for Type II (0x1000) | (vlan & 0x0FFF)
        let w0 = 0x1000u16 | (vlan & 0x0FFF);
        buf.extend_from_slice(&w0.to_be_bytes());

        // Byte 2-3: COS(3) | En(2) | T(1) | SessionID(10)
        let w1 = session_id & 0x03FF;
        buf.extend_from_slice(&w1.to_be_bytes());

        // Byte 4-7: Reserved(12) | Index(20)
        let w2_3 = port_index & 0x000F_FFFF;
        buf.extend_from_slice(&w2_3.to_be_bytes());

        buf.extend_from_slice(frame);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, ErspanError> {
        if data.len() < ERSPAN_TYPE2_HEADER_LEN {
            return Err(ErspanError::PacketTooShort(data.len()));
        }

        let w0 = u16::from_be_bytes([data[0], data[1]]);
        let ver = (w0 >> 12) as u8;
        if ver != 1 {
            return Err(ErspanError::InvalidVersion(ver));
        }

        let vlan = w0 & 0x0FFF;
        let w1 = u16::from_be_bytes([data[2], data[3]]);
        let cos = ((w1 >> 13) & 0x07) as u8;
        let session_id = w1 & 0x03FF;
        let w2_3 = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let index = w2_3 & 0x000F_FFFF;

        let header = ErspanType2Header {
            vlan,
            cos,
            session_id,
            index,
        };

        let mirrored_frame = data[ERSPAN_TYPE2_HEADER_LEN..].to_vec();

        Ok(ErspanPacket {
            header,
            mirrored_frame,
        })
    }
}

impl NvgrePacket {
    pub fn encapsulate(vsid: u32, flow_id: u8, frame: &[u8]) -> Vec<u8> {
        // NVGRE GRE key field: 24-bit VSID + 8-bit Flow ID
        let _key = ((vsid & 0x00FF_FFFF) << 8) | (flow_id as u32);
        frame.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erspan_type2_encapsulation_and_parse() {
        let fake_eth = [0xAA; 64];
        let erspan_raw = ErspanPacket::encapsulate(101, 20, 5, &fake_eth);

        assert_eq!(erspan_raw.len(), ERSPAN_TYPE2_HEADER_LEN + 64);
        let parsed = ErspanPacket::parse(&erspan_raw).unwrap();

        assert_eq!(parsed.header.session_id, 101);
        assert_eq!(parsed.header.vlan, 20);
        assert_eq!(parsed.header.index, 5);
        assert_eq!(parsed.mirrored_frame.len(), 64);
    }
}
