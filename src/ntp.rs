//! Network Time Protocol Version 4 (NTPv4 - RFC 5905).
//!
//! Provides 48-byte NTP packet encoding/decoding, 64-bit fixed-point NTP timestamps,
//! and clock offset & round-trip delay calculations over UDP port 123.

use std::fmt;

pub const NTP_PORT: u16 = 123;
pub const NTP_HEADER_LEN: usize = 48;
pub const NTP_VERSION_4: u8 = 4;

// NTP Modes
pub const NTP_MODE_CLIENT: u8 = 3;
pub const NTP_MODE_SERVER: u8 = 4;

// Seconds between 1900-01-01 (NTP epoch) and 1970-01-01 (Unix epoch)
pub const NTP_UNIX_OFFSET_SECS: u64 = 2_208_988_800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NtpTimestamp {
    pub seconds: u32,
    pub fraction: u32,
}

impl NtpTimestamp {
    pub const ZERO: NtpTimestamp = NtpTimestamp { seconds: 0, fraction: 0 };

    pub fn new(seconds: u32, fraction: u32) -> Self {
        NtpTimestamp { seconds, fraction }
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&self.seconds.to_be_bytes());
        b[4..8].copy_from_slice(&self.fraction.to_be_bytes());
        b
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let seconds = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let fraction = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
        NtpTimestamp { seconds, fraction }
    }

    pub fn to_unix_f64(&self) -> f64 {
        if self.seconds == 0 && self.fraction == 0 {
            return 0.0;
        }
        let unix_secs = (self.seconds as i64) - (NTP_UNIX_OFFSET_SECS as i64);
        let frac_secs = (self.fraction as f64) / (u32::MAX as f64 + 1.0);
        (unix_secs as f64) + frac_secs
    }

    pub fn from_unix_f64(unix_secs: f64) -> Self {
        let ntp_secs = unix_secs + (NTP_UNIX_OFFSET_SECS as f64);
        let seconds = ntp_secs.floor() as u32;
        let frac = ntp_secs.fract();
        let fraction = (frac * (u32::MAX as f64 + 1.0)).round() as u32;
        NtpTimestamp { seconds, fraction }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtpPacket {
    pub leap_indicator: u8,
    pub version: u8,
    pub mode: u8,
    pub stratum: u8,
    pub poll: i8,
    pub precision: i8,
    pub root_delay: u32,
    pub root_dispersion: u32,
    pub reference_id: [u8; 4],
    pub reference_timestamp: NtpTimestamp,
    pub origin_timestamp: NtpTimestamp,
    pub receive_timestamp: NtpTimestamp,
    pub transmit_timestamp: NtpTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NtpError {
    PacketTooShort(usize),
}

impl fmt::Display for NtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NtpError::PacketTooShort(len) => write!(f, "NTP packet too short ({} bytes, min 48)", len),
        }
    }
}

impl std::error::Error for NtpError {}

impl NtpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, NtpError> {
        if data.len() < NTP_HEADER_LEN {
            return Err(NtpError::PacketTooShort(data.len()));
        }

        let b0 = data[0];
        let leap_indicator = (b0 >> 6) & 0x03;
        let version = (b0 >> 3) & 0x07;
        let mode = b0 & 0x07;

        let stratum = data[1];
        let poll = data[2] as i8;
        let precision = data[3] as i8;

        let root_delay = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let root_dispersion = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let mut reference_id = [0u8; 4];
        reference_id.copy_from_slice(&data[12..16]);

        let reference_timestamp = NtpTimestamp::from_bytes(&data[16..24]);
        let origin_timestamp = NtpTimestamp::from_bytes(&data[24..32]);
        let receive_timestamp = NtpTimestamp::from_bytes(&data[32..40]);
        let transmit_timestamp = NtpTimestamp::from_bytes(&data[40..48]);

