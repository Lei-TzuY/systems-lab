//! WireGuard VPN Protocol (Noise IK Handshake & Data Transport over UDP 51820).
//!
//! Modern, fast, and secure point-to-point Layer 3 UDP tunnel encapsulation.

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const WIREGUARD_PORT: u16 = 51820;

// Message Types
pub const WG_MSG_INITIATION: u8 = 1;
pub const WG_MSG_RESPONSE: u8 = 2;
pub const WG_MSG_COOKIE: u8 = 3;
pub const WG_MSG_DATA: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireguardMessage {
    HandshakeInitiation {
        sender_index: u32,
        unencrypted_ephemeral: [u8; 32],
        encrypted_static: [u8; 48],
        encrypted_timestamp: [u8; 28],
        mac1: [u8; 16],
        mac2: [u8; 16],
    },
    HandshakeResponse {
        sender_index: u32,
        receiver_index: u32,
        unencrypted_ephemeral: [u8; 32],
        encrypted_empty: [u8; 16],
        mac1: [u8; 16],
        mac2: [u8; 16],
    },
    CookieReply {
        receiver_index: u32,
        nonce: [u8; 24],
        encrypted_cookie: [u8; 32],
    },
    Data {
        receiver_index: u32,
        counter: u64,
        encrypted_payload: Vec<u8>, // Includes 16-byte authentication tag
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireguardError {
    PacketTooShort(usize),
    InvalidMessageType(u8),
    InvalidLength,
}

impl fmt::Display for WireguardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireguardError::PacketTooShort(l) => write!(f, "WireGuard packet too short ({} bytes)", l),
            WireguardError::InvalidMessageType(t) => write!(f, "Unknown WireGuard message type: {}", t),
            WireguardError::InvalidLength => write!(f, "Invalid WireGuard message length"),
        }
    }
}

impl std::error::Error for WireguardError {}

impl WireguardMessage {
    pub fn build_initiation(sender_index: u32, ephemeral: [u8; 32]) -> Self {
        WireguardMessage::HandshakeInitiation {
            sender_index,
            unencrypted_ephemeral: ephemeral,
            encrypted_static: [0xAA; 48],
            encrypted_timestamp: [0xBB; 28],
            mac1: [0xCC; 16],
            mac2: [0x00; 16],
        }
    }

    pub fn build_response(sender_index: u32, receiver_index: u32, ephemeral: [u8; 32]) -> Self {
        WireguardMessage::HandshakeResponse {
            sender_index,
            receiver_index,
            unencrypted_ephemeral: ephemeral,
            encrypted_empty: [0xDD; 16],
            mac1: [0xEE; 16],
            mac2: [0x00; 16],
        }
    }

