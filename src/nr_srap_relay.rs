//! 3GPP TS 38.351 Release 17 & Release 18 NR Sidelink Relay Adaptation Protocol (SRAP) Engine.
//!
//! Implements the Layer-2 SRAP sublayer located above RLC and below PDCP:
//! - Data PDU encapsulation & parsing with standard 8-bit UE ID, 16-bit extended UE ID,
//!   and transparent/headerless SRB0 format (TS 38.351 §6.2.2).
//! - Rel-18 Multi-hop extension with Hop Count decrement and loop avoidance (TS 38.351 §6.2.4).
//! - Control PDU framing for Flow Control Feedback (CPT=0), Radio Link Failure (CPT=1),
//!   and Multi-hop Routing Echo (CPT=2) (TS 38.351 §6.2.3).
//! - Bearer mapping table between (UE ID, Radio Bearer ID) and PC5 / Uu RLC channels (TS 38.351 §5.3, §5.4).
//! - Relay routing engine supporting Remote UE, Layer-2 UE-to-Network (U2N) Relay UE,
//!   gNodeB network entity, and Intermediate Multi-Hop Relay nodes (Rel-18).
//! - Dynamic flow control & backpressure servo with buffer occupancy watermarking.
//!
//! Pure standard Rust with zero external dependencies.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Constants (3GPP TS 38.351 §6)
// ---------------------------------------------------------------------------

/// Maximum standard 5-bit Radio Bearer ID (SRB1..3, DRB1..29 / 1..32).
pub const SRAP_MAX_BEARER_ID: u8 = 32;

/// Default maximum hop limit for Rel-18 multi-hop relaying to prevent packet looping.
pub const DEFAULT_MAX_HOPS: u8 = 8;
pub const SRAP_DEFAULT_MAX_HOPS: u8 = DEFAULT_MAX_HOPS;

/// Default high watermark buffer threshold in bytes (triggers flow control throttle).
pub const DEFAULT_HIGH_WATERMARK_BYTES: usize = 65536;

/// Default low watermark buffer threshold in bytes (triggers flow control resume).
pub const DEFAULT_LOW_WATERMARK_BYTES: usize = 16384;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered in SRAP processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrapError {
    /// Buffer is shorter than required header size.
    TruncatedBuffer { expected: usize, actual: usize },
    /// Radio Bearer ID exceeds the 5-bit range (1..32).
    InvalidBearerId(u8),
    /// Control PDU Type (CPT) is unknown or unsupported.
    UnsupportedControlPduType(u8),
    /// Ingress or egress bearer mapping not configured.
    MappingNotFound {
        ue_id: u16,
        bearer_id: u8,
        channel_id: u8,
    },
    /// Route for multi-hop destination not found.
    RouteNotFound(u16),
    /// Packet dropped because hop count expired (preventing routing loop).
    HopLimitExceeded { ue_id: u16, hop_count: u8 },
    /// Forwarding loop detected across intermediate relays.
    LoopDetected { ue_id: u16, node_id: u16 },
    /// Queue capacity exceeded in Relay UE buffer.
    BufferOverflow {
        ue_id: u16,
        bearer_id: u8,
        capacity: usize,
    },
    /// Invalid configuration parameter.
    InvalidConfiguration(String),
}

impl fmt::Display for SrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrapError::TruncatedBuffer { expected, actual } => {
                write!(
                    f,
                    "SRAP buffer truncated: expected at least {} bytes, got {}",
                    expected, actual
                )
            }
            SrapError::InvalidBearerId(id) => {
                write!(
                    f,
                    "SRAP Bearer ID {} is invalid (must be between 1 and 32)",
                    id
                )
            }
            SrapError::UnsupportedControlPduType(cpt) => {
                write!(f, "Unsupported SRAP Control PDU Type (CPT): {:#X}", cpt)
            }
            SrapError::MappingNotFound {
                ue_id,
                bearer_id,
                channel_id,
            } => {
                write!(
                    f,
                    "SRAP bearer mapping not found for UE {:#06X}, Bearer {}, RLC Channel {}",
                    ue_id, bearer_id, channel_id
                )
            }
            SrapError::RouteNotFound(ue_id) => {
                write!(
                    f,
                    "SRAP multi-hop route not found for destination UE {:#06X}",
                    ue_id
                )
            }
            SrapError::HopLimitExceeded { ue_id, hop_count } => {
                write!(
                    f,
                    "SRAP hop limit expired for destination UE {:#06X} (hop count: {})",
                    ue_id, hop_count
                )
            }
            SrapError::LoopDetected { ue_id, node_id } => {
                write!(
                    f,
                    "SRAP routing loop detected for UE {:#06X} visiting node {:#06X}",
                    ue_id, node_id
                )
            }
            SrapError::BufferOverflow {
                ue_id,
                bearer_id,
                capacity,
            } => {
                write!(
                    f,
                    "SRAP buffer overflow for UE {:#06X}, Bearer {} (capacity: {} bytes)",
                    ue_id, bearer_id, capacity
                )
            }
            SrapError::InvalidConfiguration(msg) => {
                write!(f, "SRAP configuration error: {}", msg)
            }
        }
    }
}

impl std::error::Error for SrapError {}

// ---------------------------------------------------------------------------
// Protocol Data Unit (PDU) Formats (3GPP TS 38.351 §6)
// ---------------------------------------------------------------------------

/// PDU discriminator: Data PDU vs Control PDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrapPduType {
    ControlPdu = 0,
    DataPdu = 1,
}

