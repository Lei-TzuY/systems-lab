//! Point-to-Point Protocol over Ethernet (PPPoE - RFC 2516).
//!
//! Carrier broadband encapsulation covering PPPoE Discovery Stage (EtherType 0x8863)
//! and PPPoE Session Stage (EtherType 0x8864).

use std::fmt;

pub const ETHERTYPE_PPPOE_DISCOVERY: u16 = 0x8863;
pub const ETHERTYPE_PPPOE_SESSION: u16 = 0x8864;
pub const PPPOE_HEADER_LEN: usize = 6;

// PPPoE Discovery Codes
pub const PPPOE_CODE_SESSION_DATA: u8 = 0x00;
pub const PPPOE_CODE_PADO: u8 = 0x07;
pub const PPPOE_CODE_PADI: u8 = 0x09;
pub const PPPOE_CODE_PADR: u8 = 0x19;
pub const PPPOE_CODE_PADS: u8 = 0x65;
pub const PPPOE_CODE_PADT: u8 = 0xa7;

// PPP Protocols
pub const PPP_PROTO_IPV4: u16 = 0x0021;
pub const PPP_PROTO_IPV6: u16 = 0x0057;
pub const PPP_PROTO_LCP: u16 = 0xc021;
pub const PPP_PROTO_IPCP: u16 = 0x8021;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PppoePacket {
    pub version: u8,
    pub pppoe_type: u8,
    pub code: u8,
    pub session_id: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PppoeError {
    PacketTooShort(usize),
    InvalidVersionType(u8),
    PayloadLengthMismatch(usize, usize),
}

impl fmt::Display for PppoeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PppoeError::PacketTooShort(l) => {
                write!(f, "PPPoE packet too short ({} bytes, min 6)", l)
            }
            PppoeError::InvalidVersionType(v) => write!(
                f,
                "Invalid PPPoE version/type byte: 0x{:02x} (expected 0x11)",
                v
            ),
            PppoeError::PayloadLengthMismatch(h, a) => write!(
                f,
                "PPPoE length mismatch: header specifies {}, received {}",
                h, a
            ),
        }
    }
}

impl std::error::Error for PppoeError {}

impl PppoePacket {
    pub fn parse(data: &[u8]) -> Result<Self, PppoeError> {
        if data.len() < PPPOE_HEADER_LEN {
            return Err(PppoeError::PacketTooShort(data.len()));
        }

        let ver_type = data[0];
        if ver_type != 0x11 {
            return Err(PppoeError::InvalidVersionType(ver_type));
        }

        let code = data[1];
        let session_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]) as usize;

        if data.len() < PPPOE_HEADER_LEN + length {
            return Err(PppoeError::PayloadLengthMismatch(
                length,
                data.len() - PPPOE_HEADER_LEN,
            ));
        }

        let payload = data[PPPOE_HEADER_LEN..PPPOE_HEADER_LEN + length].to_vec();

        Ok(PppoePacket {
            version: 1,
            pppoe_type: 1,
            code,
            session_id,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let total_len = PPPOE_HEADER_LEN + self.payload.len();
        let mut buf = vec![0u8; total_len];

        buf[0] = 0x11; // Version 1, Type 1
        buf[1] = self.code;
        buf[2..4].copy_from_slice(&self.session_id.to_be_bytes());
        buf[4..6].copy_from_slice(&(self.payload.len() as u16).to_be_bytes());
        buf[6..].copy_from_slice(&self.payload);

        buf
    }

    pub fn build_padi() -> Self {
        // Tag Service-Name (Type 0x0101, Length 0)
        let payload = vec![0x01, 0x01, 0x00, 0x00];
        PppoePacket {
            version: 1,
            pppoe_type: 1,
            code: PPPOE_CODE_PADI,
            session_id: 0,
            payload,
        }
    }

    pub fn build_session_ipv4(session_id: u16, ipv4_packet: &[u8]) -> Self {
        let mut payload = Vec::with_capacity(2 + ipv4_packet.len());
        payload.extend_from_slice(&PPP_PROTO_IPV4.to_be_bytes());
        payload.extend_from_slice(ipv4_packet);

        PppoePacket {
            version: 1,
            pppoe_type: 1,
            code: PPPOE_CODE_SESSION_DATA,
            session_id,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pppoe_padi_discovery_packet() {
        let padi = PppoePacket::build_padi();
        let raw = padi.serialize();

        assert_eq!(raw.len(), PPPOE_HEADER_LEN + 4);
        let parsed = PppoePacket::parse(&raw).unwrap();
        assert_eq!(parsed.code, PPPOE_CODE_PADI);
        assert_eq!(parsed.session_id, 0);
    }

    #[test]
    fn test_pppoe_session_ipv4_encapsulation() {
        let ip_payload = b"GET / HTTP/1.1\r\n\r\n";
        let session_pkt = PppoePacket::build_session_ipv4(0x0042, ip_payload);
        let raw = session_pkt.serialize();

        let parsed = PppoePacket::parse(&raw).unwrap();
        assert_eq!(parsed.code, PPPOE_CODE_SESSION_DATA);
        assert_eq!(parsed.session_id, 0x0042);
        assert_eq!(
            u16::from_be_bytes([parsed.payload[0], parsed.payload[1]]),
            PPP_PROTO_IPV4
        );
        assert_eq!(&parsed.payload[2..], ip_payload);
    }
}
