//! Border Gateway Protocol Version 4 (BGP-4 - RFC 4271).
//!
//! Inter-domain path-vector routing protocol over TCP port 179.
//! Features 19-byte BGP framing, OPEN, UPDATE (AS_PATH / NEXT_HOP), and KEEPALIVE messages.

use crate::ipv4::Ipv4Address;
use std::collections::HashMap;
use std::fmt;

pub const BGP_PORT: u16 = 179;
pub const BGP_HEADER_LEN: usize = 19;
pub const BGP_MARKER: [u8; 16] = [0xFF; 16];

// BGP Message Types
pub const BGP_MSG_OPEN: u8 = 1;
pub const BGP_MSG_UPDATE: u8 = 2;
pub const BGP_MSG_NOTIFICATION: u8 = 3;
pub const BGP_MSG_KEEPALIVE: u8 = 4;

// BGP Path Attribute Types
pub const BGP_ATTR_ORIGIN: u8 = 1;
pub const BGP_ATTR_AS_PATH: u8 = 2;
pub const BGP_ATTR_NEXT_HOP: u8 = 3;
pub const BGP_ATTR_MED: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgpMessage {
    Open {
        version: u8,
        my_as: u16,
        hold_time: u16,
        bgp_id: Ipv4Address,
    },
    Update {
        as_path: Vec<u16>,
        next_hop: Ipv4Address,
        nlri_prefix: Ipv4Address,
        nlri_mask: u8,
    },
    Keepalive,
    Notification {
        error_code: u8,
        error_subcode: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgpError {
    PacketTooShort(usize),
    InvalidMarker,
    InvalidType(u8),
    InvalidLength(u16),
}

impl fmt::Display for BgpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BgpError::PacketTooShort(l) => write!(f, "BGP packet too short ({} bytes, min 19)", l),
            BgpError::InvalidMarker => write!(f, "Invalid BGP 16-byte marker (expected all 0xFF)"),
            BgpError::InvalidType(t) => write!(f, "Invalid BGP message type: {}", t),
            BgpError::InvalidLength(l) => write!(f, "Invalid BGP message length: {}", l),
        }
    }
}

impl std::error::Error for BgpError {}

impl BgpMessage {
    pub fn parse(data: &[u8]) -> Result<Self, BgpError> {
        if data.len() < BGP_HEADER_LEN {
            return Err(BgpError::PacketTooShort(data.len()));
        }

        if data[0..16] != BGP_MARKER {
            return Err(BgpError::InvalidMarker);
        }

        let length = u16::from_be_bytes([data[16], data[17]]);
        let msg_type = data[18];

        if (data.len() as u16) < length {
            return Err(BgpError::PacketTooShort(data.len()));
        }

        let body = &data[BGP_HEADER_LEN..length as usize];

        match msg_type {
            BGP_MSG_OPEN => {
                if body.len() < 10 {
                    return Err(BgpError::PacketTooShort(body.len()));
                }
                let version = body[0];
                let my_as = u16::from_be_bytes([body[1], body[2]]);
                let hold_time = u16::from_be_bytes([body[3], body[4]]);
                let bgp_id = Ipv4Address([body[5], body[6], body[7], body[8]]);
                Ok(BgpMessage::Open {
                    version,
                    my_as,
                    hold_time,
                    bgp_id,
                })
            }
            BGP_MSG_KEEPALIVE => Ok(BgpMessage::Keepalive),
            BGP_MSG_NOTIFICATION => {
                let error_code = body.first().copied().unwrap_or(0);
                let error_subcode = body.get(1).copied().unwrap_or(0);
                Ok(BgpMessage::Notification {
                    error_code,
                    error_subcode,
                })
            }
            BGP_MSG_UPDATE => {
                // Simplified parser for AS_PATH and NEXT_HOP + NLRI
                let mut as_path = Vec::new();
                let mut next_hop = Ipv4Address::new(0, 0, 0, 0);
                let mut nlri_prefix = Ipv4Address::new(0, 0, 0, 0);
                let mut nlri_mask = 24u8;

                if body.len() >= 4 {
                    let withdrawn_len = u16::from_be_bytes([body[0], body[1]]) as usize;
                    let attr_offset = 2 + withdrawn_len;
                    if body.len() >= attr_offset + 2 {
                        let total_attr_len =
                            u16::from_be_bytes([body[attr_offset], body[attr_offset + 1]]) as usize;
                        let mut curr = attr_offset + 2;
                        let attr_end = curr + total_attr_len;

                        while curr + 3 <= attr_end && curr + 3 <= body.len() {
                            let _flags = body[curr];
                            let type_code = body[curr + 1];
                            let attr_len = body[curr + 2] as usize;
                            let val_start = curr + 3;
                            let val_end = val_start + attr_len;

                            if val_end <= body.len() {
                                match type_code {
                                    BGP_ATTR_AS_PATH => {
                                        if attr_len >= 2 {
                                            // Segment Type (1B), Path Length (1B), AS numbers (2B each)
                                            let seg_len = body[val_start + 1] as usize;
                                            for i in 0..seg_len {
                                                let offset = val_start + 2 + i * 2;
                                                if offset + 2 <= val_end {
                                                    as_path.push(u16::from_be_bytes([
                                                        body[offset],
                                                        body[offset + 1],
                                                    ]));
                                                }
                                            }
                                        }
                                    }
                                    BGP_ATTR_NEXT_HOP if attr_len == 4 => {
                                        next_hop = Ipv4Address([
                                            body[val_start],
                                            body[val_start + 1],
                                            body[val_start + 2],
                                            body[val_start + 3],
                                        ]);
                                    }
                                    _ => {}
                                }
                            }
                            curr = val_end;
                        }

                        // NLRI at end of update
                        if attr_end < body.len() {
                            nlri_mask = body[attr_end];
                            if attr_end + 4 < body.len() {
                                nlri_prefix = Ipv4Address([
                                    body[attr_end + 1],
                                    body[attr_end + 2],
                                    body[attr_end + 3],
                                    body[attr_end + 4],
                                ]);
                            }
                        }
                    }
                }

                Ok(BgpMessage::Update {
                    as_path,
                    next_hop,
                    nlri_prefix,
                    nlri_mask,
                })
            }
            _ => Err(BgpError::InvalidType(msg_type)),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut body = Vec::new();
        let msg_type = match self {
            BgpMessage::Open {
                version,
                my_as,
                hold_time,
                bgp_id,
            } => {
                body.push(*version);
                body.extend_from_slice(&my_as.to_be_bytes());
                body.extend_from_slice(&hold_time.to_be_bytes());
                body.extend_from_slice(&bgp_id.0);
                body.push(0); // Opt Param Len = 0
                BGP_MSG_OPEN
            }
            BgpMessage::Keepalive => BGP_MSG_KEEPALIVE,
            BgpMessage::Notification {
                error_code,
                error_subcode,
            } => {
                body.push(*error_code);
                body.push(*error_subcode);
                BGP_MSG_NOTIFICATION
            }
            BgpMessage::Update {
                as_path,
                next_hop,
                nlri_prefix,
                nlri_mask,
            } => {
                body.extend_from_slice(&0u16.to_be_bytes()); // Withdrawn Routes Len = 0

                let mut attrs = Vec::new();
                // 1. ORIGIN = IGP (0)
                attrs.extend_from_slice(&[0x40, BGP_ATTR_ORIGIN, 1, 0]);

                // 2. AS_PATH
                let mut as_seg = Vec::new();
                as_seg.push(2); // AS_SEQUENCE (2)
                as_seg.push(as_path.len() as u8);
                for asn in as_path {
                    as_seg.extend_from_slice(&asn.to_be_bytes());
                }
                attrs.push(0x40);
                attrs.push(BGP_ATTR_AS_PATH);
                attrs.push(as_seg.len() as u8);
                attrs.extend(as_seg);

                // 3. NEXT_HOP
                attrs.extend_from_slice(&[0x40, BGP_ATTR_NEXT_HOP, 4]);
                attrs.extend_from_slice(&next_hop.0);

                body.extend_from_slice(&(attrs.len() as u16).to_be_bytes());
                body.extend(attrs);

                // NLRI
                body.push(*nlri_mask);
                body.extend_from_slice(&nlri_prefix.0);

                BGP_MSG_UPDATE
            }
        };

        let total_len = (BGP_HEADER_LEN + body.len()) as u16;
        let mut buf = Vec::with_capacity(total_len as usize);

        buf.extend_from_slice(&BGP_MARKER);
        buf.extend_from_slice(&total_len.to_be_bytes());
        buf.push(msg_type);
        buf.extend(body);

        buf
    }

    pub fn build_open(asn: u16, hold_time: u16, router_id: Ipv4Address) -> Self {
        BgpMessage::Open {
            version: 4,
            my_as: asn,
            hold_time,
            bgp_id: router_id,
        }
    }

    pub fn build_update(
        prefix: Ipv4Address,
        mask: u8,
        next_hop: Ipv4Address,
        as_path: Vec<u16>,
    ) -> Self {
        BgpMessage::Update {
            as_path,
            next_hop,
            nlri_prefix: prefix,
            nlri_mask: mask,
        }
    }
}

/// BGP Routing Information Base (RIB)
pub struct BgpRib {
    routes: HashMap<(Ipv4Address, u8), (Ipv4Address, Vec<u16>)>,
}

impl Default for BgpRib {
    fn default() -> Self {
        Self::new()
    }
}

impl BgpRib {
    pub fn new() -> Self {
        let mut rib = BgpRib {
            routes: HashMap::new(),
        };
        rib.insert(
            Ipv4Address::new(8, 8, 8, 0),
            24,
            Ipv4Address::new(198, 51, 100, 1),
            vec![65001, 15169],
        );
        rib.insert(
            Ipv4Address::new(1, 1, 1, 0),
            24,
            Ipv4Address::new(203, 0, 113, 1),
            vec![65001, 13335],
        );
        rib
    }

