//! eCPRI - enhanced Common Public Radio Interface (eCPRI Specification V2.0).
//!
//! Packet-based 5G fronthaul transport between the eCPRI Radio Equipment Control
//! (eREC, i.e. the O-RAN O-DU) and the eCPRI Radio Equipment (eRE, i.e. the O-RU).
//!
//! Implements the 4-byte eCPRI common header, message concatenation with 4-byte
//! alignment padding, IQ Data (Type 0) and Real-Time Control (Type 2) framing with
//! radio-transport-level subsequence fragmentation (Sequence ID / E-bit / Subsequence ID),
//! Event Indication (Type 7), and the Type 5 One-way Delay Measurement protocol using
//! IEEE 1588 style 10-byte timestamps and a correction-field encoded compensation value.

use std::collections::HashMap;
use std::fmt;

/// EtherType for eCPRI directly over Ethernet (eCPRI Spec V2.0 section 3.1.1).
pub const ECPRI_ETHERTYPE: u16 = 0xAEFE;
/// IANA assigned destination port for eCPRI over UDP/IP.
pub const ECPRI_UDP_PORT: u16 = 5391;
/// Protocol revision carried in the top nibble of the common header.
pub const ECPRI_PROTOCOL_REVISION: u8 = 0x1;
/// Fixed size of the eCPRI common header.
pub const ECPRI_COMMON_HEADER_LEN: usize = 4;
/// Size of the eCPRI timestamp field (48-bit seconds + 32-bit nanoseconds).
pub const ECPRI_TIMESTAMP_LEN: usize = 10;
/// Minimum One-way Delay Measurement payload: ID + action + timestamp + compensation.
pub const ECPRI_OWD_PAYLOAD_MIN_LEN: usize = 20;
/// Concatenated eCPRI messages must start on a 4-byte boundary.
pub const ECPRI_CONCATENATION_ALIGNMENT: usize = 4;

// eCPRI message types (eCPRI Spec V2.0 Table 3).
pub const ECPRI_MSG_IQ_DATA: u8 = 0x00;
pub const ECPRI_MSG_BIT_SEQUENCE: u8 = 0x01;
pub const ECPRI_MSG_RT_CONTROL: u8 = 0x02;
pub const ECPRI_MSG_GENERIC_DATA: u8 = 0x03;
pub const ECPRI_MSG_REMOTE_MEMORY: u8 = 0x04;
pub const ECPRI_MSG_DELAY_MEASUREMENT: u8 = 0x05;
pub const ECPRI_MSG_REMOTE_RESET: u8 = 0x06;
pub const ECPRI_MSG_EVENT_INDICATION: u8 = 0x07;
pub const ECPRI_MSG_IWF_STARTUP: u8 = 0x08;
pub const ECPRI_MSG_IWF_OPERATION: u8 = 0x09;
pub const ECPRI_MSG_IWF_MAPPING: u8 = 0x0A;
pub const ECPRI_MSG_IWF_DELAY_CONTROL: u8 = 0x0B;
/// Message types at or above this value are reserved for vendor specific use.
pub const ECPRI_MSG_VENDOR_SPECIFIC_BASE: u8 = 0x40;

/// Classification of an eCPRI message type code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcpriMessageType {
    /// Type 0: user plane IQ samples for one physical channel / antenna-carrier.
    IqData,
    /// Type 1: transparent bit sequence transfer.
    BitSequence,
    /// Type 2: real-time control data (beamforming, scheduling commands).
    RealTimeControl,
    /// Type 3: generic (non real-time) data transfer.
    GenericDataTransfer,
    /// Type 4: remote memory read / write access.
    RemoteMemoryAccess,
    /// Type 5: one-way delay measurement.
    OneWayDelayMeasurement,
    /// Type 6: remote reset request / response.
    RemoteReset,
    /// Type 7: event indication (fault / notification reporting).
    EventIndication,
    /// Types 8..0x0B: CPRI interworking function control.
    Iwf(u8),
    /// Types 0x0C..0x3F: reserved by the specification.
    Reserved(u8),
    /// Types 0x40..0xFF: vendor specific.
    VendorSpecific(u8),
}

