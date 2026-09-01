//! Bidirectional Forwarding Detection (BFD - RFC 5880 / RFC 5881).
//!
//! Sub-second link and path liveness detection operating over UDP port 3784 (Control)
//! and UDP port 3785 (Echo). Supports authentication (Simple Password, Keyed MD5/SHA1).

use std::fmt;

pub const BFD_CONTROL_PORT: u16 = 3784;
pub const BFD_ECHO_PORT: u16 = 3785;
pub const BFD_MIN_PACKET_LEN: usize = 24;

// BFD Authentication Types (RFC 5880 Section 4.1)
pub const BFD_AUTH_SIMPLE_PASSWORD: u8 = 1;
pub const BFD_AUTH_KEYED_MD5: u8 = 2;
pub const BFD_AUTH_METICULOUS_KEYED_MD5: u8 = 3;
pub const BFD_AUTH_KEYED_SHA1: u8 = 4;
pub const BFD_AUTH_METICULOUS_KEYED_SHA1: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BfdState {
    AdminDown = 0,
    Down = 1,
    Init = 2,
    Up = 3,
}

impl BfdState {
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => BfdState::AdminDown,
            1 => BfdState::Down,
            2 => BfdState::Init,
            3 => BfdState::Up,
            _ => BfdState::Down,
        }
    }
}

impl fmt::Display for BfdState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BfdState::AdminDown => write!(f, "ADMIN_DOWN"),
            BfdState::Down => write!(f, "DOWN"),
            BfdState::Init => write!(f, "INIT"),
            BfdState::Up => write!(f, "UP"),
        }
    }
}

/// BFD Authentication Header (RFC 5880 Section 6.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfdAuthHeader {
    SimplePassword {
        key_id: u8,
        password: Vec<u8>,
    },
    KeyedMd5 {
        meticulous: bool,
        key_id: u8,
        sequence_number: u32,
        auth_key_hash: [u8; 16],
    },
    KeyedSha1 {
        meticulous: bool,
        key_id: u8,
        sequence_number: u32,
        auth_key_hash: [u8; 20],
    },
    Raw {
        auth_type: u8,
        key_id: u8,
        data: Vec<u8>,
    },
}

impl BfdAuthHeader {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            BfdAuthHeader::SimplePassword { key_id, password } => {
                buf.push(BFD_AUTH_SIMPLE_PASSWORD);
                let auth_len = 3 + password.len().min(16);
                buf.push(auth_len as u8);
                buf.push(*key_id);
                buf.extend_from_slice(&password[..password.len().min(16)]);
            }
            BfdAuthHeader::KeyedMd5 {
                meticulous,
                key_id,
                sequence_number,
                auth_key_hash,
            } => {
                buf.push(if *meticulous {
                    BFD_AUTH_METICULOUS_KEYED_MD5
                } else {
                    BFD_AUTH_KEYED_MD5
                });
                buf.push(24); // Length: 1 auth_type + 1 auth_len + 1 key_id + 1 reserved + 4 seq + 16 digest
                buf.push(*key_id);
                buf.push(0); // Reserved
                buf.extend_from_slice(&sequence_number.to_be_bytes());
                buf.extend_from_slice(auth_key_hash);
            }
            BfdAuthHeader::KeyedSha1 {
                meticulous,
                key_id,
                sequence_number,
                auth_key_hash,
            } => {
                buf.push(if *meticulous {
                    BFD_AUTH_METICULOUS_KEYED_SHA1
                } else {
                    BFD_AUTH_KEYED_SHA1
                });
                buf.push(28); // Length: 1 auth_type + 1 auth_len + 1 key_id + 1 reserved + 4 seq + 20 digest
                buf.push(*key_id);
                buf.push(0); // Reserved
                buf.extend_from_slice(&sequence_number.to_be_bytes());
                buf.extend_from_slice(auth_key_hash);
            }
            BfdAuthHeader::Raw {
                auth_type,
                key_id,
                data,
            } => {
                buf.push(*auth_type);
                buf.push((3 + data.len()) as u8);
                buf.push(*key_id);
                buf.extend_from_slice(data);
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let auth_type = data[0];
        let auth_len = data[1] as usize;
        if auth_len < 3 || auth_len > data.len() {
            return None;
        }
        let key_id = data[2];

        match auth_type {
            BFD_AUTH_SIMPLE_PASSWORD => {
                let password = data[3..auth_len].to_vec();
                Some(BfdAuthHeader::SimplePassword { key_id, password })
            }
            BFD_AUTH_KEYED_MD5 | BFD_AUTH_METICULOUS_KEYED_MD5 if auth_len >= 24 => {
                let sequence_number = u32::from_be_bytes(data[4..8].try_into().ok()?);
                let mut auth_key_hash = [0u8; 16];
                auth_key_hash.copy_from_slice(&data[8..24]);
                Some(BfdAuthHeader::KeyedMd5 {
                    meticulous: auth_type == BFD_AUTH_METICULOUS_KEYED_MD5,
                    key_id,
                    sequence_number,
                    auth_key_hash,
                })
            }
            BFD_AUTH_KEYED_SHA1 | BFD_AUTH_METICULOUS_KEYED_SHA1 if auth_len >= 28 => {
                let sequence_number = u32::from_be_bytes(data[4..8].try_into().ok()?);
                let mut auth_key_hash = [0u8; 20];
                auth_key_hash.copy_from_slice(&data[8..28]);
                Some(BfdAuthHeader::KeyedSha1 {
                    meticulous: auth_type == BFD_AUTH_METICULOUS_KEYED_SHA1,
                    key_id,
                    sequence_number,
                    auth_key_hash,
                })
            }
            _ => {
                let raw_data = data[3..auth_len].to_vec();
                Some(BfdAuthHeader::Raw {
                    auth_type,
                    key_id,
                    data: raw_data,
                })
            }
        }
    }
}