    pub fn build_data(receiver_index: u32, counter: u64, plaintext_ip_packet: &[u8]) -> Self {
        // Simulated authenticated encryption (Plaintext + 16-byte Poly1305 simulated tag)
        let mut enc = plaintext_ip_packet.to_vec();
        enc.extend_from_slice(&[0xFF; 16]); // Simulated authentication tag
        WireguardMessage::Data {
            receiver_index,
            counter,
            encrypted_payload: enc,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            WireguardMessage::HandshakeInitiation {
                sender_index,
                unencrypted_ephemeral,
                encrypted_static,
                encrypted_timestamp,
                mac1,
                mac2,
            } => {
                buf.push(WG_MSG_INITIATION);
                buf.extend_from_slice(&[0, 0, 0]); // Reserved
                buf.extend_from_slice(&sender_index.to_le_bytes());
                buf.extend_from_slice(unencrypted_ephemeral);
                buf.extend_from_slice(encrypted_static);
                buf.extend_from_slice(encrypted_timestamp);
                buf.extend_from_slice(mac1);
                buf.extend_from_slice(mac2);
            }
            WireguardMessage::HandshakeResponse {
                sender_index,
                receiver_index,
                unencrypted_ephemeral,
                encrypted_empty,
                mac1,
                mac2,
            } => {
                buf.push(WG_MSG_RESPONSE);
                buf.extend_from_slice(&[0, 0, 0]); // Reserved
                buf.extend_from_slice(&sender_index.to_le_bytes());
                buf.extend_from_slice(&receiver_index.to_le_bytes());
                buf.extend_from_slice(unencrypted_ephemeral);
                buf.extend_from_slice(encrypted_empty);
                buf.extend_from_slice(mac1);
                buf.extend_from_slice(mac2);
            }
            WireguardMessage::CookieReply { receiver_index, nonce, encrypted_cookie } => {
                buf.push(WG_MSG_COOKIE);
                buf.extend_from_slice(&[0, 0, 0]);
                buf.extend_from_slice(&receiver_index.to_le_bytes());
                buf.extend_from_slice(nonce);
                buf.extend_from_slice(encrypted_cookie);
            }
            WireguardMessage::Data { receiver_index, counter, encrypted_payload } => {
                buf.push(WG_MSG_DATA);
                buf.extend_from_slice(&[0, 0, 0]); // Reserved
                buf.extend_from_slice(&receiver_index.to_le_bytes());
                buf.extend_from_slice(&counter.to_le_bytes());
                buf.extend_from_slice(encrypted_payload);
                // WireGuard aligns data packet lengths to multiples of 16 if needed
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, WireguardError> {
        if data.len() < 4 {
            return Err(WireguardError::PacketTooShort(data.len()));
        }

        let msg_type = data[0];
        match msg_type {
            WG_MSG_INITIATION => {
                if data.len() < 148 {
                    return Err(WireguardError::PacketTooShort(data.len()));
                }
                let sender_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let mut unencrypted_ephemeral = [0u8; 32];
                unencrypted_ephemeral.copy_from_slice(&data[8..40]);
                let mut encrypted_static = [0u8; 48];
                encrypted_static.copy_from_slice(&data[40..88]);
                let mut encrypted_timestamp = [0u8; 28];
                encrypted_timestamp.copy_from_slice(&data[88..116]);
                let mut mac1 = [0u8; 16];
                mac1.copy_from_slice(&data[116..132]);
                let mut mac2 = [0u8; 16];
                mac2.copy_from_slice(&data[132..148]);

                Ok(WireguardMessage::HandshakeInitiation {
                    sender_index,
                    unencrypted_ephemeral,
                    encrypted_static,
                    encrypted_timestamp,
                    mac1,
                    mac2,
                })
            }
            WG_MSG_RESPONSE => {
                if data.len() < 92 {
                    return Err(WireguardError::PacketTooShort(data.len()));
                }
                let sender_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let receiver_index = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let mut unencrypted_ephemeral = [0u8; 32];
                unencrypted_ephemeral.copy_from_slice(&data[12..44]);
                let mut encrypted_empty = [0u8; 16];
                encrypted_empty.copy_from_slice(&data[44..60]);
                let mut mac1 = [0u8; 16];
                mac1.copy_from_slice(&data[60..76]);
                let mut mac2 = [0u8; 16];
                mac2.copy_from_slice(&data[76..92]);

                Ok(WireguardMessage::HandshakeResponse {
                    sender_index,
                    receiver_index,
                    unencrypted_ephemeral,
                    encrypted_empty,
                    mac1,
                    mac2,
                })
            }
            WG_MSG_COOKIE => {
                if data.len() < 64 {
                    return Err(WireguardError::PacketTooShort(data.len()));
                }
                let receiver_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let mut nonce = [0u8; 24];
                nonce.copy_from_slice(&data[8..32]);
                let mut encrypted_cookie = [0u8; 32];
                encrypted_cookie.copy_from_slice(&data[32..64]);

                Ok(WireguardMessage::CookieReply {
                    receiver_index,
                    nonce,
                    encrypted_cookie,
                })
            }
            WG_MSG_DATA => {
                if data.len() < 16 {
                    return Err(WireguardError::PacketTooShort(data.len()));
                }
                let receiver_index = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let counter = u64::from_le_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                let payload = data[16..].to_vec();

                Ok(WireguardMessage::Data {
                    receiver_index,
                    counter,
                    encrypted_payload: payload,
                })
            }
            _ => Err(WireguardError::InvalidMessageType(msg_type)),
        }
    }
}

/// Simulated WireGuard Peer Tunnel Session
#[derive(Debug, Clone)]
pub struct WireguardPeer {
    pub public_key: [u8; 32],
    pub endpoint_ip: Ipv4Address,
    pub endpoint_port: u16,
    pub allowed_ips: Vec<(Ipv4Address, u8)>,
    pub local_index: u32,
    pub remote_index: Option<u32>,
    pub send_counter: u64,
    pub is_established: bool,
}

impl WireguardPeer {
    pub fn new(public_key: [u8; 32], endpoint_ip: Ipv4Address, endpoint_port: u16, tunnel_ip: Ipv4Address) -> Self {
        WireguardPeer {
            public_key,
            endpoint_ip,
            endpoint_port,
            allowed_ips: vec![(tunnel_ip, 32)],
            local_index: 0x01020304,
            remote_index: None,
            send_counter: 0,
            is_established: false,
        }
    }

    pub fn handle_response(&mut self, sender_index: u32, receiver_index: u32) {
        if receiver_index == self.local_index {
            self.remote_index = Some(sender_index);
            self.is_established = true;
        }
    }

    pub fn encapsulate_packet(&mut self, plaintext: &[u8]) -> Option<Vec<u8>> {
        if !self.is_established {
            return None;
        }
        let r_idx = self.remote_index?;
        let ctr = self.send_counter;
        self.send_counter += 1;
        let data_msg = WireguardMessage::build_data(r_idx, ctr, plaintext);
        Some(data_msg.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wireguard_handshake_and_data_framing() {
        let ephem = [0x42; 32];
        let init = WireguardMessage::build_initiation(0x11223344, ephem);
        let raw_init = init.serialize();
        assert_eq!(raw_init.len(), 148);

        let parsed_init = WireguardMessage::parse(&raw_init).unwrap();
        if let WireguardMessage::HandshakeInitiation { sender_index, .. } = parsed_init {
            assert_eq!(sender_index, 0x11223344);
        } else {
            panic!("Expected HandshakeInitiation");
        }

        let resp = WireguardMessage::build_response(0x55667788, 0x11223344, ephem);
        let raw_resp = resp.serialize();
        assert_eq!(raw_resp.len(), 92);

        let parsed_resp = WireguardMessage::parse(&raw_resp).unwrap();
        if let WireguardMessage::HandshakeResponse { sender_index, receiver_index, .. } = parsed_resp {
            assert_eq!(sender_index, 0x55667788);
            assert_eq!(receiver_index, 0x11223344);
        } else {
            panic!("Expected HandshakeResponse");
        }

        let data = WireguardMessage::build_data(0x55667788, 1, b"Ping through WireGuard Tunnel");
        let raw_data = data.serialize();
        assert_eq!(raw_data.len() >= 32, true);

        let parsed_data = WireguardMessage::parse(&raw_data).unwrap();
        if let WireguardMessage::Data { receiver_index, counter, encrypted_payload } = parsed_data {
            assert_eq!(receiver_index, 0x55667788);
            assert_eq!(counter, 1);
            assert_eq!(&encrypted_payload[..29], b"Ping through WireGuard Tunnel");
        } else {
            panic!("Expected Data message");
        }
    }
}
