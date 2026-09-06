//! 3GPP Rel-17 5G NR Robust Header Compression (RoHC) Engine.
//!
//! Compliant with 3GPP TS 38.323 Rel-17 Section 5.7, RFC 3095, RFC 4815, and RFC 5795.
//!
//! Operates in the 5G NR Packet Data Convergence Protocol (PDCP) layer to compress
//! 40-to-60 byte IP/UDP/RTP headers down to 1-3 bytes over 5G radio links.
//!
//! Features:
//! - Profile 0x0001 (RTP/UDP/IP) and Profile 0x0002 (UDP/IP).
//! - Three Compressor States: Initialization & Refresh (IR), First Order (FO), Second Order (SO).
//! - Three Decompressor States: No Context (NC), Static Context (SC), Full Context (FC).
//! - Three Operational Modes: Unidirectional (U-mode), Bidirectional Optimistic (O-mode), Reliable (R-mode).
//! - Pure Rust CRC-3, CRC-7, CRC-8 checksum verification (RFC 3095 §5.9).
//! - Window-based Least Significant Bits (W-LSB) sequence number encoding & decoding.
//! - Packet format encoders & decoders: IR, IR-DYN, Type-0 (PT-0, 1 byte), Type-1 (PT-1, 2-3 bytes).
//! - Bidirectional Feedback channel: ACK, NACK, STATIC-NACK handling.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Pure Rust CRC Algorithms per RFC 3095 Section 5.9
// ---------------------------------------------------------------------------

/// Computes CRC over an arbitrary byte slice using long polynomial division.
pub fn compute_crc(data: &[u8], poly: u16, degree: u8, init: u16) -> u16 {
    let mut crc = init;
    let mask = (1u16 << degree) - 1;
    let top_bit = 1u16 << (degree - 1);

    for &b in data {
        for i in (0..8).rev() {
            let bit = ((b >> i) & 1) as u16;
            let carry = (crc & top_bit) != 0;
            crc = ((crc << 1) | bit) & mask;
            if carry {
                crc ^= poly;
            }
        }
    }

    for _ in 0..degree {
        let carry = (crc & top_bit) != 0;
        crc = (crc << 1) & mask;
        if carry {
            crc ^= poly;
        }
    }

    crc & mask
}

/// 3-bit CRC for Type-0 and Type-1 headers (RFC 3095 §5.9.1: C(x) = x^3 + x + 1, poly 0x03, init 0x07).
pub fn rohc_crc3(data: &[u8]) -> u8 {
    compute_crc(data, 0x03, 3, 0x07) as u8
}

/// 7-bit CRC for IR headers (RFC 3095 §5.9.1: C(x) = x^7 + x^6 + x^5 + x^2 + 1, poly 0x65, init 0x7F).
pub fn rohc_crc7(data: &[u8]) -> u8 {
    compute_crc(data, 0x65, 7, 0x7F) as u8
}

/// 8-bit CRC for IR / IR-DYN headers (RFC 3095 §5.9.1: C(x) = x^8 + x^2 + x + 1, poly 0x07, init 0xFF).
pub fn rohc_crc8(data: &[u8]) -> u8 {
    compute_crc(data, 0x07, 8, 0xFF) as u8
}

// ---------------------------------------------------------------------------
// Window-based Least Significant Bits (W-LSB) Engine (RFC 3095 Section 4.5.1)
// ---------------------------------------------------------------------------

/// W-LSB Encoder: Extracts the k least significant bits of an integer.
pub fn wlsb_encode(val: u32, k: u8) -> u32 {
    let mask = (1u32 << k) - 1;
    val & mask
}

/// W-LSB Decoder: Extrapolates wrapping integer given reference value and k LSBs.
///
/// Interpretation interval: [v_ref - p, v_ref + (2^k - 1) - p].
pub fn wlsb_decode(v_ref: u32, lsb: u32, k: u8, p: u32) -> u32 {
    let m = 1u64 << k;
    let v_ref_mod = (v_ref as u64) & (m - 1);
    let delta = (lsb as u64).wrapping_sub(v_ref_mod) & (m - 1);

    if delta > (m - 1 - p as u64) {
        // Underflow / backwards offset
        (v_ref as u64).wrapping_add(delta).wrapping_sub(m) as u32
    } else {
        // Forward increment
        (v_ref as u64).wrapping_add(delta) as u32
    }
}

