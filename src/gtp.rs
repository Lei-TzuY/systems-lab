//! GPRS Tunnelling Protocol User Plane (GTP-U - 3GPP TS 29.281).
//!
//! Core datacenter and radio access network encapsulation for 3G, 4G LTE, and 5G cellular user data.

use crate::ipv4::Ipv4Address;
use std::collections::BTreeMap;
use std::fmt;

pub const GTP_U_UDP_PORT: u16 = 2152;
pub const GTP_U_HEADER_LEN: usize = 8;

// GTP-U Message Types
pub const GTP_MSG_ECHO_REQUEST: u8 = 1;
pub const GTP_MSG_ECHO_RESPONSE: u8 = 2;
pub const GTP_MSG_ERROR_INDICATION: u8 = 26;
pub const GTP_MSG_END_MARKER: u8 = 254;
pub const GTP_MSG_GPDU: u8 = 255; // User plane PDU (IP packet)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpHeader {
    pub version: u8,      // Typically 1
    pub protocol_type: bool, // 1 for GTP (0 for GTP')
    pub ext_flag: bool,   // Extension header present
    pub seq_flag: bool,   // Sequence number present
    pub npdu_flag: bool,  // N-PDU number present
    pub msg_type: u8,
    pub length: u16,      // Length of payload (plus optional 4B if flags set)
    pub teid: u32,        // Tunnel Endpoint Identifier
    pub seq_num: Option<u16>,
    pub npdu_num: Option<u8>,
    pub next_ext: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpPacket {
    pub header: GtpHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpTunnelSession {
    pub teid: u32,
    pub subscriber_ip: Ipv4Address,
    pub gnb_upf_ip: Ipv4Address,
}

#[derive(Debug, Clone, Default)]
pub struct GtpTunnelTable {
    pub sessions: BTreeMap<u32, GtpTunnelSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GtpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidLength,
}

impl fmt::Display for GtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GtpError::PacketTooShort(l) => write!(f, "GTP-U packet too short ({} bytes)", l),
            GtpError::InvalidVersion(v) => write!(f, "Invalid GTP version: {}", v),
            GtpError::InvalidLength => write!(f, "GTP-U payload length mismatch"),
        }
    }
}

impl std::error::Error for GtpError {}

impl GtpPacket {
    pub fn build_gpdu(teid: u32, payload: &[u8]) -> Self {
        GtpPacket {
            header: GtpHeader {
                version: 1,
                protocol_type: true,
                ext_flag: false,
                seq_flag: false,
                npdu_flag: false,
                msg_type: GTP_MSG_GPDU,
                length: payload.len() as u16,
                teid,
                seq_num: None,
                npdu_num: None,
                next_ext: None,
            },
            payload: payload.to_vec(),
        }
    }

    pub fn build_echo_request(teid: u32, seq: u16) -> Self {
        GtpPacket {
            header: GtpHeader {
                version: 1,
                protocol_type: true,
                ext_flag: false,
                seq_flag: true,
                npdu_flag: false,
                msg_type: GTP_MSG_ECHO_REQUEST,
                length: 4, // 4 bytes for optional seq + npdu + ext
                teid,
                seq_num: Some(seq),
                npdu_num: Some(0),
                next_ext: Some(0),
            },
            payload: Vec::new(),
        }
    }