impl EcpriMessageType {
    pub fn from_u8(code: u8) -> Self {
        match code {
            ECPRI_MSG_IQ_DATA => EcpriMessageType::IqData,
            ECPRI_MSG_BIT_SEQUENCE => EcpriMessageType::BitSequence,
            ECPRI_MSG_RT_CONTROL => EcpriMessageType::RealTimeControl,
            ECPRI_MSG_GENERIC_DATA => EcpriMessageType::GenericDataTransfer,
            ECPRI_MSG_REMOTE_MEMORY => EcpriMessageType::RemoteMemoryAccess,
            ECPRI_MSG_DELAY_MEASUREMENT => EcpriMessageType::OneWayDelayMeasurement,
            ECPRI_MSG_REMOTE_RESET => EcpriMessageType::RemoteReset,
            ECPRI_MSG_EVENT_INDICATION => EcpriMessageType::EventIndication,
            ECPRI_MSG_IWF_STARTUP..=ECPRI_MSG_IWF_DELAY_CONTROL => EcpriMessageType::Iwf(code),
            c if c >= ECPRI_MSG_VENDOR_SPECIFIC_BASE => EcpriMessageType::VendorSpecific(c),
            c => EcpriMessageType::Reserved(c),
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            EcpriMessageType::IqData => ECPRI_MSG_IQ_DATA,
            EcpriMessageType::BitSequence => ECPRI_MSG_BIT_SEQUENCE,
            EcpriMessageType::RealTimeControl => ECPRI_MSG_RT_CONTROL,
            EcpriMessageType::GenericDataTransfer => ECPRI_MSG_GENERIC_DATA,
            EcpriMessageType::RemoteMemoryAccess => ECPRI_MSG_REMOTE_MEMORY,
            EcpriMessageType::OneWayDelayMeasurement => ECPRI_MSG_DELAY_MEASUREMENT,
            EcpriMessageType::RemoteReset => ECPRI_MSG_REMOTE_RESET,
            EcpriMessageType::EventIndication => ECPRI_MSG_EVENT_INDICATION,
            EcpriMessageType::Iwf(c)
            | EcpriMessageType::Reserved(c)
            | EcpriMessageType::VendorSpecific(c) => *c,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            EcpriMessageType::IqData => "IQ Data",
            EcpriMessageType::BitSequence => "Bit Sequence",
            EcpriMessageType::RealTimeControl => "Real-Time Control Data",
            EcpriMessageType::GenericDataTransfer => "Generic Data Transfer",
            EcpriMessageType::RemoteMemoryAccess => "Remote Memory Access",
            EcpriMessageType::OneWayDelayMeasurement => "One-way Delay Measurement",
            EcpriMessageType::RemoteReset => "Remote Reset",
            EcpriMessageType::EventIndication => "Event Indication",
            EcpriMessageType::Iwf(_) => "IWF Control",
            EcpriMessageType::Reserved(_) => "Reserved",
            EcpriMessageType::VendorSpecific(_) => "Vendor Specific",
        }
    }

    /// User plane and real-time control traffic must never be delayed by a bridge queue.
    pub fn is_time_critical(&self) -> bool {
        matches!(
            self,
            EcpriMessageType::IqData
                | EcpriMessageType::RealTimeControl
                | EcpriMessageType::OneWayDelayMeasurement
        )
    }
}

/// Errors raised while decoding an eCPRI PDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcpriError {
    /// Fewer than the 4 bytes of the common header are present.
    HeaderTooShort(usize),
    /// The protocol revision nibble is not the supported revision 1.
    UnsupportedRevision(u8),
    /// `ecpriPayloadSize` claims more bytes than the buffer holds.
    PayloadTruncated { declared: usize, available: usize },
    /// The payload is shorter than the mandatory fields of its message type.
    PayloadTooShort {
        message_type: u8,
        need: usize,
        got: usize,
    },
    /// The payload exceeds the 16-bit `ecpriPayloadSize` field.
    PayloadTooLarge(usize),
    /// A One-way Delay Measurement action type outside 0x00..=0x05.
    UnsupportedActionType(u8),
    /// Nanoseconds field of a timestamp is not normalized below one second.
    InvalidTimestamp(u32),
    /// The C bit announced a following message but the buffer ended.
    MisalignedConcatenation(usize),
    /// A non delay-measurement message was fed to the delay measurement engine.
    NotADelayMeasurement(u8),
}

impl fmt::Display for EcpriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EcpriError::HeaderTooShort(l) => {
                write!(f, "eCPRI common header too short ({} bytes)", l)
            }
            EcpriError::UnsupportedRevision(r) => write!(f, "Unsupported eCPRI revision {}", r),
            EcpriError::PayloadTruncated {
                declared,
                available,
            } => write!(
                f,
                "eCPRI payload truncated: declared {} bytes, {} available",
                declared, available
            ),
            EcpriError::PayloadTooShort {
                message_type,
                need,
                got,
            } => write!(
                f,
                "eCPRI message type 0x{:02X} needs {} payload bytes, got {}",
                message_type, need, got
            ),
            EcpriError::PayloadTooLarge(l) => {
                write!(f, "eCPRI payload of {} bytes exceeds 65535", l)
            }
            EcpriError::UnsupportedActionType(a) => write!(
                f,
                "Unsupported eCPRI delay measurement action type 0x{:02X}",
                a
            ),
            EcpriError::InvalidTimestamp(ns) => {
                write!(f, "eCPRI timestamp nanoseconds field out of range ({})", ns)
            }
            EcpriError::MisalignedConcatenation(o) => write!(
                f,
                "eCPRI concatenation bit set but no message at offset {}",
                o
            ),
            EcpriError::NotADelayMeasurement(t) => write!(
                f,
                "eCPRI message type 0x{:02X} is not a One-way Delay Measurement",
                t
            ),
        }
    }
}

impl std::error::Error for EcpriError {}