    pub fn insert(
        &mut self,
        prefix: Ipv4Address,
        mask: u8,
        next_hop: Ipv4Address,
        as_path: Vec<u16>,
    ) {
        self.routes.insert((prefix, mask), (next_hop, as_path));
    }

    pub fn all_routes(&self) -> &HashMap<(Ipv4Address, u8), (Ipv4Address, Vec<u16>)> {
        &self.routes
    }
}

// ============================================================================
// Strict RFC 4271 wire layer.
//
// `BgpMessage` above is the original convenience codec: it models one NLRI per
// UPDATE and is tolerant of odd encodings. The control plane needs something
// stricter and richer, so the types below add full path-attribute handling,
// withdrawn routes, multi-prefix NLRI, and validation that maps every failure to
// the NOTIFICATION code the RFC prescribes. Both share the framing constants and
// the 16-byte marker.
// ============================================================================

pub const BGP_VERSION: u8 = 4;
/// Largest legal BGP message, RFC 4271 section 4.1. Nothing larger is ever buffered.
pub const BGP_MAX_MESSAGE_LEN: usize = 4096;
/// Smallest non-zero hold time a peer may propose, RFC 4271 section 4.2.
pub const BGP_MIN_HOLD_TIME: u16 = 3;
/// Default LOCAL_PREF applied to paths that arrive without the attribute.
pub const BGP_DEFAULT_LOCAL_PREF: u32 = 100;

pub const BGP_ATTR_LOCAL_PREF: u8 = 5;
pub const BGP_ATTR_ATOMIC_AGGREGATE: u8 = 6;
pub const BGP_ATTR_AGGREGATOR: u8 = 7;

pub const BGP_ATTR_FLAG_OPTIONAL: u8 = 0x80;
pub const BGP_ATTR_FLAG_TRANSITIVE: u8 = 0x40;
pub const BGP_ATTR_FLAG_PARTIAL: u8 = 0x20;
pub const BGP_ATTR_FLAG_EXT_LEN: u8 = 0x10;

/// Largest number of ASNs one AS_PATH segment can carry: the count is a single octet.
pub const AS_PATH_MAX_SEGMENT_ASNS: usize = 255;

pub const BGP_AS_SET: u8 = 1;
pub const BGP_AS_SEQUENCE: u8 = 2;

// NOTIFICATION error codes (RFC 4271 section 4.5).
pub const BGP_ERR_MESSAGE_HEADER: u8 = 1;
pub const BGP_ERR_OPEN_MESSAGE: u8 = 2;
pub const BGP_ERR_UPDATE_MESSAGE: u8 = 3;
pub const BGP_ERR_HOLD_TIMER_EXPIRED: u8 = 4;
pub const BGP_ERR_FSM: u8 = 5;
pub const BGP_ERR_CEASE: u8 = 6;

// Message-header subcodes.
pub const BGP_SUB_CONNECTION_NOT_SYNCHRONIZED: u8 = 1;
pub const BGP_SUB_BAD_MESSAGE_LENGTH: u8 = 2;
pub const BGP_SUB_BAD_MESSAGE_TYPE: u8 = 3;

// OPEN subcodes.
pub const BGP_SUB_UNSUPPORTED_VERSION: u8 = 1;
pub const BGP_SUB_BAD_PEER_AS: u8 = 2;
pub const BGP_SUB_BAD_BGP_IDENTIFIER: u8 = 3;
pub const BGP_SUB_UNSUPPORTED_OPT_PARAM: u8 = 4;
pub const BGP_SUB_UNACCEPTABLE_HOLD_TIME: u8 = 6;

// UPDATE subcodes.
pub const BGP_SUB_MALFORMED_ATTRIBUTE_LIST: u8 = 1;
pub const BGP_SUB_UNRECOGNIZED_WELL_KNOWN_ATTR: u8 = 2;
pub const BGP_SUB_MISSING_WELL_KNOWN_ATTR: u8 = 3;
pub const BGP_SUB_ATTRIBUTE_FLAGS_ERROR: u8 = 4;
pub const BGP_SUB_ATTRIBUTE_LENGTH_ERROR: u8 = 5;
pub const BGP_SUB_INVALID_ORIGIN: u8 = 6;
pub const BGP_SUB_INVALID_NEXT_HOP: u8 = 8;
pub const BGP_SUB_INVALID_NETWORK_FIELD: u8 = 10;
pub const BGP_SUB_MALFORMED_AS_PATH: u8 = 11;

/// A decoding failure, carrying the NOTIFICATION code/subcode the peer should be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpParseError {
    pub code: u8,
    pub subcode: u8,
    pub reason: String,
}

