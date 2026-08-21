//! Real-time Transport Protocol (RTP) & RTCP (RFC 3550).
//!
//! Real-time audio and video streaming transport over UDP.

use std::fmt;

pub const RTP_FIXED_HEADER_LEN: usize = 12;

// Standard RTP Payload Types
pub const RTP_PT_PCMU: u8 = 0;   // G.711 mu-law audio, 8000 Hz
pub const RTP_PT_PCMA: u8 = 8;   // G.711 A-law audio, 8000 Hz
pub const RTP_PT_DYNAMIC: u8 = 96; // Dynamic payload type (e.g., Opus / H.264)

// RTCP Packet Types
pub const RTCP_PT_SR: u8 = 200;   // Sender Report
pub const RTCP_PT_RR: u8 = 201;   // Receiver Report
pub const RTCP_PT_SDES: u8 = 202; // Source Description
pub const RTCP_PT_BYE: u8 = 203;  // Goodbye

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpPacket {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub csrc_list: Vec<u32>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpSenderReport {
    pub ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtpError {
    PacketTooShort(usize),
    InvalidVersion(u8),
}

impl fmt::Display for RtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RtpError::PacketTooShort(l) => write!(f, "RTP packet too short ({} bytes)", l),
            RtpError::InvalidVersion(v) => write!(f, "Invalid RTP version: expected 2, found {}", v),
        }
    }
}

impl std::error::Error for RtpError {}

impl RtpPacket {
    pub fn build_audio(pt: u8, seq: u16, timestamp: u32, ssrc: u32, marker: bool, audio_data: &[u8]) -> Self {
        RtpPacket {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker,
            payload_type: pt & 0x7F,
            sequence_number: seq,
            timestamp,
            ssrc,
            csrc_list: Vec::new(),
            payload: audio_data.to_vec(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let cc = (self.csrc_list.len() as u8) & 0x0F;
        let mut b0 = (self.version << 6) | cc;
        if self.padding { b0 |= 0x20; }
        if self.extension { b0 |= 0x10; }
        buf.push(b0);

        let mut b1 = self.payload_type & 0x7F;
        if self.marker { b1 |= 0x80; }
        buf.push(b1);

        buf.extend_from_slice(&self.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.ssrc.to_be_bytes());

        for csrc in &self.csrc_list {
            buf.extend_from_slice(&csrc.to_be_bytes());
        }

        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<Self, RtpError> {
        if data.len() < RTP_FIXED_HEADER_LEN {
            return Err(RtpError::PacketTooShort(data.len()));
        }

        let version = data[0] >> 6;
        if version != 2 {
            return Err(RtpError::InvalidVersion(version));
        }

        let padding = (data[0] & 0x20) != 0;
        let extension = (data[0] & 0x10) != 0;
        let csrc_count = data[0] & 0x0F;

        let marker = (data[1] & 0x80) != 0;
        let payload_type = data[1] & 0x7F;

        let sequence_number = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let mut offset = RTP_FIXED_HEADER_LEN;
        let csrc_bytes = (csrc_count as usize) * 4;
        if data.len() < offset + csrc_bytes {
            return Err(RtpError::PacketTooShort(data.len()));
        }

        let mut csrc_list = Vec::new();
        for _ in 0..csrc_count {
            let csrc = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            csrc_list.push(csrc);
            offset += 4;
        }

        let payload = data[offset..].to_vec();

        Ok(RtpPacket {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc_list,
            payload,
        })
    }
}

impl RtcpSenderReport {
    pub fn build(ssrc: u32, ntp: u64, rtp_ts: u32, pkts: u32, octets: u32) -> Self {
        RtcpSenderReport {
            ssrc,
            ntp_timestamp: ntp,
            rtp_timestamp: rtp_ts,
            packet_count: pkts,
            octet_count: octets,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Byte 0: V=2, P=0, RC=0 (2 << 6 = 0x80)
        buf.push(0x80);
        buf.push(RTCP_PT_SR);
        let length_words: u16 = 6; // (28 bytes total - 4) / 4 = 6 words
        buf.extend_from_slice(&length_words.to_be_bytes());
        buf.extend_from_slice(&self.ssrc.to_be_bytes());
        buf.extend_from_slice(&self.ntp_timestamp.to_be_bytes());
        buf.extend_from_slice(&self.rtp_timestamp.to_be_bytes());
        buf.extend_from_slice(&self.packet_count.to_be_bytes());
        buf.extend_from_slice(&self.octet_count.to_be_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 || data[1] != RTCP_PT_SR {
            return None;
        }

        let ssrc = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ntp_timestamp = u64::from_be_bytes([data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]]);
        let rtp_timestamp = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let packet_count = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        let octet_count = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);

        Some(RtcpSenderReport {
            ssrc,
            ntp_timestamp,
            rtp_timestamp,
            packet_count,
            octet_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtp_audio_packet_roundtrip() {
        let audio_samples = [0xD5u8; 160]; // 20ms of G.711 audio (160 bytes @ 8kHz)
        let rtp = RtpPacket::build_audio(RTP_PT_PCMU, 1001, 160000, 0x11223344, false, &audio_samples);
        let raw = rtp.serialize();

        assert_eq!(raw.len(), RTP_FIXED_HEADER_LEN + 160);
        let parsed = RtpPacket::parse(&raw).unwrap();

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.payload_type, RTP_PT_PCMU);
        assert_eq!(parsed.sequence_number, 1001);
        assert_eq!(parsed.timestamp, 160000);
        assert_eq!(parsed.ssrc, 0x11223344);
        assert_eq!(parsed.payload.len(), 160);
    }

    #[test]
    fn test_rtcp_sender_report_roundtrip() {
        let sr = RtcpSenderReport::build(0x11223344, 0xE584123400000000, 160000, 50, 8000);
        let raw = sr.serialize();
        let parsed = RtcpSenderReport::parse(&raw).unwrap();

        assert_eq!(parsed.ssrc, 0x11223344);
        assert_eq!(parsed.packet_count, 50);
        assert_eq!(parsed.octet_count, 8000);
    }
}