/// Control PDU Type (CPT - 3 bits per TS 38.351 §6.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrapControlPduType {
    /// Flow Control Feedback PDU (buffer status / backpressure).
    FlowControlFeedback,
    /// Radio Link Failure notification on PC5 / Uu leg.
    RadioLinkFailureReport,
    /// Multi-hop Route Echo / probe for loop detection (Rel-18).
    MultiHopRouteEcho,
    /// Reserved / vendor-specific CPT.
    Reserved(u8),
}

impl From<u8> for SrapControlPduType {
    fn from(val: u8) -> Self {
        match val & 0x07 {
            0 => SrapControlPduType::FlowControlFeedback,
            1 => SrapControlPduType::RadioLinkFailureReport,
            2 => SrapControlPduType::MultiHopRouteEcho,
            other => SrapControlPduType::Reserved(other),
        }
    }
}

impl From<SrapControlPduType> for u8 {
    fn from(cpt: SrapControlPduType) -> Self {
        match cpt {
            SrapControlPduType::FlowControlFeedback => 0,
            SrapControlPduType::RadioLinkFailureReport => 1,
            SrapControlPduType::MultiHopRouteEcho => 2,
            SrapControlPduType::Reserved(val) => val & 0x07,
        }
    }
}

/// SRAP Data PDU Header fields (TS 38.351 §6.2.2 & Rel-18 §6.2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrapDataHeader {
    /// Radio Bearer ID (5 bits: 1..32).
    pub bearer_id: u8,
    /// Local Remote UE ID (8 bits in standard format, 16 bits in extended format).
    pub ue_id: u16,
    /// Whether 16-bit extended UE ID addressing is used.
    pub is_extended_ue_id: bool,
    /// Optional Rel-18 Multi-hop Hop Count (4 bits: 0..15). Decremented on each hop.
    pub hop_count: Option<u8>,
}

impl SrapDataHeader {
    /// Create standard 8-bit UE ID header.
    pub fn new_standard(ue_id: u8, bearer_id: u8) -> Result<Self, SrapError> {
        if bearer_id == 0 || bearer_id > SRAP_MAX_BEARER_ID {
            return Err(SrapError::InvalidBearerId(bearer_id));
        }
        Ok(Self {
            bearer_id,
            ue_id: ue_id as u16,
            is_extended_ue_id: false,
            hop_count: None,
        })
    }

    /// Create extended 16-bit UE ID header.
    pub fn new_extended(ue_id: u16, bearer_id: u8) -> Result<Self, SrapError> {
        if bearer_id == 0 || bearer_id > SRAP_MAX_BEARER_ID {
            return Err(SrapError::InvalidBearerId(bearer_id));
        }
        Ok(Self {
            bearer_id,
            ue_id,
            is_extended_ue_id: true,
            hop_count: None,
        })
    }

    /// Create Rel-18 multi-hop header with hop count.
    pub fn new_multihop(ue_id: u16, bearer_id: u8, hop_count: u8) -> Result<Self, SrapError> {
        if bearer_id == 0 || bearer_id > SRAP_MAX_BEARER_ID {
            return Err(SrapError::InvalidBearerId(bearer_id));
        }
        Ok(Self {
            bearer_id,
            ue_id,
            is_extended_ue_id: ue_id > 255,
            hop_count: Some(hop_count & 0x0F),
        })
    }

    /// Header size in bytes.
    pub fn byte_len(&self) -> usize {
        let base_len = if self.is_extended_ue_id { 3 } else { 2 };
        if self.hop_count.is_some() {
            base_len + 1
        } else {
            base_len
        }
    }

    /// Serialize header to byte buffer.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.byte_len());
        // Octet 1:
        // Bit 7: D/C = 1 (Data PDU)
        // Bit 6: Extension flag (1 if hop_count present, 0 otherwise)
        // Bit 5: Ext UE ID flag (1 if 16-bit, 0 if 8-bit)
        // Bits 4..0: Bearer ID (5 bits)
        let ext_hop = if self.hop_count.is_some() { 0x40 } else { 0x00 };
        let ext_ue = if self.is_extended_ue_id { 0x20 } else { 0x00 };
        let octet1 = 0x80 | ext_hop | ext_ue | (self.bearer_id & 0x1F);
        buf.push(octet1);

        // UE ID octets:
        if self.is_extended_ue_id {
            buf.push((self.ue_id >> 8) as u8);
            buf.push((self.ue_id & 0xFF) as u8);
        } else {
            buf.push((self.ue_id & 0xFF) as u8);
        }

        // Optional Hop Count octet:
        if let Some(hops) = self.hop_count {
            buf.push(hops & 0x0F);
        }