impl BgpParseError {
    pub fn new(code: u8, subcode: u8, reason: impl Into<String>) -> Self {
        BgpParseError {
            code,
            subcode,
            reason: reason.into(),
        }
    }

    pub fn header(subcode: u8, reason: impl Into<String>) -> Self {
        Self::new(BGP_ERR_MESSAGE_HEADER, subcode, reason)
    }

    pub fn open(subcode: u8, reason: impl Into<String>) -> Self {
        Self::new(BGP_ERR_OPEN_MESSAGE, subcode, reason)
    }

    pub fn update(subcode: u8, reason: impl Into<String>) -> Self {
        Self::new(BGP_ERR_UPDATE_MESSAGE, subcode, reason)
    }
}

impl fmt::Display for BgpParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {}/{})", self.reason, self.code, self.subcode)
    }
}

impl std::error::Error for BgpParseError {}

/// An IPv4 destination prefix. Host bits below `length` are always cleared, so two
/// prefixes that describe the same destination compare and hash equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ipv4Prefix {
    pub address: Ipv4Address,
    pub length: u8,
}

impl Ipv4Prefix {
    pub fn new(address: Ipv4Address, length: u8) -> Self {
        let length = length.min(32);
        Ipv4Prefix {
            address: address.mask(length),
            length,
        }
    }

    pub fn contains(&self, ip: Ipv4Address) -> bool {
        ip.mask(self.length) == self.address
    }

    /// Bytes this prefix occupies in an NLRI list: one length octet plus the
    /// minimum number of address octets needed to carry `length` bits.
    pub fn encoded_len(&self) -> usize {
        1 + self.length.div_ceil(8) as usize
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let octets = self.length.div_ceil(8) as usize;
        out.push(self.length);
        out.extend_from_slice(&self.address.0[..octets]);
    }

    /// Decodes a complete NLRI / withdrawn-routes list. Any truncation or a prefix
    /// length above 32 is rejected rather than silently clamped.
    pub fn decode_list(data: &[u8], subcode: u8) -> Result<Vec<Ipv4Prefix>, BgpParseError> {
        let mut out = Vec::new();
        let mut i = 0usize;
        while i < data.len() {
            let bits = data[i];
            if bits > 32 {
                return Err(BgpParseError::update(
                    subcode,
                    format!("prefix length {} exceeds 32 bits", bits),
                ));
            }
            let octets = bits.div_ceil(8) as usize;
            if i + 1 + octets > data.len() {
                return Err(BgpParseError::update(
                    subcode,
                    "truncated prefix in NLRI list",
                ));
            }
            let mut addr = [0u8; 4];
            addr[..octets].copy_from_slice(&data[i + 1..i + 1 + octets]);
            out.push(Ipv4Prefix::new(Ipv4Address(addr), bits));
            i += 1 + octets;
        }
        Ok(out)
    }
}

impl fmt::Display for Ipv4Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.length)
    }
}

/// ORIGIN attribute value (RFC 4271 section 5.1.1). Ordered by preference: IGP is best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BgpOrigin {
    #[default]
    Igp,
    Egp,
    Incomplete,
}

impl BgpOrigin {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(BgpOrigin::Igp),
            1 => Some(BgpOrigin::Egp),
            2 => Some(BgpOrigin::Incomplete),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            BgpOrigin::Igp => 0,
            BgpOrigin::Egp => 1,
            BgpOrigin::Incomplete => 2,
        }
    }
}

impl fmt::Display for BgpOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BgpOrigin::Igp => write!(f, "i"),
            BgpOrigin::Egp => write!(f, "e"),
            BgpOrigin::Incomplete => write!(f, "?"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AsPathSegmentKind {
    Set,
    Sequence,
}

/// One AS_PATH segment. A SET contributes 1 to the path length no matter how many
/// ASNs it holds, which is what RFC 4271 section 9.1.2.2 requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AsPathSegment {
    pub kind: AsPathSegmentKind,
    pub asns: Vec<u16>,
}

/// The AS_PATH attribute as a list of segments, with the helpers the decision
/// process and loop detection need.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AsPath {
    pub segments: Vec<AsPathSegment>,
}

impl AsPath {
    pub fn empty() -> Self {
        AsPath::default()
    }

    /// Builds a single AS_SEQUENCE path, leftmost ASN first.
    pub fn sequence(asns: Vec<u16>) -> Self {
        if asns.is_empty() {
            return AsPath::default();
        }
        AsPath {
            segments: vec![AsPathSegment {
                kind: AsPathSegmentKind::Sequence,
                asns,
            }],
        }
    }

    /// Path length used by the decision process: every AS in a SEQUENCE counts once,
    /// a whole SET counts once.
    pub fn length(&self) -> usize {
        self.segments
            .iter()
            .map(|s| match s.kind {
                AsPathSegmentKind::Sequence => s.asns.len(),
                AsPathSegmentKind::Set => 1,
            })
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.iter().all(|s| s.asns.is_empty())
    }

    pub fn contains(&self, asn: u16) -> bool {
        self.segments.iter().any(|s| s.asns.contains(&asn))
    }

    /// Leftmost ASN of the leftmost AS_SEQUENCE: the neighbouring AS that advertised
    /// the route. Used to decide whether two MEDs are comparable.
    pub fn first_as(&self) -> Option<u16> {
        self.segments
            .iter()
            .find(|s| s.kind == AsPathSegmentKind::Sequence)
            .and_then(|s| s.asns.first().copied())
    }