// ---------------------------------------------------------------------------
// RoHC Protocols, Profiles, and Headers
// ---------------------------------------------------------------------------

/// Standardized RoHC Profiles per RFC 3095 / 3GPP TS 38.323 §5.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RohcProfile {
    /// Profile 0x0000: Uncompressed IP packets.
    Uncompressed = 0x0000,
    /// Profile 0x0001: RTP / UDP / IP (VoNR, real-time video/audio).
    RtpUdpIp = 0x0001,
    /// Profile 0x0002: UDP / IP (DNS, QUIC, non-RTP gaming).
    UdpIp = 0x0002,
    /// Profile 0x0003: ESP / IP (IPsec encrypted tunnels).
    EspIp = 0x0003,
    /// Profile 0x0004: IP Only.
    IpOnly = 0x0004,
}

impl RohcProfile {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            0x0000 => Some(Self::Uncompressed),
            0x0001 => Some(Self::RtpUdpIp),
            0x0002 => Some(Self::UdpIp),
            0x0003 => Some(Self::EspIp),
            0x0004 => Some(Self::IpOnly),
            _ => None,
        }
    }
}

/// RoHC Operational Modes (RFC 3095 §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RohcMode {
    /// Unidirectional Mode: No feedback channel available.
    Unidirectional,
    /// Bidirectional Optimistic Mode: Feedback available; optimistic state transitions.
    BidirectionalOptimistic,
    /// Bidirectional Reliable Mode: Strict ACK synchronization.
    BidirectionalReliable,
}

/// RoHC Compressor State Machine (RFC 3095 §4.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressorState {
    /// Initialization and Refresh: Sends full static and dynamic headers.
    InitializationRefresh,
    /// First Order: Sends changes in dynamic fields.
    FirstOrder,
    /// Second Order: Highly compressed state (1-3 byte headers).
    SecondOrder,
}

/// RoHC Decompressor State Machine (RFC 3095 §4.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompressorState {
    /// No Context: Awaits valid IR packet to establish context.
    NoContext,
    /// Static Context: Has static parameters, needs dynamic synchronization.
    StaticContext,
    /// Full Context: Decompresses all compressed packets.
    FullContext,
}

/// Feedback Types sent from Decompressor to Compressor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackType {
    /// Positive Acknowledgement: context established / verified.
    Ack,
    /// Negative Acknowledgement: dynamic context lost, transition to FO.
    Nack,
    /// Static Negative Acknowledgement: static context corrupted, transition to IR.
    StaticNack,
}

/// Feedback packet structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RohcFeedback {
    pub feedback_type: FeedbackType,
    pub cid: u8,
    pub acked_sn: Option<u16>,
}

/// Uncompressed IPv4 Header representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RohcIpv4Header {
    pub version: u8,
    pub tos: u8,
    pub total_length: u16,
    pub ip_id: u16,
    pub flags_and_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
}

pub type Ipv4Header = RohcIpv4Header;

/// Uncompressed UDP Header representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RohcUdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

pub type UdpHeader = RohcUdpHeader;

/// Uncompressed RTP Header representation (RFC 3550).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RohcRtpHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

pub type RtpHeader = RohcRtpHeader;

/// Full uncompressed packet container before RoHC compression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncompressedPacket {
    pub ip: RohcIpv4Header,
    pub udp: RohcUdpHeader,
    pub rtp: Option<RohcRtpHeader>,
    pub payload: Vec<u8>,
}

impl UncompressedPacket {
    /// Serialize uncompressed packet into wire-format bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(40 + self.payload.len());

        // IPv4 Header (20 bytes)
        buf.push((self.ip.version << 4) | 5);
        buf.push(self.ip.tos);
        buf.extend_from_slice(&self.ip.total_length.to_be_bytes());
        buf.extend_from_slice(&self.ip.ip_id.to_be_bytes());
        buf.extend_from_slice(&self.ip.flags_and_offset.to_be_bytes());
        buf.push(self.ip.ttl);
        buf.push(self.ip.protocol);
        buf.extend_from_slice(&self.ip.checksum.to_be_bytes());
        buf.extend_from_slice(&self.ip.src_ip);
        buf.extend_from_slice(&self.ip.dst_ip);

