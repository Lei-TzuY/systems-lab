//! Two-Way Active Measurement Protocol (TWAMP - RFC 5357 / RFC 4656).
//!
//! Carrier-grade IP performance measurement for two-way latency, jitter, packet loss, and asymmetric link delays.

pub const TWAMP_CONTROL_PORT: u16 = 862;
pub const TWAMP_TEST_PORT: u16 = 862;

pub const TWAMP_MODE_UNAUTHENTICATED: u32 = 1;
pub const TWAMP_MODE_AUTHENTICATED: u32 = 2;
pub const TWAMP_MODE_ENCRYPTED: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwampServerGreeting {
    pub modes: u32,
    pub challenge: [u8; 16],
    pub salt: [u8; 16],
    pub count: u32,
}

impl TwampServerGreeting {
    pub fn new(modes: u32) -> Self {
        TwampServerGreeting {
            modes,
            challenge: [0x11; 16],
            salt: [0x22; 16],
            count: 1024,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 64];
        buf[12..16].copy_from_slice(&self.modes.to_be_bytes());
        buf[16..32].copy_from_slice(&self.challenge);
        buf[32..48].copy_from_slice(&self.salt);
        buf[48..52].copy_from_slice(&self.count.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 64 {
            return None;
        }
        let modes = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let mut challenge = [0u8; 16];
        challenge.copy_from_slice(&data[16..32]);
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&data[32..48]);
        let count = u32::from_be_bytes([data[48], data[49], data[50], data[51]]);
        Some(TwampServerGreeting {
            modes,
            challenge,
            salt,
            count,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwampTestPacket {
    pub seq_number: u32,
    pub timestamp_sec: u32,
    pub timestamp_frac: u32,
    pub error_estimate: u16,
    // Reflector additions (in unauthenticated mode, packet is 41 bytes)
    pub receive_timestamp_sec: Option<u32>,
    pub receive_timestamp_frac: Option<u32>,
    pub sender_seq_number: Option<u32>,
    pub sender_timestamp_sec: Option<u32>,
    pub sender_timestamp_frac: Option<u32>,
    pub sender_error_estimate: Option<u16>,
    pub sender_ttl: Option<u8>,
}

impl TwampTestPacket {
    pub fn build_sender_request(seq: u32, sec: u32, frac: u32) -> Self {
        TwampTestPacket {
            seq_number: seq,
            timestamp_sec: sec,
            timestamp_frac: frac,
            error_estimate: 0x0001,
            receive_timestamp_sec: None,
            receive_timestamp_frac: None,
            sender_seq_number: None,
            sender_timestamp_sec: None,
            sender_timestamp_frac: None,
            sender_error_estimate: None,
            sender_ttl: None,
        }
    }

    pub fn build_reflector_response(
        req: &TwampTestPacket,
        reflector_seq: u32,
        rx_sec: u32,
        rx_frac: u32,
        tx_sec: u32,
        tx_frac: u32,
        ttl: u8,
    ) -> Self {
        TwampTestPacket {
            seq_number: reflector_seq,
            timestamp_sec: tx_sec,
            timestamp_frac: tx_frac,
            error_estimate: 0x0001,
            receive_timestamp_sec: Some(rx_sec),
            receive_timestamp_frac: Some(rx_frac),
            sender_seq_number: Some(req.seq_number),
            sender_timestamp_sec: Some(req.timestamp_sec),
            sender_timestamp_frac: Some(req.timestamp_frac),
            sender_error_estimate: Some(req.error_estimate),
            sender_ttl: Some(ttl),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.seq_number.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_sec.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_frac.to_be_bytes());
        buf.extend_from_slice(&self.error_estimate.to_be_bytes());
        buf.extend_from_slice(&[0u8; 2]); // Padding

        if let (Some(rx_s), Some(rx_f), Some(s_seq), Some(s_s), Some(s_f), Some(s_err), Some(ttl)) = (
            self.receive_timestamp_sec,
            self.receive_timestamp_frac,
            self.sender_seq_number,
            self.sender_timestamp_sec,
            self.sender_timestamp_frac,
            self.sender_error_estimate,
            self.sender_ttl,
        ) {
            buf.extend_from_slice(&rx_s.to_be_bytes());
            buf.extend_from_slice(&rx_f.to_be_bytes());
            buf.extend_from_slice(&s_seq.to_be_bytes());
            buf.extend_from_slice(&s_s.to_be_bytes());
            buf.extend_from_slice(&s_f.to_be_bytes());
            buf.extend_from_slice(&s_err.to_be_bytes());
            buf.extend_from_slice(&[0u8; 2]); // Padding
            buf.push(ttl);
            buf.extend_from_slice(&[0u8; 3]); // Padding to 41+ bytes
        }

        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }

        let seq_number = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let timestamp_sec = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let timestamp_frac = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let error_estimate = u16::from_be_bytes([data[12], data[13]]);

        if data.len() >= 41 {
            let rx_sec = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let rx_frac = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            let s_seq = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
            let s_sec = u32::from_be_bytes([data[28], data[29], data[30], data[31]]);
            let s_frac = u32::from_be_bytes([data[32], data[33], data[34], data[35]]);
            let s_err = u16::from_be_bytes([data[36], data[37]]);
            let ttl = data[40];

            Some(TwampTestPacket {
                seq_number,
                timestamp_sec,
                timestamp_frac,
                error_estimate,
                receive_timestamp_sec: Some(rx_sec),
                receive_timestamp_frac: Some(rx_frac),
                sender_seq_number: Some(s_seq),
                sender_timestamp_sec: Some(s_sec),
                sender_timestamp_frac: Some(s_frac),
                sender_error_estimate: Some(s_err),
                sender_ttl: Some(ttl),
            })
        } else {
            Some(TwampTestPacket {
                seq_number,
                timestamp_sec,
                timestamp_frac,
                error_estimate,
                receive_timestamp_sec: None,
                receive_timestamp_frac: None,
                sender_seq_number: None,
                sender_timestamp_sec: None,
                sender_timestamp_frac: None,
                sender_error_estimate: None,
                sender_ttl: None,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwampMetrics {
    pub rtt_us: f64,
    pub forward_delay_us: f64,
    pub reverse_delay_us: f64,
}

pub fn calculate_twamp_metrics(
    t1_sec: u32,
    t1_frac: u32,
    t2_sec: u32,
    t2_frac: u32,
    t3_sec: u32,
    t3_frac: u32,
    t4_sec: u32,
    t4_frac: u32,
) -> TwampMetrics {
    let t1 = t1_sec as f64 + (t1_frac as f64 / 4294967296.0);
    let t2 = t2_sec as f64 + (t2_frac as f64 / 4294967296.0);
    let t3 = t3_sec as f64 + (t3_frac as f64 / 4294967296.0);
    let t4 = t4_sec as f64 + (t4_frac as f64 / 4294967296.0);

    let rtt = (t4 - t1) - (t3 - t2);
    let fwd = t2 - t1;
    let rev = t4 - t3;

    TwampMetrics {
        rtt_us: rtt * 1_000_000.0,
        forward_delay_us: fwd * 1_000_000.0,
        reverse_delay_us: rev * 1_000_000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twamp_greeting_and_test_packet_codec() {
        let greeting = TwampServerGreeting::new(TWAMP_MODE_UNAUTHENTICATED);
        let raw_greet = greeting.serialize();
        let parsed_greet = TwampServerGreeting::parse(&raw_greet).unwrap();
        assert_eq!(parsed_greet.modes, TWAMP_MODE_UNAUTHENTICATED);
        assert_eq!(parsed_greet.count, 1024);

        let req = TwampTestPacket::build_sender_request(1, 1700000000, 100000);
        let raw_req = req.serialize();
        let parsed_req = TwampTestPacket::parse(&raw_req).unwrap();
        assert_eq!(parsed_req.seq_number, 1);
        assert_eq!(parsed_req.timestamp_sec, 1700000000);

        let resp = TwampTestPacket::build_reflector_response(
            &req,
            101,
            1700000000,
            100500,
            1700000000,
            100600,
            64,
        );
        let raw_resp = resp.serialize();
        let parsed_resp = TwampTestPacket::parse(&raw_resp).unwrap();
        assert_eq!(parsed_resp.seq_number, 101);
        assert_eq!(parsed_resp.sender_seq_number, Some(1));
        assert_eq!(parsed_resp.sender_ttl, Some(64));
    }
}