    /// Leftmost ASN, but only when the path genuinely *begins* with an AS_SEQUENCE.
    ///
    /// This is the stricter reading needed to police an eBGP UPDATE, which has to lead
    /// with the advertising peer's own ASN. [`AsPath::first_as`] deliberately skips a
    /// leading AS_SET to find something MED-comparable; that would be the wrong answer
    /// here, because a path that leads with an AS_SET has no leading AS at all.
    pub fn leading_as(&self) -> Option<u16> {
        match self.segments.first() {
            Some(seg) if seg.kind == AsPathSegmentKind::Sequence => seg.asns.first().copied(),
            _ => None,
        }
    }

    /// Prepends the local ASN, as an eBGP speaker must do before re-advertising.
    /// A leading SEQUENCE is extended; anything else gets a fresh SEQUENCE in front.
    pub fn prepend(&mut self, asn: u16) {
        match self.segments.first_mut() {
            Some(seg) if seg.kind == AsPathSegmentKind::Sequence && seg.asns.len() < 255 => {
                seg.asns.insert(0, asn);
            }
            _ => self.segments.insert(
                0,
                AsPathSegment {
                    kind: AsPathSegmentKind::Sequence,
                    asns: vec![asn],
                },
            ),
        }
    }

    /// Flattened ASN list, left to right. Convenient for assertions and display.
    pub fn flatten(&self) -> Vec<u16> {
        self.segments.iter().flat_map(|s| s.asns.clone()).collect()
    }

    /// Encodes the path.
    ///
    /// A segment holding more than [`AS_PATH_MAX_SEGMENT_ASNS`] entries is emitted as
    /// several segments of the same kind. Writing the length as a single octet instead
    /// would truncate the count and put an AS_PATH on the wire that no decoder can
    /// read. An empty segment is dropped rather than encoded, because the wire format
    /// gives it no meaning and a decoder is required to reject it.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for seg in &self.segments {
            let kind = match seg.kind {
                AsPathSegmentKind::Set => BGP_AS_SET,
                AsPathSegmentKind::Sequence => BGP_AS_SEQUENCE,
            };
            for chunk in seg.asns.chunks(AS_PATH_MAX_SEGMENT_ASNS) {
                out.push(kind);
                out.push(chunk.len() as u8);
                for asn in chunk {
                    out.extend_from_slice(&asn.to_be_bytes());
                }
            }
        }
        out
    }

    pub fn decode(data: &[u8]) -> Result<Self, BgpParseError> {
        let mut segments = Vec::new();
        let mut i = 0usize;
        while i < data.len() {
            if i + 2 > data.len() {
                return Err(BgpParseError::update(
                    BGP_SUB_MALFORMED_AS_PATH,
                    "truncated AS_PATH segment header",
                ));
            }
            let kind = match data[i] {
                BGP_AS_SET => AsPathSegmentKind::Set,
                BGP_AS_SEQUENCE => AsPathSegmentKind::Sequence,
                other => {
                    return Err(BgpParseError::update(
                        BGP_SUB_MALFORMED_AS_PATH,
                        format!("unknown AS_PATH segment type {}", other),
                    ));
                }
            };
            let count = data[i + 1] as usize;
            if count == 0 {
                return Err(BgpParseError::update(
                    BGP_SUB_MALFORMED_AS_PATH,
                    "empty AS_PATH segment",
                ));
            }
            let end = i + 2 + count * 2;
            if end > data.len() {
                return Err(BgpParseError::update(
                    BGP_SUB_MALFORMED_AS_PATH,
                    "truncated AS_PATH segment body",
                ));
            }
            let mut asns = Vec::with_capacity(count);
            for k in 0..count {
                let off = i + 2 + k * 2;
                asns.push(u16::from_be_bytes([data[off], data[off + 1]]));
            }
            segments.push(AsPathSegment { kind, asns });
            i = end;
        }
        Ok(AsPath { segments })
    }
}

impl fmt::Display for AsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in &self.segments {
            let text = seg
                .asns
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let text = match seg.kind {
                AsPathSegmentKind::Set => format!("{{{}}}", text),
                AsPathSegmentKind::Sequence => text,
            };
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{}", text)?;
            first = false;
        }
        if first {
            write!(f, "-")?;
        }
        Ok(())
    }
}

/// The path attributes attached to the NLRI of one UPDATE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpPathAttributes {
    pub origin: BgpOrigin,
    pub as_path: AsPath,
    pub next_hop: Ipv4Address,
    pub med: Option<u32>,
    pub local_pref: Option<u32>,
    pub atomic_aggregate: bool,
}

impl BgpPathAttributes {
    pub fn new(origin: BgpOrigin, as_path: AsPath, next_hop: Ipv4Address) -> Self {
        BgpPathAttributes {
            origin,
            as_path,
            next_hop,
            med: None,
            local_pref: None,
            atomic_aggregate: false,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        out.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_ORIGIN, 1]);
        out.push(self.origin.to_u8());

        let path = self.as_path.encode();
        out.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_AS_PATH]);
        out.push(path.len() as u8);
        out.extend_from_slice(&path);

        out.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_NEXT_HOP, 4]);
        out.extend_from_slice(&self.next_hop.0);

        if let Some(med) = self.med {
            out.extend_from_slice(&[BGP_ATTR_FLAG_OPTIONAL, BGP_ATTR_MED, 4]);
            out.extend_from_slice(&med.to_be_bytes());
        }

        if let Some(lp) = self.local_pref {
            out.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_LOCAL_PREF, 4]);
            out.extend_from_slice(&lp.to_be_bytes());
        }

        if self.atomic_aggregate {
            out.extend_from_slice(&[BGP_ATTR_FLAG_TRANSITIVE, BGP_ATTR_ATOMIC_AGGREGATE, 0]);
        }

        out
    }
}

/// A decoded UPDATE: routes being withdrawn, the attributes for the announced
/// routes, and the announced NLRI. An UPDATE with neither NLRI nor withdrawn
/// routes and no attributes is the End-of-RIB marker and decodes cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpUpdateMessage {
    pub withdrawn: Vec<Ipv4Prefix>,
    pub attributes: Option<BgpPathAttributes>,
    pub nlri: Vec<Ipv4Prefix>,
}

impl BgpUpdateMessage {
    pub fn announce(attributes: BgpPathAttributes, nlri: Vec<Ipv4Prefix>) -> Self {
        BgpUpdateMessage {
            withdrawn: Vec::new(),
            attributes: Some(attributes),
            nlri,
        }
    }