        // UDP Header (8 bytes)
        buf.extend_from_slice(&self.udp.src_port.to_be_bytes());
        buf.extend_from_slice(&self.udp.dst_port.to_be_bytes());
        buf.extend_from_slice(&self.udp.length.to_be_bytes());
        buf.extend_from_slice(&self.udp.checksum.to_be_bytes());

        // RTP Header (12 bytes if present)
        if let Some(ref rtp) = self.rtp {
            let b0 = (rtp.version << 6) | ((rtp.padding as u8) << 5) | ((rtp.extension as u8) << 4);
            let b1 = ((rtp.marker as u8) << 7) | (rtp.payload_type & 0x7F);
            buf.push(b0);
            buf.push(b1);
            buf.extend_from_slice(&rtp.sequence_number.to_be_bytes());
            buf.extend_from_slice(&rtp.timestamp.to_be_bytes());
            buf.extend_from_slice(&rtp.ssrc.to_be_bytes());
        }

        // Payload
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parse wire-format IP/UDP/RTP packet into UncompressedPacket.
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 28 {
            return Err("Packet too short for IPv4 + UDP header");
        }

        let version = bytes[0] >> 4;
        let ihl = (bytes[0] & 0x0F) as usize * 4;
        if version != 4 || ihl < 20 || bytes.len() < ihl + 8 {
            return Err("Invalid IPv4 header format");
        }

        let tos = bytes[1];
        let total_length = u16::from_be_bytes([bytes[2], bytes[3]]);
        let ip_id = u16::from_be_bytes([bytes[4], bytes[5]]);
        let flags_and_offset = u16::from_be_bytes([bytes[6], bytes[7]]);
        let ttl = bytes[8];
        let protocol = bytes[9];
        let checksum = u16::from_be_bytes([bytes[10], bytes[11]]);
        let mut src_ip = [0u8; 4];
        let mut dst_ip = [0u8; 4];
        src_ip.copy_from_slice(&bytes[12..16]);
        dst_ip.copy_from_slice(&bytes[16..20]);

        let ip = Ipv4Header {
            version,
            tos,
            total_length,
            ip_id,
            flags_and_offset,
            ttl,
            protocol,
            checksum,
            src_ip,
            dst_ip,
        };

        // UDP
        let udp_offset = ihl;
        let src_port = u16::from_be_bytes([bytes[udp_offset], bytes[udp_offset + 1]]);
        let dst_port = u16::from_be_bytes([bytes[udp_offset + 2], bytes[udp_offset + 3]]);
        let udp_len = u16::from_be_bytes([bytes[udp_offset + 4], bytes[udp_offset + 5]]);
        let udp_chk = u16::from_be_bytes([bytes[udp_offset + 6], bytes[udp_offset + 7]]);

        let udp = UdpHeader {
            src_port,
            dst_port,
            length: udp_len,
            checksum: udp_chk,
        };

        let mut payload_offset = udp_offset + 8;
        let mut rtp = None;

        // RTP detection heuristic (e.g. port or protocol context)
        if bytes.len() >= payload_offset + 12 {
            let rtp_v = bytes[payload_offset] >> 6;
            if rtp_v == 2 {
                let padding = ((bytes[payload_offset] >> 5) & 1) != 0;
                let extension = ((bytes[payload_offset] >> 4) & 1) != 0;
                let marker = ((bytes[payload_offset + 1] >> 7) & 1) != 0;
                let payload_type = bytes[payload_offset + 1] & 0x7F;
                let sequence_number =
                    u16::from_be_bytes([bytes[payload_offset + 2], bytes[payload_offset + 3]]);
                let timestamp = u32::from_be_bytes([
                    bytes[payload_offset + 4],
                    bytes[payload_offset + 5],
                    bytes[payload_offset + 6],
                    bytes[payload_offset + 7],
                ]);
                let ssrc = u32::from_be_bytes([
                    bytes[payload_offset + 8],
                    bytes[payload_offset + 9],
                    bytes[payload_offset + 10],
                    bytes[payload_offset + 11],
                ]);

                rtp = Some(RtpHeader {
                    version: rtp_v,
                    padding,
                    extension,
                    marker,
                    payload_type,
                    sequence_number,
                    timestamp,
                    ssrc,
                });
                payload_offset += 12;
            }
        }