        buf
    }

    /// Parse header from byte slice.
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), SrapError> {
        if buf.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 2,
                actual: 0,
            });
        }

        let octet1 = buf[0];
        let is_data = (octet1 & 0x80) != 0;
        if !is_data {
            return Err(SrapError::UnsupportedControlPduType(octet1));
        }

        let has_hop = (octet1 & 0x40) != 0;
        let is_extended_ue_id = (octet1 & 0x20) != 0;
        let bearer_id = octet1 & 0x1F;

        let mut offset = 1;
        let ue_id = if is_extended_ue_id {
            if buf.len() < offset + 2 {
                return Err(SrapError::TruncatedBuffer {
                    expected: offset + 2,
                    actual: buf.len(),
                });
            }
            let high = buf[offset] as u16;
            let low = buf[offset + 1] as u16;
            offset += 2;
            (high << 8) | low
        } else {
            if buf.len() < offset + 1 {
                return Err(SrapError::TruncatedBuffer {
                    expected: offset + 1,
                    actual: buf.len(),
                });
            }
            let id = buf[offset] as u16;
            offset += 1;
            id
        };

        let hop_count = if has_hop {
            if buf.len() < offset + 1 {
                return Err(SrapError::TruncatedBuffer {
                    expected: offset + 1,
                    actual: buf.len(),
                });
            }
            let hops = buf[offset] & 0x0F;
            offset += 1;
            Some(hops)
        } else {
            None
        };

        Ok((
            Self {
                bearer_id,
                ue_id,
                is_extended_ue_id,
                hop_count,
            },
            offset,
        ))
    }
}

/// SRAP Data PDU (TS 38.351 §6.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrapDataPdu {
    /// Header if configured (Format 1: with header). None if transparent (Format 2: SRB0).
    pub header: Option<SrapDataHeader>,
    /// SDU payload (e.g. PDCP PDU).
    pub payload: Vec<u8>,
}

impl SrapDataPdu {
    /// Create Data PDU with header.
    pub fn new_with_header(header: SrapDataHeader, payload: Vec<u8>) -> Self {
        Self {
            header: Some(header),
            payload,
        }
    }

    /// Create Data PDU without header (transparent mode for SRB0 per Figure 6.2.2-2).
    pub fn new_transparent(payload: Vec<u8>) -> Self {
        Self {
            header: None,
            payload,
        }
    }

    /// Serialize Data PDU to bytes.
    pub fn encode(&self) -> Vec<u8> {
        match &self.header {
            Some(hdr) => {
                let mut out = hdr.encode();
                out.extend_from_slice(&self.payload);
                out
            }
            None => self.payload.clone(),
        }
    }

    /// Parse Data PDU from bytes.
    pub fn decode(buf: &[u8], has_header: bool) -> Result<Self, SrapError> {
        if has_header {
            let (header, offset) = SrapDataHeader::decode(buf)?;
            let payload = buf[offset..].to_vec();
            Ok(Self {
                header: Some(header),
                payload,
            })
        } else {
            Ok(Self {
                header: None,
                payload: buf.to_vec(),
            })
        }
    }
}

/// SRAP Control PDU (TS 38.351 §6.2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrapControlPdu {
    /// Flow Control Feedback PDU (CPT=0): Informs gNodeB or Relay of buffer status.
    FlowControlFeedback {
        ue_id: u16,
        bearer_id: u8,
        /// Current buffer occupancy in bytes.
        buffer_occupancy_bytes: u32,
        /// Recommended transmission bitrate or credit in kbps.
        credit_window_kbps: u16,
    },
    /// Radio Link Failure Report (CPT=1): Notifies peer of failure on a specific RLC channel.
    RadioLinkFailureReport {
        ue_id: u16,
        failed_rlc_channel_id: u8,
        cause_code: u8,
    },
    /// Rel-18 Multi-Hop Route Echo (CPT=2): Probe packet for topology & loop verification.
    MultiHopRouteEcho {
        originator_ue_id: u16,
        sequence_num: u16,
        hop_distance: u8,
    },
}