/// eCPRI common header (4 bytes) prefixing every eCPRI message.
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-------+-----+-+---------------+-------------------------------+
/// |  Rev  | Rsv |C|  Message Type |          Payload Size         |
/// +-------+-----+-+---------------+-------------------------------+
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcpriCommonHeader {
    pub protocol_revision: u8,
    /// C bit: another eCPRI message follows within the same transport PDU.
    pub concatenated: bool,
    pub message_type: u8,
    /// Payload bytes following this header, excluding concatenation padding.
    pub payload_size: u16,
}

impl EcpriCommonHeader {
    pub fn new(message_type: u8, payload_size: u16) -> Self {
        EcpriCommonHeader {
            protocol_revision: ECPRI_PROTOCOL_REVISION,
            concatenated: false,
            message_type,
            payload_size,
        }
    }

    pub fn serialize(&self) -> [u8; ECPRI_COMMON_HEADER_LEN] {
        let mut out = [0u8; ECPRI_COMMON_HEADER_LEN];
        out[0] = (self.protocol_revision << 4) | u8::from(self.concatenated);
        out[1] = self.message_type;
        out[2..4].copy_from_slice(&self.payload_size.to_be_bytes());
        out
    }

    pub fn parse(data: &[u8]) -> Result<Self, EcpriError> {
        if data.len() < ECPRI_COMMON_HEADER_LEN {
            return Err(EcpriError::HeaderTooShort(data.len()));
        }
        let protocol_revision = data[0] >> 4;
        if protocol_revision != ECPRI_PROTOCOL_REVISION {
            return Err(EcpriError::UnsupportedRevision(protocol_revision));
        }
        Ok(EcpriCommonHeader {
            protocol_revision,
            // Bits 3..1 are reserved and ignored on reception.
            concatenated: data[0] & 0x01 != 0,
            message_type: data[1],
            payload_size: u16::from_be_bytes([data[2], data[3]]),
        })
    }

    /// Total on-wire size of this message once padded for concatenation.
    pub fn aligned_len(&self) -> usize {
        let raw = ECPRI_COMMON_HEADER_LEN + self.payload_size as usize;
        raw.div_ceil(ECPRI_CONCATENATION_ALIGNMENT) * ECPRI_CONCATENATION_ALIGNMENT
    }
}

/// 10-byte eCPRI timestamp: 48-bit seconds plus 32-bit nanoseconds (IEEE 1588 format).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EcpriTimestamp {
    pub seconds: u64,
    pub nanoseconds: u32,
}

impl EcpriTimestamp {
    pub fn new(seconds: u64, nanoseconds: u32) -> Self {
        EcpriTimestamp {
            seconds,
            nanoseconds,
        }
    }

    /// All-zero timestamp, transmitted by Request-with-Follow_Up and Remote Request actions.
    pub fn zero() -> Self {
        EcpriTimestamp::default()
    }

    pub fn is_zero(&self) -> bool {
        self.seconds == 0 && self.nanoseconds == 0
    }

    pub fn to_total_nanoseconds(&self) -> i128 {
        (self.seconds as i128) * 1_000_000_000 + (self.nanoseconds as i128)
    }

    pub fn from_total_nanoseconds(total_ns: i128) -> Self {
        let clamped = total_ns.max(0);
        EcpriTimestamp {
            seconds: (clamped / 1_000_000_000) as u64,
            nanoseconds: (clamped % 1_000_000_000) as u32,
        }
    }

    pub fn serialize(&self) -> [u8; ECPRI_TIMESTAMP_LEN] {
        let mut out = [0u8; ECPRI_TIMESTAMP_LEN];
        let secs = self.seconds.to_be_bytes();
        // Only the low 48 bits of the seconds counter travel on the wire.
        out[0..6].copy_from_slice(&secs[2..8]);
        out[6..10].copy_from_slice(&self.nanoseconds.to_be_bytes());
        out
    }

    pub fn parse(data: &[u8]) -> Result<Self, EcpriError> {
        if data.len() < ECPRI_TIMESTAMP_LEN {
            return Err(EcpriError::PayloadTooShort {
                message_type: ECPRI_MSG_DELAY_MEASUREMENT,
                need: ECPRI_TIMESTAMP_LEN,
                got: data.len(),
            });
        }
        let mut secs = [0u8; 8];
        secs[2..8].copy_from_slice(&data[0..6]);
        let nanoseconds = u32::from_be_bytes([data[6], data[7], data[8], data[9]]);
        if nanoseconds >= 1_000_000_000 {
            return Err(EcpriError::InvalidTimestamp(nanoseconds));
        }
        Ok(EcpriTimestamp {
            seconds: u64::from_be_bytes(secs),
            nanoseconds,
        })
    }
}

/// 2-byte SEQ_ID field used by IQ Data and Real-Time Control messages.
///
/// The first byte counts messages of one flow; the second byte carries the E-bit
/// and a 7-bit subsequence counter used for radio-transport-level fragmentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcpriSeqId {
    pub sequence_id: u8,
    /// E bit: set on the last fragment of a subsequence (or when no fragmentation is used).
    pub last_subsequence: bool,
    /// 7-bit subsequence counter, 0 for the first fragment.
    pub subsequence_id: u8,
}