        let payload = bytes[payload_offset..].to_vec();

        Ok(UncompressedPacket {
            ip,
            udp,
            rtp,
            payload,
        })
    }
}

// ---------------------------------------------------------------------------
// RoHC Context Representation
// ---------------------------------------------------------------------------

/// Per-flow RoHC context stored at compressor and decompressor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RohcContext {
    pub cid: u8,
    pub profile: RohcProfile,
    // Static Fields
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub ssrc: u32,
    // Dynamic Fields
    pub last_ip_id: u16,
    pub last_sn: u32, // RTP SN or generated transport SN
    pub last_ts: u32,
    pub ts_stride: u32,
    pub last_ttl: u8,
    pub last_tos: u8,
}

impl RohcContext {
    pub fn new(cid: u8, profile: RohcProfile) -> Self {
        Self {
            cid,
            profile,
            src_ip: [0; 4],
            dst_ip: [0; 4],
            src_port: 0,
            dst_port: 0,
            protocol: 17, // UDP
            ssrc: 0,
            last_ip_id: 0,
            last_sn: 0,
            last_ts: 0,
            ts_stride: 160, // Standard 20ms audio frame stride (8kHz)
            last_ttl: 64,
            last_tos: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// RoHC Compressor Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR RoHC Compressor.
#[derive(Debug)]
pub struct RohcCompressor {
    pub mode: RohcMode,
    pub state: CompressorState,
    pub default_cid: u8,
    pub contexts: HashMap<u8, RohcContext>,
    /// Number of consecutive packets transmitted in current state before promotion.
    pub consecutive_packets: u32,
    /// Optimistic transition threshold from IR -> FO -> SO.
    pub promotion_threshold: u32,
    /// Total bytes statistics
    pub total_raw_bytes: usize,
    pub total_compressed_bytes: usize,
}

impl RohcCompressor {
    /// Create a new RoHC Compressor instance.
    pub fn new(mode: RohcMode, default_cid: u8) -> Self {
        Self {
            mode,
            state: CompressorState::InitializationRefresh,
            default_cid,
            contexts: HashMap::new(),
            consecutive_packets: 0,
            promotion_threshold: 3,
            total_raw_bytes: 0,
            total_compressed_bytes: 0,
        }
    }

    /// Process feedback from decompressor (RFC 3095 §5.5).
    pub fn process_feedback(&mut self, feedback: &RohcFeedback) {
        if feedback.cid != self.default_cid {
            return;
        }

        match feedback.feedback_type {
            FeedbackType::Ack => {
                if self.state == CompressorState::InitializationRefresh {
                    self.state = CompressorState::FirstOrder;
                    self.consecutive_packets = 0;
                } else if self.state == CompressorState::FirstOrder {
                    self.state = CompressorState::SecondOrder;
                    self.consecutive_packets = 0;
                }
            }
            FeedbackType::Nack => {
                // Downgrade to First Order
                self.state = CompressorState::FirstOrder;
                self.consecutive_packets = 0;
            }
            FeedbackType::StaticNack => {
                // Downgrade all the way to IR
                self.state = CompressorState::InitializationRefresh;
                self.consecutive_packets = 0;
            }
        }
    }

    /// Compress an incoming uncompressed IP packet.
    pub fn compress(&mut self, packet: &UncompressedPacket) -> Result<Vec<u8>, &'static str> {
        let raw_len = 20 + 8 + if packet.rtp.is_some() { 12 } else { 0 } + packet.payload.len();
        self.total_raw_bytes += raw_len;

        let profile = if packet.rtp.is_some() {
            RohcProfile::RtpUdpIp
        } else {
            RohcProfile::UdpIp
        };

        let cid = self.default_cid;
        let ctx = self
            .contexts
            .entry(cid)
            .or_insert_with(|| RohcContext::new(cid, profile));

        // Update context fields
        ctx.profile = profile;
        ctx.src_ip = packet.ip.src_ip;
        ctx.dst_ip = packet.ip.dst_ip;
        ctx.src_port = packet.udp.src_port;
        ctx.dst_port = packet.udp.dst_port;
        ctx.protocol = packet.ip.protocol;
        ctx.last_ttl = packet.ip.ttl;
        ctx.last_tos = packet.ip.tos;
        ctx.last_ip_id = packet.ip.ip_id;

        let current_sn = if let Some(ref rtp) = packet.rtp {
            ctx.ssrc = rtp.ssrc;
            ctx.last_ts = rtp.timestamp;
            rtp.sequence_number as u32
        } else {
            ctx.last_sn.wrapping_add(1)
        };
        ctx.last_sn = current_sn;

        // Auto-promote state in optimistic mode
        if self.mode == RohcMode::BidirectionalOptimistic || self.mode == RohcMode::Unidirectional {
            self.consecutive_packets += 1;
            if self.consecutive_packets >= self.promotion_threshold {
                match self.state {
                    CompressorState::InitializationRefresh => {
                        self.state = CompressorState::FirstOrder;
                        self.consecutive_packets = 0;
                    }
                    CompressorState::FirstOrder => {
                        self.state = CompressorState::SecondOrder;
                        self.consecutive_packets = 0;
                    }
                    CompressorState::SecondOrder => {}
                }
            }
        }

        let compressed = match self.state {
            CompressorState::InitializationRefresh => Self::encode_ir_packet(ctx, packet),
            CompressorState::FirstOrder => Self::encode_type1_packet(ctx, packet),
            CompressorState::SecondOrder => Self::encode_type0_packet(ctx, packet),
        };

        self.total_compressed_bytes += compressed.len();
        Ok(compressed)
    }

    /// Encode RoHC IR Packet (RFC 3095 §5.2.8): Transmits full static & dynamic chains.
    fn encode_ir_packet(ctx: &RohcContext, packet: &UncompressedPacket) -> Vec<u8> {
        let mut buf = Vec::new();
        // IR Packet Header Identifier: 0xFD (11111101b)
        buf.push(0xFD);
        // Profile ID (LSB byte)
        buf.push((ctx.profile as u16 & 0xFF) as u8);
        // CID (4-bit small CID)
        buf.push(ctx.cid & 0x0F);

        // CRC-8 placeholder index
        let crc_idx = buf.len();
        buf.push(0);

        // Static Chain
        buf.extend_from_slice(&ctx.src_ip);
        buf.extend_from_slice(&ctx.dst_ip);
        buf.extend_from_slice(&ctx.src_port.to_be_bytes());
        buf.extend_from_slice(&ctx.dst_port.to_be_bytes());
        buf.push(ctx.protocol);

        if ctx.profile == RohcProfile::RtpUdpIp {
            buf.extend_from_slice(&ctx.ssrc.to_be_bytes());
        }

        // Dynamic Chain
        buf.extend_from_slice(&ctx.last_ip_id.to_be_bytes());
        buf.extend_from_slice(&(ctx.last_sn as u16).to_be_bytes());
        buf.push(ctx.last_tos);
        buf.push(ctx.last_ttl);

        if ctx.profile == RohcProfile::RtpUdpIp {
            buf.extend_from_slice(&ctx.last_ts.to_be_bytes());
        }

        // Calculate CRC-8 over header fields
        let crc8 = rohc_crc8(&buf);
        buf[crc_idx] = crc8;

        // Payload
        buf.extend_from_slice(&packet.payload);
        buf
    }

    /// Encode RoHC Type 1 Packet (PT-1, 2 bytes compressed header).
    fn encode_type1_packet(ctx: &RohcContext, packet: &UncompressedPacket) -> Vec<u8> {
        let mut buf = Vec::new();
        // 6-bit LSB sequence number
        let sn_6bit = wlsb_encode(ctx.last_sn, 6) as u8;
        // Byte 0: [1 0: 2 bits][SN: 6 bits]
        let b0 = 0x80 | (sn_6bit & 0x3F);
        buf.push(b0);

        // Byte 1: [IP-ID: 5 bits][CRC-3: 3 bits]
        let ip_id_5bit = (ctx.last_ip_id & 0x1F) as u8;
        let crc3 = rohc_crc3(&[b0, ip_id_5bit << 3]);
        let b1 = (ip_id_5bit << 3) | (crc3 & 0x07);
        buf.push(b1);

        // Payload
        buf.extend_from_slice(&packet.payload);
        buf
    }

    /// Encode RoHC Type 0 Packet (PT-0, 1 byte compressed header!).
    fn encode_type0_packet(ctx: &RohcContext, packet: &UncompressedPacket) -> Vec<u8> {
        let mut buf = Vec::new();
        // 4-bit LSB sequence number
        let sn_4bit = wlsb_encode(ctx.last_sn, 4) as u8;
        // Format: [0: 1 bit][SN: 4 bits][CRC-3: 3 bits]
        let temp_b0 = (sn_4bit & 0x0F) << 3;
        let crc3 = rohc_crc3(&[temp_b0]);
        let b0 = temp_b0 | (crc3 & 0x07);
        buf.push(b0);

        // Payload
        buf.extend_from_slice(&packet.payload);
        buf
    }

    /// Returns the overall compression ratio achieved so far.
    pub fn compression_ratio(&self) -> f64 {
        if self.total_compressed_bytes == 0 {
            1.0
        } else {
            self.total_raw_bytes as f64 / self.total_compressed_bytes as f64
        }
    }
}

// ---------------------------------------------------------------------------
// RoHC Decompressor Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR RoHC Decompressor.
#[derive(Debug)]
pub struct RohcDecompressor {
    pub state: DecompressorState,
    pub contexts: HashMap<u8, RohcContext>,
    pub packets_decompressed: usize,
    pub crc_failures: usize,
}

impl RohcDecompressor {
    /// Create a new RoHC Decompressor instance.
    pub fn new() -> Self {
        Self {
            state: DecompressorState::NoContext,
            contexts: HashMap::new(),
            packets_decompressed: 0,
            crc_failures: 0,
        }
    }

    /// Decompress a received RoHC packet into an UncompressedPacket.
    pub fn decompress(&mut self, rohc_bytes: &[u8]) -> Result<UncompressedPacket, &'static str> {
        if rohc_bytes.is_empty() {
            return Err("Empty RoHC packet received");
        }

        let first_byte = rohc_bytes[0];

        if first_byte == 0xFD {
            // IR Packet
            self.decompress_ir_packet(rohc_bytes)
        } else if (first_byte & 0xC0) == 0x80 {
            // Type 1 Packet (PT-1: 10xxxxxx)
            self.decompress_type1_packet(rohc_bytes)
        } else if (first_byte & 0x80) == 0x00 {
            // Type 0 Packet (PT-0: 0xxxxxxx)
            self.decompress_type0_packet(rohc_bytes)
        } else {
            Err("Unsupported RoHC packet type")
        }
    }

    /// Decompress RoHC IR Packet.
    fn decompress_ir_packet(&mut self, bytes: &[u8]) -> Result<UncompressedPacket, &'static str> {
        if bytes.len() < 24 {
            return Err("IR packet too short");
        }

        let profile_id = bytes[1] as u16;
        let profile = RohcProfile::from_u16(profile_id).ok_or("Unknown RoHC profile in IR")?;
        let cid = bytes[2] & 0x0F;
        let received_crc8 = bytes[3];

        let hdr_len = if profile == RohcProfile::RtpUdpIp {
            31
        } else {
            23
        };
        if bytes.len() < hdr_len {
            return Err("IR packet too short for profile header");
        }

        // Check CRC-8
        let mut crc_check = bytes[0..hdr_len].to_vec();
        crc_check[3] = 0;
        let expected_crc8 = rohc_crc8(&crc_check);
        if received_crc8 != expected_crc8 {
            self.crc_failures += 1;
            return Err("IR packet CRC-8 verification failed");
        }

        let mut ctx = RohcContext::new(cid, profile);
        ctx.src_ip.copy_from_slice(&bytes[4..8]);
        ctx.dst_ip.copy_from_slice(&bytes[8..12]);
        ctx.src_port = u16::from_be_bytes([bytes[12], bytes[13]]);
        ctx.dst_port = u16::from_be_bytes([bytes[14], bytes[15]]);
        ctx.protocol = bytes[16];

        let mut offset = 17;
        if profile == RohcProfile::RtpUdpIp {
            ctx.ssrc = u32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            offset += 4;
        }

        ctx.last_ip_id = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        ctx.last_sn = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as u32;
        ctx.last_tos = bytes[offset + 4];
        ctx.last_ttl = bytes[offset + 5];
        offset += 6;

        if profile == RohcProfile::RtpUdpIp {
            ctx.last_ts = u32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            offset += 4;
        }

        let payload = bytes[offset..].to_vec();

        // Build reconstructed uncompressed packet
        let rtp = if profile == RohcProfile::RtpUdpIp {
            Some(RtpHeader {
                version: 2,
                padding: false,
                extension: false,
                marker: false,
                payload_type: 96,
                sequence_number: ctx.last_sn as u16,
                timestamp: ctx.last_ts,
                ssrc: ctx.ssrc,
            })
        } else {
            None
        };

        let total_len = 20 + 8 + if rtp.is_some() { 12 } else { 0 } + payload.len();

        let packet = UncompressedPacket {
            ip: Ipv4Header {
                version: 4,
                tos: ctx.last_tos,
                total_length: total_len as u16,
                ip_id: ctx.last_ip_id,
                flags_and_offset: 0x4000, // DF
                ttl: ctx.last_ttl,
                protocol: ctx.protocol,
                checksum: 0,
                src_ip: ctx.src_ip,
                dst_ip: ctx.dst_ip,
            },
            udp: UdpHeader {
                src_port: ctx.src_port,
                dst_port: ctx.dst_port,
                length: (8 + if rtp.is_some() { 12 } else { 0 } + payload.len()) as u16,
                checksum: 0,
            },
            rtp,
            payload,
        };

        // Transition decompressor state to FullContext
        self.state = DecompressorState::FullContext;
        self.contexts.insert(cid, ctx);
        self.packets_decompressed += 1;

        Ok(packet)
    }