impl SrapControlPdu {
    /// Encode Control PDU to byte buffer.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            SrapControlPdu::FlowControlFeedback {
                ue_id,
                bearer_id,
                buffer_occupancy_bytes,
                credit_window_kbps,
            } => {
                // Octet 1: Bit 7=0 (Control PDU), Bits 6..4=000 (CPT=0), Bits 3..0=Reserved (0)
                buf.push(0x00);
                buf.push((ue_id >> 8) as u8);
                buf.push((ue_id & 0xFF) as u8);
                buf.push(bearer_id & 0x1F);
                buf.extend_from_slice(&buffer_occupancy_bytes.to_be_bytes());
                buf.extend_from_slice(&credit_window_kbps.to_be_bytes());
            }
            SrapControlPdu::RadioLinkFailureReport {
                ue_id,
                failed_rlc_channel_id,
                cause_code,
            } => {
                // Octet 1: Bit 7=0 (Control), Bits 6..4=001 (CPT=1)
                buf.push(0x10);
                buf.push((ue_id >> 8) as u8);
                buf.push((ue_id & 0xFF) as u8);
                buf.push(*failed_rlc_channel_id);
                buf.push(*cause_code);
            }
            SrapControlPdu::MultiHopRouteEcho {
                originator_ue_id,
                sequence_num,
                hop_distance,
            } => {
                // Octet 1: Bit 7=0 (Control), Bits 6..4=010 (CPT=2)
                buf.push(0x20);
                buf.push((originator_ue_id >> 8) as u8);
                buf.push((originator_ue_id & 0xFF) as u8);
                buf.extend_from_slice(&sequence_num.to_be_bytes());
                buf.push(*hop_distance);
            }
        }
        buf
    }

    /// Parse Control PDU from byte buffer.
    pub fn decode(buf: &[u8]) -> Result<Self, SrapError> {
        if buf.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }
        let octet1 = buf[0];
        if (octet1 & 0x80) != 0 {
            return Err(SrapError::InvalidConfiguration(
                "Expected Control PDU (D/C=0), found Data PDU (D/C=1)".to_string(),
            ));
        }

        let cpt = (octet1 >> 4) & 0x07;
        match cpt {
            0 => {
                // FlowControlFeedback: 1 (octet1) + 2 (ue_id) + 1 (bearer_id) + 4 (buf) + 2 (credit) = 10 bytes
                if buf.len() < 10 {
                    return Err(SrapError::TruncatedBuffer {
                        expected: 10,
                        actual: buf.len(),
                    });
                }
                let ue_id = ((buf[1] as u16) << 8) | (buf[2] as u16);
                let bearer_id = buf[3] & 0x1F;
                let buffer_occupancy_bytes = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
                let credit_window_kbps = u16::from_be_bytes([buf[8], buf[9]]);
                Ok(SrapControlPdu::FlowControlFeedback {
                    ue_id,
                    bearer_id,
                    buffer_occupancy_bytes,
                    credit_window_kbps,
                })
            }
            1 => {
                // RLF Report: 1 + 2 + 1 + 1 = 5 bytes
                if buf.len() < 5 {
                    return Err(SrapError::TruncatedBuffer {
                        expected: 5,
                        actual: buf.len(),
                    });
                }
                let ue_id = ((buf[1] as u16) << 8) | (buf[2] as u16);
                let failed_rlc_channel_id = buf[3];
                let cause_code = buf[4];
                Ok(SrapControlPdu::RadioLinkFailureReport {
                    ue_id,
                    failed_rlc_channel_id,
                    cause_code,
                })
            }
            2 => {
                // MultiHopRouteEcho: 1 + 2 + 2 + 1 = 6 bytes
                if buf.len() < 6 {
                    return Err(SrapError::TruncatedBuffer {
                        expected: 6,
                        actual: buf.len(),
                    });
                }
                let originator_ue_id = ((buf[1] as u16) << 8) | (buf[2] as u16);
                let sequence_num = u16::from_be_bytes([buf[3], buf[4]]);
                let hop_distance = buf[5];
                Ok(SrapControlPdu::MultiHopRouteEcho {
                    originator_ue_id,
                    sequence_num,
                    hop_distance,
                })
            }
            other => Err(SrapError::UnsupportedControlPduType(other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Bearer Mapping Table (TS 38.351 §5.3, §5.4)
// ---------------------------------------------------------------------------

/// Individual configuration mapping an end-to-end radio bearer to physical/logical RLC channels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrapBearerMapping {
    /// Remote UE Local Identifier.
    pub ue_id: u16,
    /// Radio Bearer ID (SRB1..3, DRB1..32).
    pub bearer_id: u8,
    /// Sidelink (PC5) RLC Channel ID.
    pub sl_rlc_channel_id: u8,
    /// Uu (Relay to gNodeB) RLC Channel ID.
    pub uu_rlc_channel_id: u8,
    /// Whether SRAP header is included over the sidelink interface.
    pub has_srap_header: bool,
}

/// Bearer mapping database configured by RRC for routing and multiplexing.
#[derive(Debug, Clone, Default)]
pub struct SrapBearerMappingTable {
    pub mappings: Vec<SrapBearerMapping>,
}

impl SrapBearerMappingTable {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }

    /// Add or update bearer mapping.
    pub fn add_mapping(&mut self, mapping: SrapBearerMapping) {
        if let Some(pos) = self
            .mappings
            .iter()
            .position(|m| m.ue_id == mapping.ue_id && m.bearer_id == mapping.bearer_id)
        {
            self.mappings[pos] = mapping;
        } else {
            self.mappings.push(mapping);
        }
    }

    /// Remote UE UL lookup: Maps local Bearer ID to egress Sidelink RLC channel.
    pub fn find_sl_for_remote_ul(&self, ue_id: u16, bearer_id: u8) -> Option<&SrapBearerMapping> {
        self.mappings
            .iter()
            .find(|m| m.ue_id == ue_id && m.bearer_id == bearer_id)
    }

    /// Relay UE UL lookup: Maps ingress Sidelink RLC channel and header to egress Uu RLC channel.
    pub fn find_uu_for_relay_ul(&self, sl_channel_id: u8, ue_id: u16, bearer_id: u8) -> Option<u8> {
        self.mappings
            .iter()
            .find(|m| {
                m.sl_rlc_channel_id == sl_channel_id && m.ue_id == ue_id && m.bearer_id == bearer_id
            })
            .map(|m| m.uu_rlc_channel_id)
    }

    /// Relay UE DL lookup: Maps ingress Uu RLC channel and target UE/bearer to egress Sidelink RLC channel.
    pub fn find_sl_for_relay_dl(&self, uu_channel_id: u8, ue_id: u16, bearer_id: u8) -> Option<u8> {
        self.mappings
            .iter()
            .find(|m| {
                m.uu_rlc_channel_id == uu_channel_id && m.ue_id == ue_id && m.bearer_id == bearer_id
            })
            .map(|m| m.sl_rlc_channel_id)
    }

    /// gNodeB DL lookup: Maps Remote UE and Bearer to Uu RLC channel towards Relay UE.
    pub fn find_uu_for_gnb_dl(&self, ue_id: u16, bearer_id: u8) -> Option<u8> {
        self.mappings
            .iter()
            .find(|m| m.ue_id == ue_id && m.bearer_id == bearer_id)
            .map(|m| m.uu_rlc_channel_id)
    }

    /// gNodeB UL lookup: Validates received Uu RLC channel against UE and Bearer.
    pub fn validate_gnb_ul(&self, uu_channel_id: u8, ue_id: u16, bearer_id: u8) -> bool {
        self.mappings.iter().any(|m| {
            m.uu_rlc_channel_id == uu_channel_id && m.ue_id == ue_id && m.bearer_id == bearer_id
        })
    }
}

// ---------------------------------------------------------------------------
// Rel-18 Multi-Hop Router & Loop Prevention (§5.5)
// ---------------------------------------------------------------------------

/// Route entry for Rel-18 Multi-Hop Sidelink Relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrapRouteEntry {
    /// Destination Remote UE or Target Relay ID.
    pub dest_ue_id: u16,
    /// Next-hop Relay node ID.
    pub next_hop_ue_id: u16,
    /// Egress RLC Channel ID towards next hop.
    pub egress_channel_id: u8,
    /// Current estimated hop count distance.
    pub hop_distance: u8,
    /// Metric / link quality cost.
    pub cost_metric: u16,
}