impl EcpriSeqId {
    /// Unfragmented message: a single fragment with the E bit already set.
    pub fn single(sequence_id: u8) -> Self {
        EcpriSeqId {
            sequence_id,
            last_subsequence: true,
            subsequence_id: 0,
        }
    }

    pub fn fragment(sequence_id: u8, subsequence_id: u8, last: bool) -> Self {
        EcpriSeqId {
            sequence_id,
            last_subsequence: last,
            subsequence_id: subsequence_id & 0x7F,
        }
    }

    pub fn serialize(&self) -> [u8; 2] {
        [
            self.sequence_id,
            (u8::from(self.last_subsequence) << 7) | (self.subsequence_id & 0x7F),
        ]
    }

    pub fn parse(bytes: [u8; 2]) -> Self {
        EcpriSeqId {
            sequence_id: bytes[0],
            last_subsequence: bytes[1] & 0x80 != 0,
            subsequence_id: bytes[1] & 0x7F,
        }
    }
}

/// Action types of the Type 5 One-way Delay Measurement message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcpriDelayAction {
    /// 0x00: carries t1 of the sender in the same message.
    Request,
    /// 0x01: t1 is deferred to a following Follow_Up message.
    RequestWithFollowUp,
    /// 0x02: answer to a Remote Request, carrying the responder's own t1.
    Response,
    /// 0x03: asks the peer to start a measurement in the reverse direction.
    RemoteRequest,
    /// 0x04: asks the peer to start a two-message measurement in the reverse direction.
    RemoteRequestWithFollowUp,
    /// 0x05: supplies the precise t1 of a preceding Request with Follow_Up.
    FollowUp,
}

impl EcpriDelayAction {
    pub fn from_u8(code: u8) -> Result<Self, EcpriError> {
        match code {
            0x00 => Ok(EcpriDelayAction::Request),
            0x01 => Ok(EcpriDelayAction::RequestWithFollowUp),
            0x02 => Ok(EcpriDelayAction::Response),
            0x03 => Ok(EcpriDelayAction::RemoteRequest),
            0x04 => Ok(EcpriDelayAction::RemoteRequestWithFollowUp),
            0x05 => Ok(EcpriDelayAction::FollowUp),
            other => Err(EcpriError::UnsupportedActionType(other)),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            EcpriDelayAction::Request => 0x00,
            EcpriDelayAction::RequestWithFollowUp => 0x01,
            EcpriDelayAction::Response => 0x02,
            EcpriDelayAction::RemoteRequest => 0x03,
            EcpriDelayAction::RemoteRequestWithFollowUp => 0x04,
            EcpriDelayAction::FollowUp => 0x05,
        }
    }

    /// Whether the timestamp and compensation fields hold a meaningful value.
    ///
    /// Request-with-Follow_Up and both Remote Request actions transmit zeroed fields.
    pub fn carries_timestamp(self) -> bool {
        matches!(
            self,
            EcpriDelayAction::Request | EcpriDelayAction::Response | EcpriDelayAction::FollowUp
        )
    }

    /// Remote Request actions ask the receiver to originate the measurement instead.
    pub fn is_remote_request(self) -> bool {
        matches!(
            self,
            EcpriDelayAction::RemoteRequest | EcpriDelayAction::RemoteRequestWithFollowUp
        )
    }
}

/// Type 5 One-way Delay Measurement payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcpriDelayMeasurement {
    pub measurement_id: u8,
    pub action: EcpriDelayAction,
    pub timestamp: EcpriTimestamp,
    /// Compensation value in the IEEE 1588 correctionField encoding: nanoseconds x 2^16.
    pub compensation_value: u64,
    /// Trailing dummy bytes, used to inflate the frame to a specific test size.
    pub dummy_bytes: usize,
}

impl EcpriDelayMeasurement {
    /// Builds a measurement payload, zeroing the timestamp and compensation
    /// fields for the action types that must transmit them as zero.
    pub fn new(
        measurement_id: u8,
        action: EcpriDelayAction,
        timestamp: EcpriTimestamp,
        compensation_ns: u64,
    ) -> Self {
        let carries = action.carries_timestamp();
        EcpriDelayMeasurement {
            measurement_id,
            action,
            timestamp: if carries {
                timestamp
            } else {
                EcpriTimestamp::zero()
            },
            compensation_value: if carries {
                Self::ns_to_compensation(compensation_ns)
            } else {
                0
            },
            dummy_bytes: 0,
        }
    }

    /// Encodes whole nanoseconds into the correctionField style fixed-point field.
    pub fn ns_to_compensation(ns: u64) -> u64 {
        ns << 16
    }

    /// Compensation value truncated to whole nanoseconds.
    pub fn compensation_ns(&self) -> u64 {
        self.compensation_value >> 16
    }

    /// Pads the message with dummy bytes so the eCPRI PDU reaches `total_payload` bytes.
    pub fn with_dummy_payload(mut self, total_payload: usize) -> Self {
        self.dummy_bytes = total_payload.saturating_sub(ECPRI_OWD_PAYLOAD_MIN_LEN);
        self
    }
}