    pub fn withdraw(withdrawn: Vec<Ipv4Prefix>) -> Self {
        BgpUpdateMessage {
            withdrawn,
            attributes: None,
            nlri: Vec::new(),
        }
    }

    pub fn end_of_rib() -> Self {
        BgpUpdateMessage {
            withdrawn: Vec::new(),
            attributes: None,
            nlri: Vec::new(),
        }
    }

    pub fn is_end_of_rib(&self) -> bool {
        self.withdrawn.is_empty() && self.nlri.is_empty() && self.attributes.is_none()
    }

    fn encode_body(&self) -> Vec<u8> {
        let mut withdrawn_bytes = Vec::new();
        for p in &self.withdrawn {
            p.encode(&mut withdrawn_bytes);
        }
        let attr_bytes = match &self.attributes {
            Some(a) if !self.nlri.is_empty() => a.encode(),
            _ => Vec::new(),
        };

        let mut body = Vec::new();
        body.extend_from_slice(&(withdrawn_bytes.len() as u16).to_be_bytes());
        body.extend_from_slice(&withdrawn_bytes);
        body.extend_from_slice(&(attr_bytes.len() as u16).to_be_bytes());
        body.extend_from_slice(&attr_bytes);
        for p in &self.nlri {
            p.encode(&mut body);
        }
        body
    }

    /// Decodes an UPDATE body (everything after the 19-byte header).
    pub fn parse_body(body: &[u8]) -> Result<Self, BgpParseError> {
        if body.len() < 4 {
            return Err(BgpParseError::update(
                BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
                "UPDATE body shorter than the two length fields",
            ));
        }
        let withdrawn_len = u16::from_be_bytes([body[0], body[1]]) as usize;
        if 2 + withdrawn_len + 2 > body.len() {
            return Err(BgpParseError::update(
                BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
                "withdrawn routes length runs past the end of the UPDATE",
            ));
        }
        let withdrawn =
            Ipv4Prefix::decode_list(&body[2..2 + withdrawn_len], BGP_SUB_INVALID_NETWORK_FIELD)?;

        let attr_len_off = 2 + withdrawn_len;
        let attr_len = u16::from_be_bytes([body[attr_len_off], body[attr_len_off + 1]]) as usize;
        let attr_start = attr_len_off + 2;
        let attr_end = attr_start.saturating_add(attr_len);
        if attr_end > body.len() {
            return Err(BgpParseError::update(
                BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
                "path attribute length runs past the end of the UPDATE",
            ));
        }

        let attributes = Self::parse_attributes(&body[attr_start..attr_end])?;
        let nlri = Ipv4Prefix::decode_list(&body[attr_end..], BGP_SUB_INVALID_NETWORK_FIELD)?;

        // NLRI without attributes is meaningless: the mandatory ones carry the next hop.
        if !nlri.is_empty() && attributes.is_none() {
            return Err(BgpParseError::update(
                BGP_SUB_MISSING_WELL_KNOWN_ATTR,
                "UPDATE announces NLRI without the mandatory path attributes",
            ));
        }

        Ok(BgpUpdateMessage {
            withdrawn,
            attributes: if nlri.is_empty() { None } else { attributes },
            nlri,
        })
    }

    /// Decodes the path attribute block, enforcing flags, lengths, and the presence
    /// of the well-known mandatory attributes.
    fn parse_attributes(data: &[u8]) -> Result<Option<BgpPathAttributes>, BgpParseError> {
        if data.is_empty() {
            return Ok(None);
        }

        let mut origin: Option<BgpOrigin> = None;
        let mut as_path: Option<AsPath> = None;
        let mut next_hop: Option<Ipv4Address> = None;
        let mut med: Option<u32> = None;
        let mut local_pref: Option<u32> = None;
        let mut atomic_aggregate = false;
        let mut seen: Vec<u8> = Vec::new();

        let mut i = 0usize;
        while i < data.len() {
            if i + 2 > data.len() {
                return Err(BgpParseError::update(
                    BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
                    "truncated path attribute header",
                ));
            }
            let flags = data[i];
            let type_code = data[i + 1];
            let extended = flags & BGP_ATTR_FLAG_EXT_LEN != 0;
            let (len, hdr) = if extended {
                if i + 4 > data.len() {
                    return Err(BgpParseError::update(
                        BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                        "truncated extended attribute length",
                    ));
                }
                (u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize, 4)
            } else {
                if i + 3 > data.len() {
                    return Err(BgpParseError::update(
                        BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                        "truncated attribute length",
                    ));
                }
                (data[i + 2] as usize, 3)
            };
            let val_start = i + hdr;
            let val_end = val_start.saturating_add(len);
            if val_end > data.len() {
                return Err(BgpParseError::update(
                    BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                    format!(
                        "attribute {} claims {} bytes but only {} remain",
                        type_code,
                        len,
                        data.len() - val_start
                    ),
                ));
            }
            if seen.contains(&type_code) {
                return Err(BgpParseError::update(
                    BGP_SUB_MALFORMED_ATTRIBUTE_LIST,
                    format!("duplicate path attribute {}", type_code),
                ));
            }
            seen.push(type_code);

            let value = &data[val_start..val_end];
            let optional = flags & BGP_ATTR_FLAG_OPTIONAL != 0;

            match type_code {
                BGP_ATTR_ORIGIN => {
                    if optional {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_FLAGS_ERROR,
                            "ORIGIN marked optional",
                        ));
                    }
                    if value.len() != 1 {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                            "ORIGIN must be exactly one byte",
                        ));
                    }
                    origin = Some(BgpOrigin::from_u8(value[0]).ok_or_else(|| {
                        BgpParseError::update(
                            BGP_SUB_INVALID_ORIGIN,
                            format!("undefined ORIGIN value {}", value[0]),
                        )
                    })?);
                }
                BGP_ATTR_AS_PATH => {
                    if optional {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_FLAGS_ERROR,
                            "AS_PATH marked optional",
                        ));
                    }
                    as_path = Some(AsPath::decode(value)?);
                }
                BGP_ATTR_NEXT_HOP => {
                    if optional {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_FLAGS_ERROR,
                            "NEXT_HOP marked optional",
                        ));
                    }
                    if value.len() != 4 {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                            "NEXT_HOP must be exactly four bytes",
                        ));
                    }
                    next_hop = Some(Ipv4Address([value[0], value[1], value[2], value[3]]));
                }
                BGP_ATTR_MED => {
                    if value.len() != 4 {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                            "MULTI_EXIT_DISC must be exactly four bytes",
                        ));
                    }
                    med = Some(u32::from_be_bytes([value[0], value[1], value[2], value[3]]));
                }
                BGP_ATTR_LOCAL_PREF => {
                    if value.len() != 4 {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                            "LOCAL_PREF must be exactly four bytes",
                        ));
                    }
                    local_pref = Some(u32::from_be_bytes([value[0], value[1], value[2], value[3]]));
                }
                BGP_ATTR_ATOMIC_AGGREGATE => {
                    if !value.is_empty() {
                        return Err(BgpParseError::update(
                            BGP_SUB_ATTRIBUTE_LENGTH_ERROR,
                            "ATOMIC_AGGREGATE must be empty",
                        ));
                    }
                    atomic_aggregate = true;
                }
                other => {
                    // Unknown optional attributes are ignored, exactly as RFC 4271
                    // section 5 requires. An unknown *well-known* attribute is an error.
                    if !optional {
                        return Err(BgpParseError::update(
                            BGP_SUB_UNRECOGNIZED_WELL_KNOWN_ATTR,
                            format!("unrecognized well-known attribute {}", other),
                        ));
                    }
                }
            }

            i = val_end;
        }

        let origin = origin.ok_or_else(|| {
            BgpParseError::update(BGP_SUB_MISSING_WELL_KNOWN_ATTR, "UPDATE has no ORIGIN")
        })?;
        let as_path = as_path.ok_or_else(|| {
            BgpParseError::update(BGP_SUB_MISSING_WELL_KNOWN_ATTR, "UPDATE has no AS_PATH")
        })?;
        let next_hop = next_hop.ok_or_else(|| {
            BgpParseError::update(BGP_SUB_MISSING_WELL_KNOWN_ATTR, "UPDATE has no NEXT_HOP")
        })?;

        Ok(Some(BgpPathAttributes {
            origin,
            as_path,
            next_hop,
            med,
            local_pref,
            atomic_aggregate,
        }))
    }
}