/// Routing table and loop prevention engine for multi-hop topologies.
#[derive(Debug, Clone, Default)]
pub struct SrapMultiHopRouter {
    routes: HashMap<u16, SrapRouteEntry>,
    max_hop_limit: u8,
    visited_node_cache: Vec<(u16, u16)>, // (originator_ue_id, seq)
}

impl SrapMultiHopRouter {
    pub fn new(max_hop_limit: u8) -> Self {
        Self {
            routes: HashMap::new(),
            max_hop_limit,
            visited_node_cache: Vec::new(),
        }
    }

    /// Add or update route towards destination.
    pub fn add_route(&mut self, entry: SrapRouteEntry) {
        self.routes.insert(entry.dest_ue_id, entry);
    }

    /// Lookup next-hop route entry.
    pub fn lookup(&self, dest_ue_id: u16) -> Option<&SrapRouteEntry> {
        self.routes.get(&dest_ue_id)
    }

    /// Process hop count and verify loop avoidance.
    /// Decrements hop count; returns error if hop limit is reached.
    pub fn verify_and_decrement_hops(&mut self, hdr: &mut SrapDataHeader) -> Result<u8, SrapError> {
        let current_hops = match hdr.hop_count {
            Some(h) => h,
            None => self.max_hop_limit,
        };

        if current_hops <= 1 {
            return Err(SrapError::HopLimitExceeded {
                ue_id: hdr.ue_id,
                hop_count: current_hops,
            });
        }

        let new_hops = current_hops - 1;
        hdr.hop_count = Some(new_hops);
        Ok(new_hops)
    }