/// Decoded eCPRI message payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcpriMessage {
    /// Type 0: PC_ID + SEQ_ID + IQ samples.
    IqData {
        pc_id: u16,
        seq_id: EcpriSeqId,
        samples: Vec<u8>,
    },
    /// Type 2: RTC_ID + SEQ_ID + real-time control data.
    RealTimeControl {
        rtc_id: u16,
        seq_id: EcpriSeqId,
        data: Vec<u8>,
    },
    /// Type 5: one-way delay measurement.
    DelayMeasurement(EcpriDelayMeasurement),
    /// Type 7: event / fault indication header plus raw elements.
    EventIndication {
        event_id: u8,
        event_type: u8,
        sequence_number: u8,
        element_count: u8,
        elements: Vec<u8>,
    },
    /// Any other message type, kept as an opaque payload.
    Raw { message_type: u8, payload: Vec<u8> },
}

impl EcpriMessage {
    pub fn message_type(&self) -> u8 {
        match self {
            EcpriMessage::IqData { .. } => ECPRI_MSG_IQ_DATA,
            EcpriMessage::RealTimeControl { .. } => ECPRI_MSG_RT_CONTROL,
            EcpriMessage::DelayMeasurement(_) => ECPRI_MSG_DELAY_MEASUREMENT,
            EcpriMessage::EventIndication { .. } => ECPRI_MSG_EVENT_INDICATION,
            EcpriMessage::Raw { message_type, .. } => *message_type,
        }
    }

    pub fn serialize_payload(&self) -> Vec<u8> {
        match self {
            EcpriMessage::IqData {
                pc_id,
                seq_id,
                samples,
            } => {
                let mut out = Vec::with_capacity(4 + samples.len());
                out.extend_from_slice(&pc_id.to_be_bytes());
                out.extend_from_slice(&seq_id.serialize());
                out.extend_from_slice(samples);
                out
            }
            EcpriMessage::RealTimeControl {
                rtc_id,
                seq_id,
                data,
            } => {
                let mut out = Vec::with_capacity(4 + data.len());
                out.extend_from_slice(&rtc_id.to_be_bytes());
                out.extend_from_slice(&seq_id.serialize());
                out.extend_from_slice(data);
                out
            }
            EcpriMessage::DelayMeasurement(dm) => {
                let mut out = Vec::with_capacity(ECPRI_OWD_PAYLOAD_MIN_LEN + dm.dummy_bytes);
                out.push(dm.measurement_id);
                out.push(dm.action.to_u8());
                out.extend_from_slice(&dm.timestamp.serialize());
                out.extend_from_slice(&dm.compensation_value.to_be_bytes());
                out.resize(ECPRI_OWD_PAYLOAD_MIN_LEN + dm.dummy_bytes, 0);
                out
            }
            EcpriMessage::EventIndication {
                event_id,
                event_type,
                sequence_number,
                element_count,
                elements,
            } => {
                let mut out = Vec::with_capacity(4 + elements.len());
                out.push(*event_id);
                out.push(*event_type);
                out.push(*sequence_number);
                out.push(*element_count);
                out.extend_from_slice(elements);
                out
            }
            EcpriMessage::Raw { payload, .. } => payload.clone(),
        }
    }

    pub fn parse_payload(message_type: u8, payload: &[u8]) -> Result<Self, EcpriError> {
        match message_type {
            ECPRI_MSG_IQ_DATA | ECPRI_MSG_RT_CONTROL => {
                if payload.len() < 4 {
                    return Err(EcpriError::PayloadTooShort {
                        message_type,
                        need: 4,
                        got: payload.len(),
                    });
                }
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let seq_id = EcpriSeqId::parse([payload[2], payload[3]]);
                let body = payload[4..].to_vec();
                if message_type == ECPRI_MSG_IQ_DATA {
                    Ok(EcpriMessage::IqData {
                        pc_id: id,
                        seq_id,
                        samples: body,
                    })
                } else {
                    Ok(EcpriMessage::RealTimeControl {
                        rtc_id: id,
                        seq_id,
                        data: body,
                    })
                }
            }
            ECPRI_MSG_DELAY_MEASUREMENT => {
                if payload.len() < ECPRI_OWD_PAYLOAD_MIN_LEN {
                    return Err(EcpriError::PayloadTooShort {
                        message_type,
                        need: ECPRI_OWD_PAYLOAD_MIN_LEN,
                        got: payload.len(),
                    });
                }
                let action = EcpriDelayAction::from_u8(payload[1])?;
                let timestamp = EcpriTimestamp::parse(&payload[2..12])?;
                let mut comp = [0u8; 8];
                comp.copy_from_slice(&payload[12..20]);
                Ok(EcpriMessage::DelayMeasurement(EcpriDelayMeasurement {
                    measurement_id: payload[0],
                    action,
                    timestamp,
                    compensation_value: u64::from_be_bytes(comp),
                    dummy_bytes: payload.len() - ECPRI_OWD_PAYLOAD_MIN_LEN,
                }))
            }
            ECPRI_MSG_EVENT_INDICATION => {
                if payload.len() < 4 {
                    return Err(EcpriError::PayloadTooShort {
                        message_type,
                        need: 4,
                        got: payload.len(),
                    });
                }
                Ok(EcpriMessage::EventIndication {
                    event_id: payload[0],
                    event_type: payload[1],
                    sequence_number: payload[2],
                    element_count: payload[3],
                    elements: payload[4..].to_vec(),
                })
            }
            other => Ok(EcpriMessage::Raw {
                message_type: other,
                payload: payload.to_vec(),
            }),
        }
    }
}

