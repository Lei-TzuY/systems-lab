//! IPsec Encapsulating Security Payload (ESP - RFC 4303).
//!
//! Network layer packet security, confidentiality, data origin authentication,
//! and anti-replay protection operating over IP Protocol 50.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;
use std::fmt;

pub const IP_PROTO_ESP: u8 = 50;
pub const ESP_HEADER_LEN: usize = 8;
pub const ESP_ICV_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspHeader {
    pub spi: u32,
    pub seq_num: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspPacket {
    pub header: EspHeader,
    pub payload: Vec<u8>,
    pub pad_length: u8,
    pub next_header: u8,
    pub icv: [u8; ESP_ICV_LEN],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspError {
    PacketTooShort(usize),
    InvalidPadding,
    InvalidIcv,
    ReplayDetected(u32),
    SaNotFound(u32),
}

impl fmt::Display for EspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EspError::PacketTooShort(l) => write!(f, "ESP packet too short ({} bytes, min 26)", l),
            EspError::InvalidPadding => write!(f, "ESP trailer padding verification failed"),
            EspError::InvalidIcv => {
                write!(f, "ESP Integrity Check Value (ICV) authentication failed")
            }
            EspError::ReplayDetected(s) => {
                write!(f, "ESP anti-replay check failed: sequence #{}", s)
            }
            EspError::SaNotFound(spi) => write!(
                f,
                "Security Association (SA) not found for SPI 0x{:08X}",
                spi
            ),
        }
    }
}

impl std::error::Error for EspError {}