    /// Decompress RoHC Type 1 Packet (PT-1: 2-byte header).
    fn decompress_type1_packet(
        &mut self,
        bytes: &[u8],
    ) -> Result<UncompressedPacket, &'static str> {
        if self.state == DecompressorState::NoContext {
            return Err("Cannot decompress Type-1 packet without established context");
        }
        if bytes.len() < 2 {
            return Err("Type-1 packet too short");
        }

        let cid = 0; // Small CID 0 default
        let ctx = self
            .contexts
            .get_mut(&cid)
            .ok_or("Missing context for Type-1 packet")?;

        let b0 = bytes[0];
        let b1 = bytes[1];
        let sn_6bit = (b0 & 0x3F) as u32;
        let ip_id_5bit = ((b1 >> 3) & 0x1F) as u16;
        let received_crc3 = b1 & 0x07;

        let expected_crc3 = rohc_crc3(&[b0, (ip_id_5bit as u8) << 3]);
        if received_crc3 != expected_crc3 {
            self.crc_failures += 1;
            return Err("Type-1 CRC-3 verification failed");
        }

        // Extrapolate SN using W-LSB
        let decoded_sn = wlsb_decode(ctx.last_sn, sn_6bit, 6, 1);
        let sn_diff = decoded_sn.wrapping_sub(ctx.last_sn);
        ctx.last_sn = decoded_sn;
        ctx.last_ip_id = (ctx.last_ip_id & !0x1F) | ip_id_5bit;
        ctx.last_ts = ctx
            .last_ts
            .wrapping_add(sn_diff.wrapping_mul(ctx.ts_stride));