/// A single eCPRI message: common header plus decoded payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcpriPacket {
    pub header: EcpriCommonHeader,
    pub message: EcpriMessage,
}

impl EcpriPacket {
    /// Builds a standalone message, deriving `ecpriPayloadSize` from the payload.
    pub fn new(message: EcpriMessage) -> Self {
        let payload_size = message.serialize_payload().len() as u16;
        EcpriPacket {
            header: EcpriCommonHeader::new(message.message_type(), payload_size),
            message,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let payload = self.message.serialize_payload();
        let mut header = self.header;
        header.payload_size = payload.len() as u16;
        let mut out = Vec::with_capacity(ECPRI_COMMON_HEADER_LEN + payload.len());
        out.extend_from_slice(&header.serialize());
        out.extend_from_slice(&payload);
        out
    }

    /// Parses the first eCPRI message in `data`.
    ///
    /// Bytes past `ecpriPayloadSize` are left untouched: they are either Ethernet
    /// padding or, when the C bit is set, the next concatenated message.
    pub fn parse(data: &[u8]) -> Result<Self, EcpriError> {
        let header = EcpriCommonHeader::parse(data)?;
        let declared = header.payload_size as usize;
        let available = data.len() - ECPRI_COMMON_HEADER_LEN;
        if available < declared {
            return Err(EcpriError::PayloadTruncated {
                declared,
                available,
            });
        }
        let payload = &data[ECPRI_COMMON_HEADER_LEN..ECPRI_COMMON_HEADER_LEN + declared];
        let message = EcpriMessage::parse_payload(header.message_type, payload)?;
        Ok(EcpriPacket { header, message })
    }

    /// Concatenates messages into one transport PDU.
    ///
    /// Every message but the last sets the C bit and is zero-padded so that the
    /// following common header starts on a 4-byte boundary.
    pub fn serialize_concatenated(messages: &[EcpriMessage]) -> Result<Vec<u8>, EcpriError> {
        let mut out = Vec::new();
        for (idx, message) in messages.iter().enumerate() {
            let payload = message.serialize_payload();
            if payload.len() > u16::MAX as usize {
                return Err(EcpriError::PayloadTooLarge(payload.len()));
            }
            let last = idx + 1 == messages.len();
            let header = EcpriCommonHeader {
                protocol_revision: ECPRI_PROTOCOL_REVISION,
                concatenated: !last,
                message_type: message.message_type(),
                payload_size: payload.len() as u16,
            };
            out.extend_from_slice(&header.serialize());
            out.extend_from_slice(&payload);
            if !last {
                let pad = header.aligned_len() - (ECPRI_COMMON_HEADER_LEN + payload.len());
                out.resize(out.len() + pad, 0);
            }
        }
        Ok(out)
    }

    /// Walks a concatenated PDU, following the C bit and skipping alignment padding.
    pub fn parse_concatenated(data: &[u8]) -> Result<Vec<EcpriPacket>, EcpriError> {
        let mut packets = Vec::new();
        let mut offset = 0usize;
        loop {
            if offset >= data.len() {
                return Err(EcpriError::MisalignedConcatenation(offset));
            }
            let packet = EcpriPacket::parse(&data[offset..])?;
            let more = packet.header.concatenated;
            offset += packet.header.aligned_len();
            packets.push(packet);
            if !more {
                break;
            }
        }
        Ok(packets)
    }
}

/// One-way delay from a transmit timestamp, a receive timestamp and a compensation value.
///
/// eCPRI Spec V2.0 section 3.2.4.6: `t_D = (t_2 - t_1) - t_compensation`.
pub fn one_way_delay_ns(t1_ns: i128, t2_ns: i128, compensation_ns: i128) -> i128 {
    (t2_ns - t1_ns) - compensation_ns
}

/// Residual asymmetry of a link measured in both directions.
///
/// A positive result means the forward path is the slower one; half of the
/// difference is the correction a synchronizing clock has to apply.
pub fn estimate_link_asymmetry_ns(forward_delay_ns: i128, reverse_delay_ns: i128) -> i128 {
    (forward_delay_ns - reverse_delay_ns) / 2
}

/// Completed one-way delay measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwdMeasurementResult {
    pub measurement_id: u8,
    pub t1_ns: i128,
    pub t2_ns: i128,
    pub compensation_ns: i128,
    pub one_way_delay_ns: i128,
    /// True when t1 arrived in a separate Follow_Up message (two-step measurement).
    pub two_step: bool,
}

