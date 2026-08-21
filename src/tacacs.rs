//! Terminal Access Controller Access-Control System Plus (TACACS+ - RFC 8907).
//!
//! Enterprise AAA (Authentication, Authorization, Accounting) administrative protocol over TCP port 49.

use std::collections::BTreeMap;
use std::fmt;

pub const TACACS_PORT: u16 = 49;
pub const TACACS_HEADER_LEN: usize = 12;
pub const TACACS_MAJOR_VER: u8 = 0xC0; // Major version 12

// TACACS+ Packet Types
pub const TACACS_TYPE_AUTHEN: u8 = 1;
pub const TACACS_TYPE_AUTHOR: u8 = 2;
pub const TACACS_TYPE_ACCT: u8 = 3;

// Authentication Status Codes
pub const TACACS_AUTHEN_STATUS_PASS: u8 = 1;
pub const TACACS_AUTHEN_STATUS_FAIL: u8 = 2;
pub const TACACS_AUTHEN_STATUS_GETDATA: u8 = 3;
pub const TACACS_AUTHEN_STATUS_GETUSER: u8 = 4;
pub const TACACS_AUTHEN_STATUS_GETPASS: u8 = 5;
pub const TACACS_AUTHEN_STATUS_ERROR: u8 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacacsHeader {
    pub version: u8,
    pub packet_type: u8,
    pub seq_no: u8,
    pub flags: u8,
    pub session_id: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacacsPacket {
    pub header: TacacsHeader,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct TacacsServer {
    pub users: BTreeMap<String, String>, // username -> password
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TacacsError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidLength,
}

impl fmt::Display for TacacsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TacacsError::PacketTooShort(l) => write!(f, "TACACS+ packet too short ({} bytes)", l),
            TacacsError::InvalidVersion(v) => write!(f, "Invalid TACACS+ version: 0x{:02X}", v),
            TacacsError::InvalidLength => write!(f, "TACACS+ body length mismatch"),
        }
    }
}

impl std::error::Error for TacacsError {}

impl TacacsPacket {
    pub fn build_authen_start(session_id: u32, user: &str, port: &str, pass: &str) -> Self {
        let mut body = Vec::new();
        body.push(1); // Action: Login
        body.push(15); // Priv_lvl: 15 (Admin)
        body.push(1); // Authen_type: ASCII
        body.push(1); // Service: Login
        body.push(user.len() as u8);
        body.push(port.len() as u8);
        body.push(0); // Rem_addr_len
        body.push(pass.len() as u8);

        body.extend_from_slice(user.as_bytes());
        body.extend_from_slice(port.as_bytes());
        body.extend_from_slice(pass.as_bytes());

        TacacsPacket {
            header: TacacsHeader {
                version: TACACS_MAJOR_VER,
                packet_type: TACACS_TYPE_AUTHEN,
                seq_no: 1,
                flags: 0x01, // Unencrypted payload
                session_id,
                length: body.len() as u32,
            },
            body,
        }
    }