    /// Check loop detection probe cache.
    pub fn check_and_record_probe(&mut self, orig_ue: u16, seq: u16) -> Result<(), SrapError> {
        if self.visited_node_cache.contains(&(orig_ue, seq)) {
            return Err(SrapError::LoopDetected {
                ue_id: orig_ue,
                node_id: orig_ue,
            });
        }
        if self.visited_node_cache.len() >= 128 {
            self.visited_node_cache.remove(0);
        }
        self.visited_node_cache.push((orig_ue, seq));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Dynamic Flow Control & Backpressure Servo
// ---------------------------------------------------------------------------

/// Per-bearer queue monitoring metrics at Relay UE.
#[derive(Debug, Clone)]
pub struct BearerQueueState {
    pub current_buffer_bytes: usize,
    pub high_watermark_bytes: usize,
    pub low_watermark_bytes: usize,
    pub is_throttled: bool,
    pub recommended_credit_kbps: u16,
}

impl Default for BearerQueueState {
    fn default() -> Self {
        Self {
            current_buffer_bytes: 0,
            high_watermark_bytes: DEFAULT_HIGH_WATERMARK_BYTES,
            low_watermark_bytes: DEFAULT_LOW_WATERMARK_BYTES,
            is_throttled: false,
            recommended_credit_kbps: 1000,
        }
    }
}

/// Dynamic backpressure and flow control manager.
#[derive(Debug, Clone, Default)]
pub struct SrapFlowControlManager {
    queues: HashMap<(u16, u8), BearerQueueState>,
}

impl SrapFlowControlManager {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    /// Configure watermark limits for a specific bearer.
    pub fn configure_limits(
        &mut self,
        ue_id: u16,
        bearer_id: u8,
        high_watermark_bytes: usize,
        low_watermark_bytes: usize,
    ) {
        let entry = self.queues.entry((ue_id, bearer_id)).or_default();
        entry.high_watermark_bytes = high_watermark_bytes;
        entry.low_watermark_bytes = low_watermark_bytes;
    }

    /// Record newly arrived bytes in the buffer.
    /// Returns Some(ControlPdu) if high watermark is crossed and throttling must be signaled.
    pub fn record_enqueue(
        &mut self,
        ue_id: u16,
        bearer_id: u8,
        bytes: usize,
    ) -> Result<Option<SrapControlPdu>, SrapError> {
        let entry = self.queues.entry((ue_id, bearer_id)).or_default();
        entry.current_buffer_bytes += bytes;

        if !entry.is_throttled && entry.current_buffer_bytes >= entry.high_watermark_bytes {
            entry.is_throttled = true;
            entry.recommended_credit_kbps = 0; // Throttle to zero
            return Ok(Some(SrapControlPdu::FlowControlFeedback {
                ue_id,
                bearer_id,
                buffer_occupancy_bytes: entry.current_buffer_bytes as u32,
                credit_window_kbps: 0,
            }));
        }

        Ok(None)
    }

    /// Record successfully forwarded / dequeued bytes.
    /// Returns Some(ControlPdu) if buffer drops below low watermark and resume must be signaled.
    pub fn record_dequeue(
        &mut self,
        ue_id: u16,
        bearer_id: u8,
        bytes: usize,
    ) -> Option<SrapControlPdu> {
        let entry = self.queues.entry((ue_id, bearer_id)).or_default();
        entry.current_buffer_bytes = entry.current_buffer_bytes.saturating_sub(bytes);

        if entry.is_throttled && entry.current_buffer_bytes <= entry.low_watermark_bytes {
            entry.is_throttled = false;
            entry.recommended_credit_kbps = 1000; // Resume credit
            return Some(SrapControlPdu::FlowControlFeedback {
                ue_id,
                bearer_id,
                buffer_occupancy_bytes: entry.current_buffer_bytes as u32,
                credit_window_kbps: 1000,
            });
        }

        None
    }

    /// Check if a bearer is currently throttled.
    pub fn is_throttled(&self, ue_id: u16, bearer_id: u8) -> bool {
        self.queues
            .get(&(ue_id, bearer_id))
            .map(|q| q.is_throttled)
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// SRAP Protocol Entity & Operational Roles
// ---------------------------------------------------------------------------

/// Role of the node in the 5G NR Sidelink Relay Architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrapRole {
    /// Out-of-coverage or edge Remote UE connected via Relay.
    RemoteUe,
    /// Layer-2 UE-to-Network (U2N) Relay UE connecting Remote UE to gNodeB.
    RelayUe,
    /// 5G Core / gNodeB network base station entity.
    GNodeB,
    /// Intermediate Sidelink Relay node in Rel-18 Multi-Hop topology.
    IntermediateRelay,
}

/// Operational metrics and telemetry counters for SRAP.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SrapMetrics {
    pub tx_data_pdus: u64,
    pub rx_data_pdus: u64,
    pub tx_control_pdus: u64,
    pub rx_control_pdus: u64,
    pub relayed_pdus: u64,
    pub bytes_relayed: u64,
    pub dropped_hop_limit: u64,
    pub dropped_unmapped: u64,
    pub flow_control_throttles: u64,
    pub flow_control_resumes: u64,
}

/// The complete SRAP Protocol Entity (TS 38.351).
pub struct SrapEntity {
    pub role: SrapRole,
    pub local_ue_id: u16,
    pub mapping_table: SrapBearerMappingTable,
    pub router: SrapMultiHopRouter,
    pub flow_control: SrapFlowControlManager,
    pub metrics: SrapMetrics,
}

impl SrapEntity {
    /// Create new SRAP entity with specified role and local identifier.
    pub fn new(role: SrapRole, local_ue_id: u16) -> Self {
        Self {
            role,
            local_ue_id,
            mapping_table: SrapBearerMappingTable::new(),
            router: SrapMultiHopRouter::new(DEFAULT_MAX_HOPS),
            flow_control: SrapFlowControlManager::new(),
            metrics: SrapMetrics::default(),
        }
    }

    // -----------------------------------------------------------------------
    // Remote UE Operations
    // -----------------------------------------------------------------------

    /// Remote UE Uplink: Encapsulates SDU from PDCP and outputs (egress_sl_channel_id, pdu_bytes).
    pub fn remote_transmit_ul(
        &mut self,
        bearer_id: u8,
        sdu: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), SrapError> {
        if self.role != SrapRole::RemoteUe {
            return Err(SrapError::InvalidConfiguration(
                "remote_transmit_ul called on non-RemoteUe entity".to_string(),
            ));
        }

        let mapping = self
            .mapping_table
            .find_sl_for_remote_ul(self.local_ue_id, bearer_id)
            .ok_or(SrapError::MappingNotFound {
                ue_id: self.local_ue_id,
                bearer_id,
                channel_id: 0,
            })?;

        let pdu = if mapping.has_srap_header {
            let hdr = SrapDataHeader::new_standard(self.local_ue_id as u8, bearer_id)?;
            SrapDataPdu::new_with_header(hdr, sdu)
        } else {
            SrapDataPdu::new_transparent(sdu)
        };

        let egress_channel = mapping.sl_rlc_channel_id;
        let encoded = pdu.encode();
        self.metrics.tx_data_pdus += 1;
        Ok((egress_channel, encoded))
    }

    /// Remote UE Downlink: Receives PDU from SL RLC channel, validates/strips header,
    /// and delivers (bearer_id, sdu) to local PDCP.
    pub fn remote_receive_dl(
        &mut self,
        sl_channel_id: u8,
        raw_pdu: &[u8],
    ) -> Result<(u8, Vec<u8>), SrapError> {
        if self.role != SrapRole::RemoteUe {
            return Err(SrapError::InvalidConfiguration(
                "remote_receive_dl called on non-RemoteUe entity".to_string(),
            ));
        }

        if raw_pdu.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }

        // Check D/C bit
        let is_control = (raw_pdu[0] & 0x80) == 0;
        if is_control {
            let ctrl = SrapControlPdu::decode(raw_pdu)?;
            self.metrics.rx_control_pdus += 1;
            return self.handle_control_pdu(ctrl);
        }

        // Look up mapping to see if header is present
        let mapping = self
            .mapping_table
            .mappings
            .iter()
            .find(|m| m.ue_id == self.local_ue_id && m.sl_rlc_channel_id == sl_channel_id)
            .ok_or(SrapError::MappingNotFound {
                ue_id: self.local_ue_id,
                bearer_id: 0,
                channel_id: sl_channel_id,
            })?;

        let pdu = SrapDataPdu::decode(raw_pdu, mapping.has_srap_header)?;
        self.metrics.rx_data_pdus += 1;

        let bearer_id = match pdu.header {
            Some(ref hdr) => hdr.bearer_id,
            None => mapping.bearer_id,
        };

        Ok((bearer_id, pdu.payload))
    }

    // -----------------------------------------------------------------------
    // Relay UE Operations (Layer-2 U2N Relay)
    // -----------------------------------------------------------------------

    /// Relay UE Uplink: Receives SRAP PDU on Sidelink RLC channel, extracts target
    /// (ue_id, bearer_id), routes to corresponding Uu RLC channel, and returns
    /// (egress_uu_channel_id, forwarded_pdu_bytes, optional_flow_control_pdu).
    pub fn relay_forward_ul(
        &mut self,
        ingress_sl_channel: u8,
        raw_pdu: &[u8],
    ) -> Result<(u8, Vec<u8>, Option<Vec<u8>>), SrapError> {
        if self.role != SrapRole::RelayUe {
            return Err(SrapError::InvalidConfiguration(
                "relay_forward_ul called on non-RelayUe entity".to_string(),
            ));
        }

        if raw_pdu.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }

        // Control PDU handling
        if (raw_pdu[0] & 0x80) == 0 {
            let ctrl = SrapControlPdu::decode(raw_pdu)?;
            self.metrics.rx_control_pdus += 1;
            let _ = self.handle_control_pdu(ctrl)?;
            return Ok((0, Vec::new(), None));
        }

        // Decode Data PDU (header is always present on relay links unless dedicated 1:1)
        let (hdr, offset) = SrapDataHeader::decode(raw_pdu)?;
        let payload = &raw_pdu[offset..];

        let egress_uu_channel = self
            .mapping_table
            .find_uu_for_relay_ul(ingress_sl_channel, hdr.ue_id, hdr.bearer_id)
            .ok_or(SrapError::MappingNotFound {
                ue_id: hdr.ue_id,
                bearer_id: hdr.bearer_id,
                channel_id: ingress_sl_channel,
            })?;

        // Monitor buffer occupancy & check flow control backpressure
        let flow_ctrl =
            self.flow_control
                .record_enqueue(hdr.ue_id, hdr.bearer_id, payload.len())?;
        let flow_ctrl_bytes = flow_ctrl.map(|c| {
            self.metrics.flow_control_throttles += 1;
            self.metrics.tx_control_pdus += 1;
            c.encode()
        });

        self.metrics.rx_data_pdus += 1;
        self.metrics.relayed_pdus += 1;
        self.metrics.bytes_relayed += raw_pdu.len() as u64;

        Ok((egress_uu_channel, raw_pdu.to_vec(), flow_ctrl_bytes))
    }