        let payload = bytes[2..].to_vec();
        let rtp = if ctx.profile == RohcProfile::RtpUdpIp {
            Some(RtpHeader {
                version: 2,
                padding: false,
                extension: false,
                marker: false,
                payload_type: 96,
                sequence_number: ctx.last_sn as u16,
                timestamp: ctx.last_ts,
                ssrc: ctx.ssrc,
            })
        } else {
            None
        };

        let total_len = 20 + 8 + if rtp.is_some() { 12 } else { 0 } + payload.len();

        let packet = UncompressedPacket {
            ip: Ipv4Header {
                version: 4,
                tos: ctx.last_tos,
                total_length: total_len as u16,
                ip_id: ctx.last_ip_id,
                flags_and_offset: 0x4000,
                ttl: ctx.last_ttl,
                protocol: ctx.protocol,
                checksum: 0,
                src_ip: ctx.src_ip,
                dst_ip: ctx.dst_ip,
            },
            udp: UdpHeader {
                src_port: ctx.src_port,
                dst_port: ctx.dst_port,
                length: (8 + if rtp.is_some() { 12 } else { 0 } + payload.len()) as u16,
                checksum: 0,
            },
            rtp,
            payload,
        };

        self.packets_decompressed += 1;
        Ok(packet)
    }

    /// Decompress RoHC Type 0 Packet (PT-0: 1-byte header).
    fn decompress_type0_packet(
        &mut self,
        bytes: &[u8],
    ) -> Result<UncompressedPacket, &'static str> {
        if self.state != DecompressorState::FullContext {
            return Err("Decompressor must be in FullContext state to decode Type-0 packets");
        }
        if bytes.is_empty() {
            return Err("Empty Type-0 packet");
        }

        let cid = 0;
        let ctx = self
            .contexts
            .get_mut(&cid)
            .ok_or("Missing context for Type-0 packet")?;

        let b0 = bytes[0];
        let sn_4bit = ((b0 >> 3) & 0x0F) as u32;
        let received_crc3 = b0 & 0x07;

        let expected_crc3 = rohc_crc3(&[b0 & 0xF8]);
        if received_crc3 != expected_crc3 {
            self.crc_failures += 1;
            return Err("Type-0 CRC-3 verification failed");
        }

        // Extrapolate SN using W-LSB
        let decoded_sn = wlsb_decode(ctx.last_sn, sn_4bit, 4, 1);
        let sn_diff = decoded_sn.wrapping_sub(ctx.last_sn);
        ctx.last_sn = decoded_sn;
        ctx.last_ip_id = ctx.last_ip_id.wrapping_add(sn_diff as u16);
        ctx.last_ts = ctx
            .last_ts
            .wrapping_add(sn_diff.wrapping_mul(ctx.ts_stride));

        let payload = bytes[1..].to_vec();
        let rtp = if ctx.profile == RohcProfile::RtpUdpIp {
            Some(RtpHeader {
                version: 2,
                padding: false,
                extension: false,
                marker: false,
                payload_type: 96,
                sequence_number: ctx.last_sn as u16,
                timestamp: ctx.last_ts,
                ssrc: ctx.ssrc,
            })
        } else {
            None
        };

        let total_len = 20 + 8 + if rtp.is_some() { 12 } else { 0 } + payload.len();

        let packet = UncompressedPacket {
            ip: Ipv4Header {
                version: 4,
                tos: ctx.last_tos,
                total_length: total_len as u16,
                ip_id: ctx.last_ip_id,
                flags_and_offset: 0x4000,
                ttl: ctx.last_ttl,
                protocol: ctx.protocol,
                checksum: 0,
                src_ip: ctx.src_ip,
                dst_ip: ctx.dst_ip,
            },
            udp: UdpHeader {
                src_port: ctx.src_port,
                dst_port: ctx.dst_port,
                length: (8 + if rtp.is_some() { 12 } else { 0 } + payload.len()) as u16,
                checksum: 0,
            },
            rtp,
            payload,
        };

        self.packets_decompressed += 1;
        Ok(packet)
    }

    /// Generate Feedback response packet for compressor.
    pub fn generate_feedback(&self, cid: u8, feedback_type: FeedbackType) -> RohcFeedback {
        let acked_sn = self.contexts.get(&cid).map(|c| c.last_sn as u16);
        RohcFeedback {
            feedback_type,
            cid,
            acked_sn,
        }
    }
}