/// Outcome of feeding a received Type 5 message to [`EcpriOwdEngine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwdEvent {
    /// The delay was computed and appended to the engine's history.
    Completed(OwdMeasurementResult),
    /// A Request with Follow_Up arrived; t2 is held until its Follow_Up lands.
    AwaitingFollowUp { measurement_id: u8 },
    /// The peer asked this node to originate a measurement in the reverse direction.
    RemoteRequest {
        measurement_id: u8,
        expects_follow_up: bool,
    },
    /// A Follow_Up with no matching pending Request; discarded.
    OrphanFollowUp { measurement_id: u8 },
}

/// eCPRI Type 5 One-way Delay Measurement state machine for one node.
#[derive(Debug, Clone)]
pub struct EcpriOwdEngine {
    pub node_id: String,
    /// Transmit-path delay this node compensates for in the messages it originates.
    pub tx_compensation_ns: u64,
    pending: HashMap<u8, i128>,
    next_measurement_id: u8,
    pub completed: Vec<OwdMeasurementResult>,
    pub orphan_follow_ups: u64,
}

impl EcpriOwdEngine {
    pub fn new(node_id: &str, tx_compensation_ns: u64) -> Self {
        EcpriOwdEngine {
            node_id: node_id.to_string(),
            tx_compensation_ns,
            pending: HashMap::new(),
            next_measurement_id: 1,
            completed: Vec::new(),
            orphan_follow_ups: 0,
        }
    }

    /// Measurement IDs cycle through 1..=255; 0 is reserved as "unused".
    fn allocate_measurement_id(&mut self) -> u8 {
        let id = self.next_measurement_id;
        self.next_measurement_id = self.next_measurement_id.wrapping_add(1);
        if self.next_measurement_id == 0 {
            self.next_measurement_id = 1;
        }
        id
    }

    /// One-step Request carrying t1 in the same message.
    pub fn build_request(&mut self, t1_ns: i128) -> EcpriPacket {
        let id = self.allocate_measurement_id();
        self.build_with_id(id, EcpriDelayAction::Request, t1_ns)
    }

    /// Two-step Request; the precise t1 follows in [`EcpriOwdEngine::build_follow_up`].
    pub fn build_request_with_follow_up(&mut self) -> EcpriPacket {
        let id = self.allocate_measurement_id();
        self.build_with_id(id, EcpriDelayAction::RequestWithFollowUp, 0)
    }

    pub fn build_follow_up(&self, measurement_id: u8, t1_ns: i128) -> EcpriPacket {
        self.build_with_id(measurement_id, EcpriDelayAction::FollowUp, t1_ns)
    }

    /// Response to a Remote Request, carrying this node's own t1.
    pub fn build_response(&self, measurement_id: u8, t1_ns: i128) -> EcpriPacket {
        self.build_with_id(measurement_id, EcpriDelayAction::Response, t1_ns)
    }

    /// Asks the peer to measure the reverse direction.
    pub fn build_remote_request(&mut self, expects_follow_up: bool) -> EcpriPacket {
        let id = self.allocate_measurement_id();
        let action = if expects_follow_up {
            EcpriDelayAction::RemoteRequestWithFollowUp
        } else {
            EcpriDelayAction::RemoteRequest
        };
        self.build_with_id(id, action, 0)
    }

    fn build_with_id(&self, id: u8, action: EcpriDelayAction, t1_ns: i128) -> EcpriPacket {
        let dm = EcpriDelayMeasurement::new(
            id,
            action,
            EcpriTimestamp::from_total_nanoseconds(t1_ns),
            self.tx_compensation_ns,
        );
        EcpriPacket::new(EcpriMessage::DelayMeasurement(dm))
    }

    /// Feeds a received Type 5 message together with the local receive timestamp t2.
    pub fn on_receive(
        &mut self,
        packet: &EcpriPacket,
        t2_ns: i128,
    ) -> Result<OwdEvent, EcpriError> {
        let dm = match &packet.message {
            EcpriMessage::DelayMeasurement(dm) => dm.clone(),
            other => return Err(EcpriError::NotADelayMeasurement(other.message_type())),
        };

        match dm.action {
            EcpriDelayAction::Request | EcpriDelayAction::Response => {
                Ok(OwdEvent::Completed(self.complete(&dm, t2_ns, false)))
            }
            EcpriDelayAction::RequestWithFollowUp => {
                self.pending.insert(dm.measurement_id, t2_ns);
                Ok(OwdEvent::AwaitingFollowUp {
                    measurement_id: dm.measurement_id,
                })
            }
            EcpriDelayAction::FollowUp => match self.pending.remove(&dm.measurement_id) {
                // t2 was captured when the Request arrived, not when the Follow_Up did.
                Some(stored_t2) => Ok(OwdEvent::Completed(self.complete(&dm, stored_t2, true))),
                None => {
                    self.orphan_follow_ups += 1;
                    Ok(OwdEvent::OrphanFollowUp {
                        measurement_id: dm.measurement_id,
                    })
                }
            },
            EcpriDelayAction::RemoteRequest | EcpriDelayAction::RemoteRequestWithFollowUp => {
                Ok(OwdEvent::RemoteRequest {
                    measurement_id: dm.measurement_id,
                    expects_follow_up: dm.action == EcpriDelayAction::RemoteRequestWithFollowUp,
                })
            }
        }
    }