/// BFD Echo Packet (RFC 5880 Section 6.8.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BfdEchoPacket {
    pub my_discriminator: u32,
    pub sender_timestamp_us: u64,
    pub sequence_number: u32,
    pub payload: Vec<u8>,
}

impl BfdEchoPacket {
    pub fn new(my_disc: u32, timestamp_us: u64, seq: u32, payload: &[u8]) -> Self {
        BfdEchoPacket {
            my_discriminator: my_disc,
            sender_timestamp_us: timestamp_us,
            sequence_number: seq,
            payload: payload.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16 + self.payload.len());
        buf.extend_from_slice(&self.my_discriminator.to_be_bytes());
        buf.extend_from_slice(&self.sender_timestamp_us.to_be_bytes());
        buf.extend_from_slice(&self.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, BfdError> {
        if data.len() < 16 {
            return Err(BfdError::PacketTooShort(data.len()));
        }
        let my_discriminator = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let sender_timestamp_us = u64::from_be_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]);
        let sequence_number = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let payload = data[16..].to_vec();
        Ok(BfdEchoPacket {
            my_discriminator,
            sender_timestamp_us,
            sequence_number,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BfdControlPacket {
    pub version: u8,
    pub diagnostic: u8,
    pub state: BfdState,
    pub poll: bool,
    pub r#final: bool,
    pub cpi: bool,
    pub auth: bool,
    pub demand: bool,
    pub multipoint: bool,
    pub detect_mult: u8,
    pub length: u8,
    pub my_discriminator: u32,
    pub your_discriminator: u32,
    pub desired_min_tx_interval_us: u32,
    pub required_min_rx_interval_us: u32,
    pub required_min_echo_rx_interval_us: u32,
    pub auth_header: Option<BfdAuthHeader>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfdError {
    PacketTooShort(usize),
    InvalidVersion(u8),
    InvalidLength(u8),
    ZeroMyDiscriminator,
    InvalidAuthHeader,
}

impl fmt::Display for BfdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BfdError::PacketTooShort(l) => write!(f, "BFD packet too short ({} bytes, min 24)", l),
            BfdError::InvalidVersion(v) => {
                write!(f, "Invalid BFD version: expected 1, found {}", v)
            }
            BfdError::InvalidLength(l) => write!(f, "Invalid BFD length field: {}", l),
            BfdError::ZeroMyDiscriminator => write!(f, "BFD My Discriminator must not be zero"),
            BfdError::InvalidAuthHeader => write!(f, "Invalid BFD authentication header"),
        }
    }
}

impl std::error::Error for BfdError {}

impl BfdControlPacket {
    pub fn parse(data: &[u8]) -> Result<Self, BfdError> {
        if data.len() < BFD_MIN_PACKET_LEN {
            return Err(BfdError::PacketTooShort(data.len()));
        }

        let b0 = data[0];
        let version = b0 >> 5;
        let diagnostic = b0 & 0x1F;

        if version != 1 {
            return Err(BfdError::InvalidVersion(version));
        }

        let b1 = data[1];
        let state_val = (b1 >> 6) & 0x03;
        let poll = (b1 & 0x20) != 0;
        let r#final = (b1 & 0x10) != 0;
        let cpi = (b1 & 0x08) != 0;
        let auth = (b1 & 0x04) != 0;
        let demand = (b1 & 0x02) != 0;
        let multipoint = (b1 & 0x01) != 0;

        let detect_mult = data[2];
        let length = data[3];

        if (length as usize) < BFD_MIN_PACKET_LEN || (length as usize) > data.len() {
            return Err(BfdError::InvalidLength(length));
        }

        let my_discriminator = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if my_discriminator == 0 {
            return Err(BfdError::ZeroMyDiscriminator);
        }

        let your_discriminator = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let desired_min_tx_interval_us =
            u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let required_min_rx_interval_us =
            u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let required_min_echo_rx_interval_us =
            u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

        let auth_header = if auth && data.len() > BFD_MIN_PACKET_LEN {
            BfdAuthHeader::parse(&data[BFD_MIN_PACKET_LEN..])
        } else {
            None
        };

        Ok(BfdControlPacket {
            version,
            diagnostic,
            state: BfdState::from_u8(state_val),
            poll,
            r#final,
            cpi,
            auth,
            demand,
            multipoint,
            detect_mult,
            length,
            my_discriminator,
            your_discriminator,
            desired_min_tx_interval_us,
            required_min_rx_interval_us,
            required_min_echo_rx_interval_us,
            auth_header,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let auth_bytes = self
            .auth_header
            .as_ref()
            .map(|a| a.serialize())
            .unwrap_or_default();
        let total_len = BFD_MIN_PACKET_LEN + auth_bytes.len();

        let mut buf = vec![0u8; total_len];
        buf[0] = ((self.version & 0x07) << 5) | (self.diagnostic & 0x1F);

        let state_val = self.state as u8;
        let mut b1 = (state_val & 0x03) << 6;
        if self.poll {
            b1 |= 0x20;
        }
        if self.r#final {
            b1 |= 0x10;
        }
        if self.cpi {
            b1 |= 0x08;
        }
        if self.auth || self.auth_header.is_some() {
            b1 |= 0x04;
        }
        if self.demand {
            b1 |= 0x02;
        }
        if self.multipoint {
            b1 |= 0x01;
        }
        buf[1] = b1;

        buf[2] = self.detect_mult;
        buf[3] = (total_len as u8).max(24);
        buf[4..8].copy_from_slice(&self.my_discriminator.to_be_bytes());
        buf[8..12].copy_from_slice(&self.your_discriminator.to_be_bytes());
        buf[12..16].copy_from_slice(&self.desired_min_tx_interval_us.to_be_bytes());
        buf[16..20].copy_from_slice(&self.required_min_rx_interval_us.to_be_bytes());
        buf[20..24].copy_from_slice(&self.required_min_echo_rx_interval_us.to_be_bytes());

        if !auth_bytes.is_empty() {
            buf[24..total_len].copy_from_slice(&auth_bytes);
        }

        buf
    }

    pub fn build_control(state: BfdState, my_disc: u32, your_disc: u32, interval_us: u32) -> Self {
        BfdControlPacket {
            version: 1,
            diagnostic: 0,
            state,
            poll: false,
            r#final: false,
            cpi: false,
            auth: false,
            demand: false,
            multipoint: false,
            detect_mult: 3,
            length: 24,
            my_discriminator: my_disc,
            your_discriminator: your_disc,
            desired_min_tx_interval_us: interval_us,
            required_min_rx_interval_us: interval_us,
            required_min_echo_rx_interval_us: 0,
            auth_header: None,
        }
    }

    pub fn build_authenticated(
        state: BfdState,
        my_disc: u32,
        your_disc: u32,
        interval_us: u32,
        auth_header: BfdAuthHeader,
    ) -> Self {
        let mut pkt = Self::build_control(state, my_disc, your_disc, interval_us);
        pkt.auth = true;
        pkt.auth_header = Some(auth_header);
        pkt
    }
}

/// BFD Session State Machine with Echo & Authentication support.
#[derive(Debug, Clone)]
pub struct BfdSession {
    pub local_discriminator: u32,
    pub remote_discriminator: u32,
    pub state: BfdState,
    pub tx_interval_us: u32,
    pub rx_interval_us: u32,
    pub required_min_echo_rx_interval_us: u32,
    pub detect_mult: u8,
    pub auth_key: Option<BfdAuthHeader>,
    pub echo_sequence: u32,
    pub last_echo_rtt_us: Option<u64>,
}

impl BfdSession {
    pub fn new(local_disc: u32, interval_us: u32) -> Self {
        BfdSession {
            local_discriminator: local_disc,
            remote_discriminator: 0,
            state: BfdState::Down,
            tx_interval_us: interval_us,
            rx_interval_us: interval_us,
            required_min_echo_rx_interval_us: 50_000,
            detect_mult: 3,
            auth_key: None,
            echo_sequence: 1,
            last_echo_rtt_us: None,
        }
    }

    /// Generates an outbound BFD Echo packet.
    pub fn generate_echo_packet(&mut self, now_us: u64) -> Vec<u8> {
        let echo = BfdEchoPacket::new(
            self.local_discriminator,
            now_us,
            self.echo_sequence,
            b"BFD-ECHO-PROBE",
        );
        self.echo_sequence = self.echo_sequence.wrapping_add(1);
        echo.serialize()
    }

    /// Validates an incoming looped-back BFD Echo packet and records RTT.
    pub fn process_echo_packet(&mut self, data: &[u8], now_us: u64) -> bool {
        let echo = match BfdEchoPacket::parse(data) {
            Ok(e) => e,
            Err(_) => return false,
        };
        if echo.my_discriminator != self.local_discriminator {
            return false;
        }
        let rtt = now_us.saturating_sub(echo.sender_timestamp_us);
        self.last_echo_rtt_us = Some(rtt);
        true
    }

    /// Advances the BFD FSM upon receiving a remote BFD control packet.
    pub fn process_packet(&mut self, pkt: &BfdControlPacket) -> Option<BfdControlPacket> {
        self.remote_discriminator = pkt.my_discriminator;

        match (self.state, pkt.state) {
            (BfdState::Down, BfdState::Down) => {
                self.state = BfdState::Init;
                let mut resp = BfdControlPacket::build_control(
                    self.state,
                    self.local_discriminator,
                    self.remote_discriminator,
                    self.tx_interval_us,
                );
                resp.auth_header = self.auth_key.clone();
                resp.auth = self.auth_key.is_some();
                Some(resp)
            }
            (BfdState::Down, BfdState::Init)
            | (BfdState::Init, BfdState::Init)
            | (BfdState::Init, BfdState::Up) => {
                self.state = BfdState::Up;
                let mut resp = BfdControlPacket::build_control(
                    self.state,
                    self.local_discriminator,
                    self.remote_discriminator,
                    self.tx_interval_us,
                );
                resp.auth_header = self.auth_key.clone();
                resp.auth = self.auth_key.is_some();
                Some(resp)
            }
            (BfdState::Up, BfdState::AdminDown) => {
                self.state = BfdState::Down;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bfd_packet_roundtrip() {
        let pkt = BfdControlPacket::build_control(BfdState::Up, 0x12345678, 0x87654321, 50_000);
        let raw = pkt.serialize();

        assert_eq!(raw.len(), BFD_MIN_PACKET_LEN);
        let parsed = BfdControlPacket::parse(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.state, BfdState::Up);
        assert_eq!(parsed.my_discriminator, 0x12345678);
        assert_eq!(parsed.your_discriminator, 0x87654321);
        assert_eq!(parsed.desired_min_tx_interval_us, 50_000);
    }

    #[test]
    fn test_bfd_session_state_transition() {
        let mut session = BfdSession::new(0x1001, 100_000);
        assert_eq!(session.state, BfdState::Down);

        // Remote sends Down packet -> Local transitions to Init
        let incoming_down = BfdControlPacket::build_control(BfdState::Down, 0x2002, 0, 100_000);
        let resp = session.process_packet(&incoming_down).unwrap();
        assert_eq!(session.state, BfdState::Init);
        assert_eq!(resp.state, BfdState::Init);

        // Remote sends Init/Up packet -> Local transitions to Up
        let incoming_init =
            BfdControlPacket::build_control(BfdState::Init, 0x2002, 0x1001, 100_000);
        let resp2 = session.process_packet(&incoming_init).unwrap();
        assert_eq!(session.state, BfdState::Up);
        assert_eq!(resp2.state, BfdState::Up);
    }
}