    pub fn build_authen_reply(session_id: u32, seq_no: u8, status: u8, server_msg: &str) -> Self {
        let mut body = Vec::new();
        body.push(status);
        body.push(0x00); // Flags
        let msg_len = server_msg.len() as u16;
        body.extend_from_slice(&msg_len.to_be_bytes());
        body.extend_from_slice(&0u16.to_be_bytes()); // Data_len
        body.extend_from_slice(server_msg.as_bytes());

        TacacsPacket {
            header: TacacsHeader {
                version: TACACS_MAJOR_VER,
                packet_type: TACACS_TYPE_AUTHEN,
                seq_no,
                flags: 0x01,
                session_id,
                length: body.len() as u32,
            },
            body,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.header.version);
        buf.push(self.header.packet_type);
        buf.push(self.header.seq_no);
        buf.push(self.header.flags);
        buf.extend_from_slice(&self.header.session_id.to_be_bytes());
        buf.extend_from_slice(&self.header.length.to_be_bytes());
        buf.extend_from_slice(&self.body);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, TacacsError> {
        if data.len() < TACACS_HEADER_LEN {
            return Err(TacacsError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if (version & 0xF0) != TACACS_MAJOR_VER {
            return Err(TacacsError::InvalidVersion(version));
        }

        let packet_type = data[1];
        let seq_no = data[2];
        let flags = data[3];
        let session_id = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let length = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;

        if data.len() < TACACS_HEADER_LEN + length {
            return Err(TacacsError::PacketTooShort(data.len()));
        }

        let body = data[TACACS_HEADER_LEN..TACACS_HEADER_LEN + length].to_vec();

        Ok(TacacsPacket {
            header: TacacsHeader {
                version,
                packet_type,
                seq_no,
                flags,
                session_id,
                length: length as u32,
            },
            body,
        })
    }
}

impl TacacsServer {
    pub fn new() -> Self {
        let mut users = BTreeMap::new();
        users.insert("admin".to_string(), "cisco123".to_string());
        users.insert("network_op".to_string(), "toystack_secret".to_string());
        TacacsServer { users }
    }

    pub fn authenticate(&self, pkt: &TacacsPacket) -> TacacsPacket {
        if pkt.header.packet_type != TACACS_TYPE_AUTHEN || pkt.body.len() < 8 {
            return TacacsPacket::build_authen_reply(
                pkt.header.session_id,
                pkt.header.seq_no + 1,
                TACACS_AUTHEN_STATUS_ERROR,
                "Malformed request",
            );
        }

        let user_len = pkt.body[4] as usize;
        let port_len = pkt.body[5] as usize;
        let data_len = pkt.body[7] as usize;

        if pkt.body.len() < 8 + user_len + port_len + data_len {
            return TacacsPacket::build_authen_reply(
                pkt.header.session_id,
                pkt.header.seq_no + 1,
                TACACS_AUTHEN_STATUS_ERROR,
                "Payload truncated",
            );
        }

        let user_bytes = &pkt.body[8..8 + user_len];
        let pass_bytes = &pkt.body[8 + user_len + port_len..8 + user_len + port_len + data_len];

        let user = String::from_utf8_lossy(user_bytes).to_string();
        let pass = String::from_utf8_lossy(pass_bytes).to_string();

        if let Some(expected_pass) = self.users.get(&user) {
            if expected_pass == &pass {
                TacacsPacket::build_authen_reply(
                    pkt.header.session_id,
                    pkt.header.seq_no + 1,
                    TACACS_AUTHEN_STATUS_PASS,
                    "Access Granted: TACACS+ Authentication Successful",
                )
            } else {
                TacacsPacket::build_authen_reply(
                    pkt.header.session_id,
                    pkt.header.seq_no + 1,
                    TACACS_AUTHEN_STATUS_FAIL,
                    "Access Denied: Invalid Password",
                )
            }
        } else {
            TacacsPacket::build_authen_reply(
                pkt.header.session_id,
                pkt.header.seq_no + 1,
                TACACS_AUTHEN_STATUS_FAIL,
                "Access Denied: User Not Found",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tacacs_packet_roundtrip() {
        let pkt = TacacsPacket::build_authen_start(0x12345678, "admin", "tty0", "cisco123");
        let raw = pkt.serialize();

        assert_eq!(raw.len() >= TACACS_HEADER_LEN, true);
        let parsed = TacacsPacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.session_id, 0x12345678);
        assert_eq!(parsed.header.seq_no, 1);
        assert_eq!(parsed.header.packet_type, TACACS_TYPE_AUTHEN);
    }

    #[test]
    fn test_tacacs_server_authentication_flow() {
        let server = TacacsServer::new();

        // Valid Auth
        let req_valid = TacacsPacket::build_authen_start(0x99, "admin", "console", "cisco123");
        let resp_valid = server.authenticate(&req_valid);
        assert_eq!(resp_valid.header.session_id, 0x99);
        assert_eq!(resp_valid.header.seq_no, 2);
        assert_eq!(resp_valid.body[0], TACACS_AUTHEN_STATUS_PASS);

        // Invalid Auth
        let req_invalid = TacacsPacket::build_authen_start(0x99, "admin", "console", "wrongpass");
        let resp_invalid = server.authenticate(&req_invalid);
        assert_eq!(resp_invalid.body[0], TACACS_AUTHEN_STATUS_FAIL);
    }
}