/// A decoded OPEN message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpOpenMessage {
    pub version: u8,
    pub my_as: u16,
    pub hold_time: u16,
    pub bgp_id: Ipv4Address,
    pub opt_params: Vec<u8>,
}

impl BgpOpenMessage {
    pub fn new(my_as: u16, hold_time: u16, bgp_id: Ipv4Address) -> Self {
        BgpOpenMessage {
            version: BGP_VERSION,
            my_as,
            hold_time,
            bgp_id,
            opt_params: Vec::new(),
        }
    }

    fn encode_body(&self) -> Vec<u8> {
        let mut body = Vec::with_capacity(10 + self.opt_params.len());
        body.push(self.version);
        body.extend_from_slice(&self.my_as.to_be_bytes());
        body.extend_from_slice(&self.hold_time.to_be_bytes());
        body.extend_from_slice(&self.bgp_id.0);
        body.push(self.opt_params.len() as u8);
        body.extend_from_slice(&self.opt_params);
        body
    }

    pub fn parse_body(body: &[u8]) -> Result<Self, BgpParseError> {
        if body.len() < 10 {
            return Err(BgpParseError::header(
                BGP_SUB_BAD_MESSAGE_LENGTH,
                "OPEN body shorter than the 10-byte fixed part",
            ));
        }
        let opt_len = body[9] as usize;
        if 10 + opt_len > body.len() {
            return Err(BgpParseError::open(
                BGP_SUB_UNSUPPORTED_OPT_PARAM,
                "optional parameter length runs past the end of the OPEN",
            ));
        }
        Ok(BgpOpenMessage {
            version: body[0],
            my_as: u16::from_be_bytes([body[1], body[2]]),
            hold_time: u16::from_be_bytes([body[3], body[4]]),
            bgp_id: Ipv4Address([body[5], body[6], body[7], body[8]]),
            opt_params: body[10..10 + opt_len].to_vec(),
        })
    }
}

/// A decoded NOTIFICATION message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgpNotificationMessage {
    pub error_code: u8,
    pub error_subcode: u8,
    pub data: Vec<u8>,
}

impl BgpNotificationMessage {
    pub fn new(error_code: u8, error_subcode: u8) -> Self {
        BgpNotificationMessage {
            error_code,
            error_subcode,
            data: Vec::new(),
        }
    }

    pub fn describe(&self) -> String {
        let code = match self.error_code {
            BGP_ERR_MESSAGE_HEADER => "Message Header Error",
            BGP_ERR_OPEN_MESSAGE => "OPEN Message Error",
            BGP_ERR_UPDATE_MESSAGE => "UPDATE Message Error",
            BGP_ERR_HOLD_TIMER_EXPIRED => "Hold Timer Expired",
            BGP_ERR_FSM => "Finite State Machine Error",
            BGP_ERR_CEASE => "Cease",
            _ => "Unknown Error",
        };
        format!("{} ({}/{})", code, self.error_code, self.error_subcode)
    }
}

/// A fully decoded BGP protocol data unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BgpPdu {
    Open(BgpOpenMessage),
    Update(BgpUpdateMessage),
    Notification(BgpNotificationMessage),
    Keepalive,
}

impl BgpPdu {
    pub fn type_code(&self) -> u8 {
        match self {
            BgpPdu::Open(_) => BGP_MSG_OPEN,
            BgpPdu::Update(_) => BGP_MSG_UPDATE,
            BgpPdu::Notification(_) => BGP_MSG_NOTIFICATION,
            BgpPdu::Keepalive => BGP_MSG_KEEPALIVE,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            BgpPdu::Open(_) => "OPEN",
            BgpPdu::Update(_) => "UPDATE",
            BgpPdu::Notification(_) => "NOTIFICATION",
            BgpPdu::Keepalive => "KEEPALIVE",
        }
    }

