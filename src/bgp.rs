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
                        let total_attr_len = u16::from_be_bytes([body[attr_offset], body[attr_offset + 1]]) as usize;
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
                                                    as_path.push(u16::from_be_bytes([body[offset], body[offset + 1]]));
                                                }
                                            }
                                        }
                                    }
                                    BGP_ATTR_NEXT_HOP => {
                                        if attr_len == 4 {
                                            next_hop = Ipv4Address([body[val_start], body[val_start + 1], body[val_start + 2], body[val_start + 3]]);
                                        }
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
                                nlri_prefix = Ipv4Address([body[attr_end + 1], body[attr_end + 2], body[attr_end + 3], body[attr_end + 4]]);
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

    pub fn build_update(prefix: Ipv4Address, mask: u8, next_hop: Ipv4Address, as_path: Vec<u16>) -> Self {
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
        rib.insert(Ipv4Address::new(8, 8, 8, 0), 24, Ipv4Address::new(198, 51, 100, 1), vec![65001, 15169]);
        rib.insert(Ipv4Address::new(1, 1, 1, 0), 24, Ipv4Address::new(203, 0, 113, 1), vec![65001, 13335]);
        rib
    }

    pub fn insert(&mut self, prefix: Ipv4Address, mask: u8, next_hop: Ipv4Address, as_path: Vec<u16>) {
        self.routes.insert((prefix, mask), (next_hop, as_path));
    }

    pub fn all_routes(&self) -> &HashMap<(Ipv4Address, u8), (Ipv4Address, Vec<u16>)> {
        &self.routes
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
        if let BgpMessage::Open { my_as, hold_time, bgp_id, .. } = parsed {
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
    fn test_bgp_update_nlri_and_as_path() {
        let update = BgpMessage::build_update(
            Ipv4Address::new(172, 16, 0, 0),
            16,
            Ipv4Address::new(192, 168, 1, 1),
            vec![65001, 65002, 65003],
        );
        let raw = update.serialize();
        let parsed = BgpMessage::parse(&raw).unwrap();

        if let BgpMessage::Update { as_path, next_hop, nlri_prefix, nlri_mask } = parsed {
            assert_eq!(as_path, vec![65001, 65002, 65003]);
            assert_eq!(next_hop, Ipv4Address::new(192, 168, 1, 1));
            assert_eq!(nlri_prefix, Ipv4Address::new(172, 16, 0, 0));
            assert_eq!(nlri_mask, 16);
        } else {
            panic!("Expected Update message");
        }
    }
}