    fn complete(
        &mut self,
        dm: &EcpriDelayMeasurement,
        t2_ns: i128,
        two_step: bool,
    ) -> OwdMeasurementResult {
        let t1_ns = dm.timestamp.to_total_nanoseconds();
        let compensation_ns = dm.compensation_ns() as i128;
        let result = OwdMeasurementResult {
            measurement_id: dm.measurement_id,
            t1_ns,
            t2_ns,
            compensation_ns,
            one_way_delay_ns: one_way_delay_ns(t1_ns, t2_ns, compensation_ns),
            two_step,
        };
        self.completed.push(result);
        result
    }

    /// Measurements still waiting for their Follow_Up message.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Mean of every completed one-way delay, rounded toward zero.
    pub fn average_one_way_delay_ns(&self) -> Option<i128> {
        if self.completed.is_empty() {
            return None;
        }
        let sum: i128 = self.completed.iter().map(|m| m.one_way_delay_ns).sum();
        Some(sum / self.completed.len() as i128)
    }

    /// Spread between the smallest and largest completed measurement (observed PDV).
    pub fn delay_variation_ns(&self) -> Option<i128> {
        let min = self.completed.iter().map(|m| m.one_way_delay_ns).min()?;
        let max = self.completed.iter().map(|m| m.one_way_delay_ns).max()?;
        Some(max - min)
    }
}

/// Reassembles IQ Data messages fragmented at radio-transport level.
///
/// Fragments of one `sequence_id` arrive with increasing `subsequence_id`; the
/// fragment carrying the E bit terminates the burst.
#[derive(Debug, Clone)]
pub struct EcpriIqReassembler {
    pub pc_id: u16,
    current_sequence: Option<u8>,
    next_subsequence: u8,
    buffer: Vec<u8>,
    pub completed_bursts: Vec<Vec<u8>>,
    pub discarded_fragments: u64,
    pub aborted_bursts: u64,
}

/// Result of feeding one IQ Data fragment to [`EcpriIqReassembler`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IqReassemblyEvent {
    /// Fragment stored; more subsequence fragments are expected.
    Buffered {
        sequence_id: u8,
        buffered_len: usize,
    },
    /// E bit seen: the burst is complete and appended to `completed_bursts`.
    BurstComplete { sequence_id: u8, payload_len: usize },
    /// Out-of-order, duplicate or foreign fragment; dropped.
    Discarded { reason: &'static str },
}

impl EcpriIqReassembler {
    pub fn new(pc_id: u16) -> Self {
        EcpriIqReassembler {
            pc_id,
            current_sequence: None,
            next_subsequence: 0,
            buffer: Vec::new(),
            completed_bursts: Vec::new(),
            discarded_fragments: 0,
            aborted_bursts: 0,
        }
    }

    pub fn accept(&mut self, message: &EcpriMessage) -> IqReassemblyEvent {
        let (pc_id, seq_id, samples) = match message {
            EcpriMessage::IqData {
                pc_id,
                seq_id,
                samples,
            } => (*pc_id, *seq_id, samples),
            _ => return self.discard("not an IQ Data message"),
        };

        if pc_id != self.pc_id {
            return self.discard("PC_ID does not belong to this flow");
        }

        match self.current_sequence {
            // A new burst may only start at subsequence 0.
            None => {
                if seq_id.subsequence_id != 0 {
                    return self.discard("burst does not start at subsequence 0");
                }
                self.current_sequence = Some(seq_id.sequence_id);
                self.next_subsequence = 0;
            }
            Some(active) => {
                if active != seq_id.sequence_id {
                    // The sender moved on: the partial burst can never complete.
                    self.buffer.clear();
                    self.aborted_bursts += 1;
                    if seq_id.subsequence_id != 0 {
                        self.current_sequence = None;
                        self.next_subsequence = 0;
                        return self.discard("new sequence started mid-burst");
                    }
                    self.current_sequence = Some(seq_id.sequence_id);
                    self.next_subsequence = 0;
                } else if seq_id.subsequence_id != self.next_subsequence {
                    return self.discard("subsequence gap or duplicate");
                }
            }
        }

        self.buffer.extend_from_slice(samples);
        self.next_subsequence = self.next_subsequence.wrapping_add(1) & 0x7F;

        if seq_id.last_subsequence {
            let payload_len = self.buffer.len();
            self.completed_bursts.push(std::mem::take(&mut self.buffer));
            self.current_sequence = None;
            self.next_subsequence = 0;
            IqReassemblyEvent::BurstComplete {
                sequence_id: seq_id.sequence_id,
                payload_len,
            }
        } else {
            IqReassemblyEvent::Buffered {
                sequence_id: seq_id.sequence_id,
                buffered_len: self.buffer.len(),
            }
        }
    }

    fn discard(&mut self, reason: &'static str) -> IqReassemblyEvent {
        self.discarded_fragments += 1;
        IqReassemblyEvent::Discarded { reason }
    }

    /// Bytes held for the burst currently being reassembled.
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}