        Ok(NtpPacket {
            leap_indicator,
            version,
            mode,
            stratum,
            poll,
            precision,
            root_delay,
            root_dispersion,
            reference_id,
            reference_timestamp,
            origin_timestamp,
            receive_timestamp,
            transmit_timestamp,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; NTP_HEADER_LEN];

        buf[0] = ((self.leap_indicator & 0x03) << 6) | ((self.version & 0x07) << 3) | (self.mode & 0x07);
        buf[1] = self.stratum;
        buf[2] = self.poll as u8;
        buf[3] = self.precision as u8;

        buf[4..8].copy_from_slice(&self.root_delay.to_be_bytes());
        buf[8..12].copy_from_slice(&self.root_dispersion.to_be_bytes());
        buf[12..16].copy_from_slice(&self.reference_id);

        buf[16..24].copy_from_slice(&self.reference_timestamp.to_bytes());
        buf[24..32].copy_from_slice(&self.origin_timestamp.to_bytes());
        buf[32..40].copy_from_slice(&self.receive_timestamp.to_bytes());
        buf[40..48].copy_from_slice(&self.transmit_timestamp.to_bytes());

        buf
    }

    /// Builds an NTP Client Request packet with the transmit timestamp (t1)
    pub fn build_client_request(client_transmit_time: NtpTimestamp) -> Self {
        NtpPacket {
            leap_indicator: 0,
            version: NTP_VERSION_4,
            mode: NTP_MODE_CLIENT,
            stratum: 0,
            poll: 4,
            precision: -6,
            root_delay: 0,
            root_dispersion: 0,
            reference_id: [0; 4],
            reference_timestamp: NtpTimestamp::ZERO,
            origin_timestamp: NtpTimestamp::ZERO,
            receive_timestamp: NtpTimestamp::ZERO,
            transmit_timestamp: client_transmit_time,
        }
    }

    /// Builds an NTP Server Response packet responding to a client query
    pub fn build_server_response(
        req: &NtpPacket,
        recv_time: NtpTimestamp,
        transmit_time: NtpTimestamp,
    ) -> Self {
        NtpPacket {
            leap_indicator: 0,
            version: NTP_VERSION_4,
            mode: NTP_MODE_SERVER,
            stratum: 1, // Stratum 1 Primary Reference
            poll: req.poll,
            precision: -20,
            root_delay: 0,
            root_dispersion: 10,
            reference_id: *b"GPS ",
            reference_timestamp: recv_time,
            origin_timestamp: req.transmit_timestamp, // t1 copied into Origin
            receive_timestamp: recv_time,             // t2
            transmit_timestamp: transmit_time,        // t3
        }
    }
}

/// Calculates clock offset theta and round-trip delay delta (RFC 5905).
///
/// Returns: (Offset in seconds, Round-Trip Delay in seconds)
pub fn calculate_offset_and_delay(t1: f64, t2: f64, t3: f64, t4: f64) -> (f64, f64) {
    let offset = ((t2 - t1) + (t3 - t4)) / 2.0;
    let delay = (t4 - t1) - (t3 - t2);
    (offset, delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntp_timestamp_roundtrip() {
        let unix_time = 1700000000.500; // 2023-11-14 22:13:20.500 UTC
        let ntp = NtpTimestamp::from_unix_f64(unix_time);
        let recovered = ntp.to_unix_f64();

        assert!((recovered - unix_time).abs() < 0.0001);
    }

    #[test]
    fn test_ntp_packet_roundtrip() {
        let t1 = NtpTimestamp::new(3900000000, 500);
        let req = NtpPacket::build_client_request(t1);
        let raw = req.serialize();

        let parsed = NtpPacket::parse(&raw).unwrap();
        assert_eq!(parsed.version, NTP_VERSION_4);
        assert_eq!(parsed.mode, NTP_MODE_CLIENT);
        assert_eq!(parsed.transmit_timestamp, t1);
    }

    #[test]
    fn test_ntp_clock_offset_and_delay() {
        let t1 = 100.0;
        let t2 = 100.050; // +50ms flight + clock sync
        let t3 = 100.055;
        let t4 = 100.110;

        let (offset, delay) = calculate_offset_and_delay(t1, t2, t3, t4);
        assert!((delay - 0.105).abs() < 0.0001);
        assert!((offset - (-0.0025)).abs() < 0.0001);
    }
}