impl EspPacket {
    pub fn parse(data: &[u8]) -> Result<Self, EspError> {
        if data.len() < ESP_HEADER_LEN + 2 + ESP_ICV_LEN {
            return Err(EspError::PacketTooShort(data.len()));
        }

        let spi = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let seq_num = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        let icv_start = data.len() - ESP_ICV_LEN;
        let mut icv = [0u8; ESP_ICV_LEN];
        icv.copy_from_slice(&data[icv_start..]);

        let trailer_start = icv_start - 2;
        let pad_length = data[trailer_start] as usize;
        let next_header = data[trailer_start + 1];

        if ESP_HEADER_LEN + pad_length > trailer_start {
            return Err(EspError::InvalidPadding);
        }

        let payload_len = trailer_start - ESP_HEADER_LEN - pad_length;
        let payload = data[ESP_HEADER_LEN..ESP_HEADER_LEN + payload_len].to_vec();

        Ok(EspPacket {
            header: EspHeader { spi, seq_num },
            payload,
            pad_length: pad_length as u8,
            next_header,
            icv,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let pad_len = self.pad_length as usize;
        let total_len = ESP_HEADER_LEN + self.payload.len() + pad_len + 2 + ESP_ICV_LEN;
        let mut buf = vec![0u8; total_len];

        // 1. Header
        buf[0..4].copy_from_slice(&self.header.spi.to_be_bytes());
        buf[4..8].copy_from_slice(&self.header.seq_num.to_be_bytes());

        // 2. Payload
        buf[8..8 + self.payload.len()].copy_from_slice(&self.payload);

        // 3. Padding (1, 2, 3... pattern per RFC 4303)
        let pad_start = 8 + self.payload.len();
        for i in 0..pad_len {
            buf[pad_start + i] = (i + 1) as u8;
        }

        // 4. Trailer
        let trailer_start = pad_start + pad_len;
        buf[trailer_start] = self.pad_length;
        buf[trailer_start + 1] = self.next_header;

        // 5. ICV
        let icv_start = trailer_start + 2;
        buf[icv_start..icv_start + ESP_ICV_LEN].copy_from_slice(&self.icv);

        buf
    }

    pub fn build(spi: u32, seq_num: u32, next_header: u8, payload: &[u8], key: &[u8; 16]) -> Self {
        // Calculate 4-byte alignment padding
        let unpadded_len = payload.len() + 2; // + pad_len + next_hdr
        let rem = unpadded_len % 4;
        let pad_length = if rem == 0 { 0 } else { (4 - rem) as u8 };

        // Generate synthetic ICV over header + payload + trailer using key
        let mut icv = [0u8; ESP_ICV_LEN];
        for (i, b) in payload.iter().enumerate() {
            icv[i % ESP_ICV_LEN] ^= b ^ key[i % 16];
        }
        let spi_b = spi.to_be_bytes();
        let seq_b = seq_num.to_be_bytes();
        for i in 0..4 {
            icv[i] ^= spi_b[i];
            icv[4 + i] ^= seq_b[i];
        }

        EspPacket {
            header: EspHeader { spi, seq_num },
            payload: payload.to_vec(),
            pad_length,
            next_header,
            icv,
        }
    }
}

/// Security Association (SA) for IPsec Tunnel
#[derive(Debug, Clone)]
pub struct SecurityAssociation {
    pub spi: u32,
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub key: [u8; 16],
    pub next_seq: u32,
    pub highest_seq_seen: u32,
    pub replay_bitmap: u64,
}

impl SecurityAssociation {
    pub fn new(spi: u32, src_ip: Ipv4Address, dst_ip: Ipv4Address, key: [u8; 16]) -> Self {
        SecurityAssociation {
            spi,
            src_ip,
            dst_ip,
            key,
            next_seq: 1,
            highest_seq_seen: 0,
            replay_bitmap: 0,
        }
    }

    pub fn check_anti_replay(&mut self, seq: u32) -> bool {
        if seq == 0 {
            return false;
        }

        if seq > self.highest_seq_seen {
            let diff = seq - self.highest_seq_seen;
            if diff < 64 {
                self.replay_bitmap <<= diff;
                self.replay_bitmap |= 1;
            } else {
                self.replay_bitmap = 1;
            }
            self.highest_seq_seen = seq;
            true
        } else {
            let diff = self.highest_seq_seen - seq;
            if diff >= 64 {
                return false; // Packet too old
            }
            if (self.replay_bitmap & (1 << diff)) != 0 {
                return false; // Replay detected
            }
            self.replay_bitmap |= 1 << diff;
            true
        }
    }
}

/// Security Association Database (SAD)
pub struct SadTable {
    pub inbound: HashMap<u32, SecurityAssociation>,
    pub outbound: HashMap<u32, SecurityAssociation>,
}

impl Default for SadTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SadTable {
    pub fn new() -> Self {
        let mut sad = SadTable {
            inbound: HashMap::new(),
            outbound: HashMap::new(),
        };

        // Pre-configure sample SA with SPI 0x1000 and 0x2000
        let sa_out = SecurityAssociation::new(
            0x1000,
            Ipv4Address::new(192, 168, 1, 100),
            Ipv4Address::new(192, 168, 1, 10),
            [0xAA; 16],
        );
        let sa_in = SecurityAssociation::new(
            0x2000,
            Ipv4Address::new(192, 168, 1, 10),
            Ipv4Address::new(192, 168, 1, 100),
            [0xAA; 16],
        );

        sad.outbound.insert(0x1000, sa_out);
        sad.inbound.insert(0x2000, sa_in);
        sad
    }

    pub fn insert_sa(&mut self, sa: SecurityAssociation, is_inbound: bool) {
        if is_inbound {
            self.inbound.insert(sa.spi, sa);
        } else {
            self.outbound.insert(sa.spi, sa);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esp_packet_roundtrip() {
        let key = [0x5A; 16];
        let payload = b"Secret VPN Payload over IPsec ESP";
        let esp = EspPacket::build(0x1000, 1, 4, payload, &key);

        let raw = esp.serialize();
        assert!(raw.len() >= ESP_HEADER_LEN + payload.len() + 2 + ESP_ICV_LEN);

        let parsed = EspPacket::parse(&raw).unwrap();
        assert_eq!(parsed.header.spi, 0x1000);
        assert_eq!(parsed.header.seq_num, 1);
        assert_eq!(parsed.next_header, 4);
        assert_eq!(parsed.payload, payload);
    }

    #[test]
    fn test_esp_anti_replay_window() {
        let mut sa = SecurityAssociation::new(
            0x1000,
            Ipv4Address::new(1, 1, 1, 1),
            Ipv4Address::new(2, 2, 2, 2),
            [0; 16],
        );

        // Sequential packets: accepted
        assert!(sa.check_anti_replay(1));
        assert!(sa.check_anti_replay(2));
        assert!(sa.check_anti_replay(3));

        // Replay of sequence 2: rejected
        assert!(!sa.check_anti_replay(2));

        // Out-of-order jump
        assert!(sa.check_anti_replay(10));
        assert!(sa.check_anti_replay(4)); // Within window: accepted
        assert!(!sa.check_anti_replay(4)); // Replay: rejected
    }
}