    /// Relay UE Downlink: Receives SRAP PDU on Uu RLC channel from gNodeB,
    /// parses Remote UE ID and Bearer ID, resolves egress Sidelink RLC channel,
    /// and forwards to Remote UE.
    pub fn relay_forward_dl(
        &mut self,
        ingress_uu_channel: u8,
        raw_pdu: &[u8],
    ) -> Result<(u8, Vec<u8>), SrapError> {
        if self.role != SrapRole::RelayUe {
            return Err(SrapError::InvalidConfiguration(
                "relay_forward_dl called on non-RelayUe entity".to_string(),
            ));
        }

        if raw_pdu.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }

        if (raw_pdu[0] & 0x80) == 0 {
            let ctrl = SrapControlPdu::decode(raw_pdu)?;
            self.metrics.rx_control_pdus += 1;
            let _ = self.handle_control_pdu(ctrl)?;
            return Ok((0, Vec::new()));
        }

        let (hdr, _offset) = SrapDataHeader::decode(raw_pdu)?;
        let egress_sl_channel = self
            .mapping_table
            .find_sl_for_relay_dl(ingress_uu_channel, hdr.ue_id, hdr.bearer_id)
            .ok_or(SrapError::MappingNotFound {
                ue_id: hdr.ue_id,
                bearer_id: hdr.bearer_id,
                channel_id: ingress_uu_channel,
            })?;

        self.metrics.rx_data_pdus += 1;
        self.metrics.relayed_pdus += 1;
        self.metrics.bytes_relayed += raw_pdu.len() as u64;