    /// Serializes into a complete on-the-wire message including the 19-byte header.
    pub fn serialize(&self) -> Vec<u8> {
        let body = match self {
            BgpPdu::Open(o) => o.encode_body(),
            BgpPdu::Update(u) => u.encode_body(),
            BgpPdu::Notification(n) => {
                let mut b = vec![n.error_code, n.error_subcode];
                b.extend_from_slice(&n.data);
                b
            }
            BgpPdu::Keepalive => Vec::new(),
        };
        let total = BGP_HEADER_LEN + body.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&BGP_MARKER);
        out.extend_from_slice(&(total as u16).to_be_bytes());
        out.push(self.type_code());
        out.extend_from_slice(&body);
        out
    }

    /// Decodes one complete framed message. `frame` must be exactly one message as
    /// produced by `BgpFramer`; trailing bytes are rejected rather than ignored.
    pub fn parse(frame: &[u8]) -> Result<Self, BgpParseError> {
        let (msg_type, body) = parse_bgp_header(frame)?;
        match msg_type {
            BGP_MSG_OPEN => Ok(BgpPdu::Open(BgpOpenMessage::parse_body(body)?)),
            BGP_MSG_UPDATE => Ok(BgpPdu::Update(BgpUpdateMessage::parse_body(body)?)),
            BGP_MSG_NOTIFICATION => {
                if body.len() < 2 {
                    return Err(BgpParseError::header(
                        BGP_SUB_BAD_MESSAGE_LENGTH,
                        "NOTIFICATION shorter than its two-byte error code",
                    ));
                }
                Ok(BgpPdu::Notification(BgpNotificationMessage {
                    error_code: body[0],
                    error_subcode: body[1],
                    data: body[2..].to_vec(),
                }))
            }
            BGP_MSG_KEEPALIVE => {
                if !body.is_empty() {
                    return Err(BgpParseError::header(
                        BGP_SUB_BAD_MESSAGE_LENGTH,
                        "KEEPALIVE must be exactly 19 bytes",
                    ));
                }
                Ok(BgpPdu::Keepalive)
            }
            other => Err(BgpParseError::header(
                BGP_SUB_BAD_MESSAGE_TYPE,
                format!("unsupported message type {}", other),
            )),
        }
    }
}

/// Validates the 19-byte header and returns `(message_type, body)`.
///
/// Every field is checked before any of it is trusted: the marker must be the
/// all-ones pattern, the length must be inside the type's legal range, and the
/// frame must be exactly as long as the length field claims.
pub fn parse_bgp_header(frame: &[u8]) -> Result<(u8, &[u8]), BgpParseError> {
    if frame.len() < BGP_HEADER_LEN {
        return Err(BgpParseError::header(
            BGP_SUB_BAD_MESSAGE_LENGTH,
            format!("frame of {} bytes is shorter than the header", frame.len()),
        ));
    }
    if frame[0..16] != BGP_MARKER {
        return Err(BgpParseError::header(
            BGP_SUB_CONNECTION_NOT_SYNCHRONIZED,
            "marker is not the all-ones synchronisation pattern",
        ));
    }
    let length = u16::from_be_bytes([frame[16], frame[17]]) as usize;
    let msg_type = frame[18];
    if !(BGP_HEADER_LEN..=BGP_MAX_MESSAGE_LEN).contains(&length) {
        return Err(BgpParseError::header(
            BGP_SUB_BAD_MESSAGE_LENGTH,
            format!("length field {} is outside 19..=4096", length),
        ));
    }
    let min_len = match msg_type {
        BGP_MSG_OPEN => BGP_HEADER_LEN + 10,
        BGP_MSG_UPDATE => BGP_HEADER_LEN + 4,
        BGP_MSG_NOTIFICATION => BGP_HEADER_LEN + 2,
        BGP_MSG_KEEPALIVE => BGP_HEADER_LEN,
        other => {
            return Err(BgpParseError::header(
                BGP_SUB_BAD_MESSAGE_TYPE,
                format!("unsupported message type {}", other),
            ));
        }
    };
    if length < min_len {
        return Err(BgpParseError::header(
            BGP_SUB_BAD_MESSAGE_LENGTH,
            format!("length {} too small for message type {}", length, msg_type),
        ));
    }
    if frame.len() != length {
        return Err(BgpParseError::header(
            BGP_SUB_BAD_MESSAGE_LENGTH,
            format!(
                "frame carries {} bytes but the length field says {}",
                frame.len(),
                length
            ),
        ));
    }
    Ok((msg_type, &frame[BGP_HEADER_LEN..length]))
}

/// Message-type byte of a framed BGP message, without decoding the body.
/// Used by capture assertions and diagnostics.
pub fn peek_bgp_message_type(frame: &[u8]) -> Option<u8> {
    if frame.len() >= BGP_HEADER_LEN && frame[0..16] == BGP_MARKER {
        Some(frame[18])
    } else {
        None
    }
}

/// Reassembles BGP messages out of a TCP byte stream.
///
/// TCP gives no message boundaries, so a read may deliver half a header, half a
/// message, or six messages at once. The framer buffers whatever arrives and hands
/// back one complete message at a time. The buffer is hard-capped: a peer cannot
/// make it grow without bound, because a header is validated as soon as 19 bytes
/// are present and no legal message exceeds 4096 bytes.
#[derive(Debug, Clone)]
pub struct BgpFramer {
    buf: Vec<u8>,
    capacity: usize,
    pub bytes_received: u64,
    pub messages_decoded: u64,
}

/// Framer buffer cap: enough for one maximum-size message plus a partial follow-on.
pub const BGP_FRAMER_CAPACITY: usize = 2 * BGP_MAX_MESSAGE_LEN;

impl Default for BgpFramer {
    fn default() -> Self {
        Self::new()
    }
}

impl BgpFramer {
    pub fn new() -> Self {
        Self::with_capacity(BGP_FRAMER_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        BgpFramer {
            buf: Vec::new(),
            capacity: capacity.max(BGP_MAX_MESSAGE_LEN),
            bytes_received: 0,
            messages_decoded: 0,
        }
    }

    /// Appends freshly read stream bytes. Rejects input that would push the
    /// reassembly buffer past its cap instead of growing without limit.
    pub fn push(&mut self, data: &[u8]) -> Result<(), BgpParseError> {
        if self.buf.len() + data.len() > self.capacity {
            return Err(BgpParseError::new(
                BGP_ERR_CEASE,
                0,
                format!(
                    "reassembly buffer would exceed {} bytes; peer is not framing BGP",
                    self.capacity
                ),
            ));
        }
        self.buf.extend_from_slice(data);
        self.bytes_received += data.len() as u64;
        Ok(())
    }

    /// Pops the next complete message, or `Ok(None)` when more bytes are needed.
    /// A structurally invalid header is a hard error: the stream is desynchronised
    /// and the session must be torn down rather than resynchronised by guessing.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, BgpParseError> {
        if self.buf.len() < BGP_HEADER_LEN {
            return Ok(None);
        }
        if self.buf[0..16] != BGP_MARKER {
            return Err(BgpParseError::header(
                BGP_SUB_CONNECTION_NOT_SYNCHRONIZED,
                "marker is not the all-ones synchronisation pattern",
            ));
        }
        let length = u16::from_be_bytes([self.buf[16], self.buf[17]]) as usize;
        if !(BGP_HEADER_LEN..=BGP_MAX_MESSAGE_LEN).contains(&length) {
            return Err(BgpParseError::header(
                BGP_SUB_BAD_MESSAGE_LENGTH,
                format!("length field {} is outside 19..=4096", length),
            ));
        }
        if self.buf.len() < length {
            return Ok(None);
        }
        let frame: Vec<u8> = self.buf.drain(..length).collect();
        self.messages_decoded += 1;
        Ok(Some(frame))
    }