    pub fn build_echo_response(teid: u32, seq: u16) -> Self {
        GtpPacket {
            header: GtpHeader {
                version: 1,
                protocol_type: true,
                ext_flag: false,
                seq_flag: true,
                npdu_flag: false,
                msg_type: GTP_MSG_ECHO_RESPONSE,
                length: 4,
                teid,
                seq_num: Some(seq),
                npdu_num: Some(0),
                next_ext: Some(0),
            },
            payload: Vec::new(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut flags = (self.header.version & 0x07) << 5;
        if self.header.protocol_type {
            flags |= 0x10;
        }
        if self.header.ext_flag {
            flags |= 0x04;
        }
        if self.header.seq_flag {
            flags |= 0x02;
        }
        if self.header.npdu_flag {
            flags |= 0x01;
        }

        buf.push(flags);
        buf.push(self.header.msg_type);
        buf.extend_from_slice(&self.header.length.to_be_bytes());
        buf.extend_from_slice(&self.header.teid.to_be_bytes());

        let has_optional = self.header.ext_flag || self.header.seq_flag || self.header.npdu_flag;
        if has_optional {
            let seq = self.header.seq_num.unwrap_or(0);
            buf.extend_from_slice(&seq.to_be_bytes());
            buf.push(self.header.npdu_num.unwrap_or(0));
            buf.push(self.header.next_ext.unwrap_or(0));
        }

        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, GtpError> {
        if data.len() < GTP_U_HEADER_LEN {
            return Err(GtpError::PacketTooShort(data.len()));
        }

        let flags = data[0];
        let version = (flags >> 5) & 0x07;
        if version != 1 {
            return Err(GtpError::InvalidVersion(version));
        }

        let protocol_type = (flags & 0x10) != 0;
        let ext_flag = (flags & 0x04) != 0;
        let seq_flag = (flags & 0x02) != 0;
        let npdu_flag = (flags & 0x01) != 0;

        let msg_type = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]);
        let teid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        let has_optional = ext_flag || seq_flag || npdu_flag;
        let header_len = if has_optional { 12 } else { 8 };

        if data.len() < header_len {
            return Err(GtpError::PacketTooShort(data.len()));
        }

        let (seq_num, npdu_num, next_ext) = if has_optional {
            let seq = u16::from_be_bytes([data[8], data[9]]);
            let npdu = data[10];
            let next = data[11];
            (Some(seq), Some(npdu), Some(next))
        } else {
            (None, None, None)
        };

        let payload = data[header_len..].to_vec();

        Ok(GtpPacket {
            header: GtpHeader {
                version,
                protocol_type,
                ext_flag,
                seq_flag,
                npdu_flag,
                msg_type,
                length,
                teid,
                seq_num,
                npdu_num,
                next_ext,
            },
            payload,
        })
    }
}

impl GtpTunnelTable {
    pub fn new() -> Self {
        GtpTunnelTable {
            sessions: BTreeMap::new(),
        }
    }

    pub fn insert_session(&mut self, teid: u32, sub_ip: Ipv4Address, node_ip: Ipv4Address) {
        self.sessions.insert(
            teid,
            GtpTunnelSession {
                teid,
                subscriber_ip: sub_ip,
                gnb_upf_ip: node_ip,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtp_gpdu_encapsulation_and_parse() {
        let teid = 0x01020304;
        let inner_ip_pkt = b"IPv4 Subscriber Cellular Data Payload (e.g. Video Stream)";
        let pkt = GtpPacket::build_gpdu(teid, inner_ip_pkt);
        let raw = pkt.serialize();

        assert_eq!(raw.len(), 8 + inner_ip_pkt.len());
        let parsed = GtpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.version, 1);
        assert_eq!(parsed.header.msg_type, GTP_MSG_GPDU);
        assert_eq!(parsed.header.teid, teid);
        assert_eq!(parsed.payload, inner_ip_pkt);
    }

    #[test]
    fn test_gtp_echo_request_response_with_sequence() {
        let echo_req = GtpPacket::build_echo_request(0, 100);
        let raw = echo_req.serialize();
        assert_eq!(raw.len(), 12);

        let parsed = GtpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.msg_type, GTP_MSG_ECHO_REQUEST);
        assert_eq!(parsed.header.seq_num, Some(100));

        let echo_resp = GtpPacket::build_echo_response(0, 100);
        let parsed_resp = GtpPacket::parse(&echo_resp.serialize()).unwrap();
        assert_eq!(parsed_resp.header.msg_type, GTP_MSG_ECHO_RESPONSE);
        assert_eq!(parsed_resp.header.seq_num, Some(100));
    }
}