        Ok((egress_sl_channel, raw_pdu.to_vec()))
    }

    // -----------------------------------------------------------------------
    // gNodeB Operations
    // -----------------------------------------------------------------------

    /// gNodeB Downlink: Transmits PDCP PDU destined for Remote UE via Relay UE.
    /// Encapsulates SRAP header (Remote UE ID, Bearer ID) and maps to Uu RLC channel.
    pub fn gnb_transmit_dl(
        &mut self,
        target_ue_id: u16,
        bearer_id: u8,
        sdu: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), SrapError> {
        if self.role != SrapRole::GNodeB {
            return Err(SrapError::InvalidConfiguration(
                "gnb_transmit_dl called on non-gNodeB entity".to_string(),
            ));
        }

        let egress_uu_channel = self
            .mapping_table
            .find_uu_for_gnb_dl(target_ue_id, bearer_id)
            .ok_or(SrapError::MappingNotFound {
                ue_id: target_ue_id,
                bearer_id,
                channel_id: 0,
            })?;

        let hdr = SrapDataHeader::new_standard(target_ue_id as u8, bearer_id)?;
        let pdu = SrapDataPdu::new_with_header(hdr, sdu);
        let encoded = pdu.encode();

        self.metrics.tx_data_pdus += 1;
        Ok((egress_uu_channel, encoded))
    }

    /// gNodeB Uplink: Receives SRAP PDU on Uu RLC channel from Relay UE,
    /// strips SRAP header, and demultiplexes to (remote_ue_id, bearer_id, sdu).
    pub fn gnb_receive_ul(
        &mut self,
        ingress_uu_channel: u8,
        raw_pdu: &[u8],
    ) -> Result<(u16, u8, Vec<u8>), SrapError> {
        if self.role != SrapRole::GNodeB {
            return Err(SrapError::InvalidConfiguration(
                "gnb_receive_ul called on non-gNodeB entity".to_string(),
            ));
        }

        if raw_pdu.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }

        if (raw_pdu[0] & 0x80) == 0 {
            let ctrl = SrapControlPdu::decode(raw_pdu)?;
            self.metrics.rx_control_pdus += 1;
            let _ = self.handle_control_pdu(ctrl)?;
            return Ok((0, 0, Vec::new()));
        }

        let (hdr, offset) = SrapDataHeader::decode(raw_pdu)?;
        if !self
            .mapping_table
            .validate_gnb_ul(ingress_uu_channel, hdr.ue_id, hdr.bearer_id)
        {
            self.metrics.dropped_unmapped += 1;
            return Err(SrapError::MappingNotFound {
                ue_id: hdr.ue_id,
                bearer_id: hdr.bearer_id,
                channel_id: ingress_uu_channel,
            });
        }

        self.metrics.rx_data_pdus += 1;
        Ok((hdr.ue_id, hdr.bearer_id, raw_pdu[offset..].to_vec()))
    }

    // -----------------------------------------------------------------------
    // Rel-18 Multi-Hop Intermediate Relay Operations (§5.5)
    // -----------------------------------------------------------------------

    /// Rel-18 Intermediate Relay: Relays SRAP Data PDU across intermediate hops.
    /// Decrements hop count, verifies loop prevention, and resolves next-hop egress channel.
    pub fn intermediate_forward_multihop(
        &mut self,
        _ingress_channel: u8,
        raw_pdu: &[u8],
    ) -> Result<(u8, u16, Vec<u8>), SrapError> {
        if raw_pdu.is_empty() {
            return Err(SrapError::TruncatedBuffer {
                expected: 1,
                actual: 0,
            });
        }

        let (mut hdr, offset) = SrapDataHeader::decode(raw_pdu)?;
        let payload = &raw_pdu[offset..];

        // Decrement hop count
        match self.router.verify_and_decrement_hops(&mut hdr) {
            Ok(_) => {}
            Err(e) => {
                self.metrics.dropped_hop_limit += 1;
                return Err(e);
            }
        }

        let route = self
            .router
            .lookup(hdr.ue_id)
            .ok_or(SrapError::RouteNotFound(hdr.ue_id))?;

        let egress_channel = route.egress_channel_id;
        let next_hop = route.next_hop_ue_id;

        // Re-encode PDU with updated hop count
        let mut out = hdr.encode();
        out.extend_from_slice(payload);

        self.metrics.relayed_pdus += 1;
        self.metrics.bytes_relayed += out.len() as u64;

        Ok((egress_channel, next_hop, out))
    }

    // -----------------------------------------------------------------------
    // Control PDU Handling
    // -----------------------------------------------------------------------

    /// Internal handler for received SRAP Control PDUs.
    fn handle_control_pdu(&mut self, ctrl: SrapControlPdu) -> Result<(u8, Vec<u8>), SrapError> {
        match ctrl {
            SrapControlPdu::FlowControlFeedback {
                ue_id,
                bearer_id,
                buffer_occupancy_bytes,
                credit_window_kbps,
            } => {
                if credit_window_kbps == 0 {
                    self.metrics.flow_control_throttles += 1;
                } else {
                    self.metrics.flow_control_resumes += 1;
                }
                // Return notification: (bearer_id, payload with 4-byte buffer size + 2-byte credit)
                let mut p = Vec::with_capacity(8);
                p.extend_from_slice(&ue_id.to_be_bytes());
                p.extend_from_slice(&buffer_occupancy_bytes.to_be_bytes());
                p.extend_from_slice(&credit_window_kbps.to_be_bytes());
                Ok((bearer_id, p))
            }
            SrapControlPdu::RadioLinkFailureReport {
                ue_id,
                failed_rlc_channel_id,
                cause_code,
            } => {
                let mut p = Vec::with_capacity(4);
                p.extend_from_slice(&ue_id.to_be_bytes());
                p.push(failed_rlc_channel_id);
                p.push(cause_code);
                Ok((0, p))
            }
            SrapControlPdu::MultiHopRouteEcho {
                originator_ue_id,
                sequence_num,
                hop_distance,
            } => {
                self.router
                    .check_and_record_probe(originator_ue_id, sequence_num)?;
                let mut p = Vec::with_capacity(5);
                p.extend_from_slice(&originator_ue_id.to_be_bytes());
                p.extend_from_slice(&sequence_num.to_be_bytes());
                p.push(hop_distance);
                Ok((0, p))
            }
        }
    }
}