    /// Bytes currently held awaiting the rest of a message.
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bgp_open_and_keepalive_roundtrip() {
        let open = BgpMessage::build_open(65001, 180, Ipv4Address::new(10, 0, 0, 1));
        let raw_open = open.serialize();

        let parsed = BgpMessage::parse(&raw_open).unwrap();
        if let BgpMessage::Open {
            my_as,
            hold_time,
            bgp_id,
            ..
        } = parsed
        {
            assert_eq!(my_as, 65001);
            assert_eq!(hold_time, 180);
            assert_eq!(bgp_id, Ipv4Address::new(10, 0, 0, 1));
        } else {
            panic!("Expected Open message");
        }

        let keepalive = BgpMessage::Keepalive;
        let raw_ka = keepalive.serialize();
        assert_eq!(raw_ka.len(), 19);
        assert_eq!(BgpMessage::parse(&raw_ka).unwrap(), BgpMessage::Keepalive);
    }

    #[test]
    fn test_as_path_length_counts_a_set_as_one_hop() {
        let mut path = AsPath::sequence(vec![65001, 65002]);
        assert_eq!(path.length(), 2);

        path.segments.push(AsPathSegment {
            kind: AsPathSegmentKind::Set,
            asns: vec![65010, 65011, 65012],
        });
        // The whole SET contributes one hop, not three (RFC 4271 section 9.1.2.2).
        assert_eq!(path.length(), 3);
        assert!(path.contains(65011));
        assert_eq!(path.first_as(), Some(65001));
    }

    #[test]
    fn test_as_path_prepend_extends_a_leading_sequence() {
        let mut path = AsPath::sequence(vec![65002, 65003]);
        path.prepend(65001);
        assert_eq!(path.segments.len(), 1, "prepend should not add a segment");
        assert_eq!(path.flatten(), vec![65001, 65002, 65003]);
        assert_eq!(path.length(), 3);
        assert_eq!(path.first_as(), Some(65001));

        // Prepending in front of a SET has to create a new SEQUENCE instead.
        let mut path = AsPath {
            segments: vec![AsPathSegment {
                kind: AsPathSegmentKind::Set,
                asns: vec![65005, 65006],
            }],
        };
        path.prepend(65001);
        assert_eq!(path.segments.len(), 2);
        assert_eq!(path.segments[0].kind, AsPathSegmentKind::Sequence);
        assert_eq!(path.length(), 2);
        assert_eq!(path.first_as(), Some(65001));

        // An empty path is the locally originated case.
        let mut empty = AsPath::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.first_as(), None);
        empty.prepend(65001);
        assert_eq!(empty.flatten(), vec![65001]);
    }

    #[test]
    fn test_as_path_round_trips_including_sets() {
        let path = AsPath {
            segments: vec![
                AsPathSegment {
                    kind: AsPathSegmentKind::Sequence,
                    asns: vec![65001, 65002],
                },
                AsPathSegment {
                    kind: AsPathSegmentKind::Set,
                    asns: vec![65010, 65011],
                },
            ],
        };
        let encoded = path.encode();
        assert_eq!(AsPath::decode(&encoded).unwrap(), path);
        assert_eq!(path.to_string(), "65001 65002 {65010 65011}");
    }

    #[test]
    fn test_bgp_update_nlri_and_as_path() {
        let update = BgpMessage::build_update(
            Ipv4Address::new(172, 16, 0, 0),
            16,
            Ipv4Address::new(192, 168, 1, 1),
            vec![65001, 65002, 65003],
        );
        let raw = update.serialize();
        let parsed = BgpMessage::parse(&raw).unwrap();

        if let BgpMessage::Update {
            as_path,
            next_hop,
            nlri_prefix,
            nlri_mask,
        } = parsed
        {
            assert_eq!(as_path, vec![65001, 65002, 65003]);
            assert_eq!(next_hop, Ipv4Address::new(192, 168, 1, 1));
            assert_eq!(nlri_prefix, Ipv4Address::new(172, 16, 0, 0));
            assert_eq!(nlri_mask, 16);
        } else {
            panic!("Expected Update message");
        }
    }

    #[test]
    fn test_a_segment_longer_than_255_asns_is_split_rather_than_truncated() {
        // The segment count is one octet. Writing 300 as a u8 would put 44 on the
        // wire and leave the remaining ASNs to be read as segment headers, producing
        // a stream no decoder can follow.
        let asns: Vec<u16> = (0..300u16).map(|i| 1_000u16 + i).collect();
        let encoded = AsPath::sequence(asns.clone()).encode();
        let decoded = AsPath::decode(&encoded).expect("a 300-ASN path must survive encoding");

        assert_eq!(decoded.segments.len(), 2);
        assert_eq!(decoded.segments[0].asns.len(), AS_PATH_MAX_SEGMENT_ASNS);
        assert_eq!(decoded.segments[1].asns.len(), 45);
        // Splitting an AS_SEQUENCE changes nothing that matters: same ASNs, same
        // order, and the decision process still counts the same number of hops.
        assert_eq!(decoded.flatten(), asns);
        assert_eq!(decoded.length(), 300);
    }

    #[test]
    fn test_an_empty_segment_is_dropped_instead_of_encoded() {
        let path = AsPath {
            segments: vec![AsPathSegment {
                kind: AsPathSegmentKind::Sequence,
                asns: Vec::new(),
            }],
        };
        // A zero-length segment is what a decoder is required to reject, so emitting
        // one would mean generating a message we would refuse ourselves.
        assert!(path.encode().is_empty());
        assert!(AsPath::decode(&path.encode()).unwrap().is_empty());
    }

    #[test]
    fn test_leading_as_is_stricter_than_first_as() {
        let seq = AsPath::sequence(vec![65002, 65003]);
        assert_eq!(seq.leading_as(), Some(65002));
        assert_eq!(seq.first_as(), Some(65002));

        // A path that opens with an AS_SET has no leading AS at all, even though
        // first_as happily skips ahead to the sequence behind it to compare MEDs.
        let set_first = AsPath {
            segments: vec![
                AsPathSegment {
                    kind: AsPathSegmentKind::Set,
                    asns: vec![65010, 65011],
                },
                AsPathSegment {
                    kind: AsPathSegmentKind::Sequence,
                    asns: vec![65002],
                },
            ],
        };
        assert_eq!(set_first.leading_as(), None);
        assert_eq!(set_first.first_as(), Some(65002));

        assert_eq!(AsPath::empty().leading_as(), None);
    }
}
