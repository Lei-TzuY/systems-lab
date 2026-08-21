//! OpenFlow Switch Protocol Version 1.3 (ONF TS-025).
//!
//! SDN controller-to-switch programmable flow table management and packet processing over TCP port 6653.

use crate::ipv4::Ipv4Address;
use std::fmt;

pub const OFP_TCP_PORT: u16 = 6653;
pub const OFP_VERSION_1_3: u8 = 0x04;

// OpenFlow Message Types
pub const OFPT_HELLO: u8 = 0;
pub const OFPT_ERROR: u8 = 1;
pub const OFPT_ECHO_REQUEST: u8 = 2;
pub const OFPT_ECHO_REPLY: u8 = 3;
pub const OFPT_FEATURES_REQUEST: u8 = 5;
pub const OFPT_FEATURES_REPLY: u8 = 6;
pub const OFPT_PACKET_IN: u8 = 10;
pub const OFPT_FLOW_MOD: u8 = 14;
pub const OFPT_PACKET_OUT: u8 = 13;

// FlowMod Commands
pub const OFPFC_ADD: u8 = 0;
pub const OFPFC_MODIFY: u8 = 1;
pub const OFPFC_DELETE: u8 = 3;

// OpenFlow Actions
pub const OFPAT_OUTPUT: u16 = 0;
pub const OFPAT_SET_FIELD: u16 = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfpHeader {
    pub version: u8,
    pub msg_type: u8,
    pub length: u16,
    pub xid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OfpMatch {
    pub in_port: Option<u32>,
    pub eth_type: Option<u16>,
    pub ip_dst: Option<Ipv4Address>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfpAction {
    Output(u32), // port number
    SetVlan(u16), // VLAN ID
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfpFlowEntry {
    pub priority: u16,
    pub match_fields: OfpMatch,
    pub actions: Vec<OfpAction>,
    pub packet_count: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfpMessage {
    Hello { version_bitmap: u32 },
    EchoRequest { data: Vec<u8> },
    EchoReply { data: Vec<u8> },
    FeaturesRequest,
    FeaturesReply { datapath_id: u64, n_buffers: u32, n_tables: u8 },
    FlowMod {
        command: u8,
        priority: u16,
        match_fields: OfpMatch,
        actions: Vec<OfpAction>,
    },
    PacketIn {
        buffer_id: u32,
        total_len: u16,
        in_port: u32,
        data: Vec<u8>,
    },
    PacketOut {
        buffer_id: u32,
        in_port: u32,
        actions: Vec<OfpAction>,
        data: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfpError {
    PacketTooShort(usize),
    UnsupportedVersion(u8),
    InvalidLength,
}

impl fmt::Display for OfpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OfpError::PacketTooShort(l) => write!(f, "OpenFlow message too short ({} bytes)", l),
            OfpError::UnsupportedVersion(v) => write!(f, "Unsupported OpenFlow version: 0x{:02X}", v),
            OfpError::InvalidLength => write!(f, "Invalid OpenFlow message length"),
        }
    }
}

impl std::error::Error for OfpError {}

impl OfpMatch {
    pub fn matches(&self, in_port: u32, eth_type: u16, ip_dst: Option<Ipv4Address>) -> bool {
        if let Some(p) = self.in_port {
            if p != in_port {
                return false;
            }
        }
        if let Some(et) = self.eth_type {
            if et != eth_type {
                return false;
            }
        }
        if let Some(target_dst) = self.ip_dst {
            if let Some(dst) = ip_dst {
                if dst != target_dst {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct OfpFlowTable {
    pub entries: Vec<OfpFlowEntry>,
}

impl OfpFlowTable {
    pub fn new() -> Self {
        OfpFlowTable { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, priority: u16, match_fields: OfpMatch, actions: Vec<OfpAction>) {
        self.entries.push(OfpFlowEntry {
            priority,
            match_fields,
            actions,
            packet_count: 0,
            byte_count: 0,
        });
        // Sort highest priority first
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn lookup_and_execute(&mut self, in_port: u32, eth_type: u16, ip_dst: Option<Ipv4Address>, pkt_len: usize) -> Option<Vec<OfpAction>> {
        for entry in &mut self.entries {
            if entry.match_fields.matches(in_port, eth_type, ip_dst) {
                entry.packet_count += 1;
                entry.byte_count += pkt_len as u64;
                return Some(entry.actions.clone());
            }
        }
        None
    }
}

impl OfpMessage {
    pub fn build_hello(xid: u32) -> (OfpHeader, Self) {
        let hdr = OfpHeader {
            version: OFP_VERSION_1_3,
            msg_type: OFPT_HELLO,
            length: 16,
            xid,
        };
        (hdr, OfpMessage::Hello { version_bitmap: 0x00000010 })
    }

    pub fn build_features_reply(xid: u32, datapath_id: u64) -> (OfpHeader, Self) {
        let hdr = OfpHeader {
            version: OFP_VERSION_1_3,
            msg_type: OFPT_FEATURES_REPLY,
            length: 32,
            xid,
        };
        (hdr, OfpMessage::FeaturesReply {
            datapath_id,
            n_buffers: 256,
            n_tables: 64,
        })
    }

    pub fn serialize(&self, hdr: &OfpHeader) -> Vec<u8> {
        let mut body = Vec::new();
        match self {
            OfpMessage::Hello { version_bitmap } => {
                body.extend_from_slice(&1u16.to_be_bytes()); // Element type 1 = VersionBitmap
                body.extend_from_slice(&8u16.to_be_bytes()); // Length 8
                body.extend_from_slice(&version_bitmap.to_be_bytes());
            }
            OfpMessage::FeaturesReply { datapath_id, n_buffers, n_tables } => {
                body.extend_from_slice(&datapath_id.to_be_bytes());
                body.extend_from_slice(&n_buffers.to_be_bytes());
                body.push(*n_tables);
                body.push(0); // Aux ID
                body.extend_from_slice(&[0, 0]); // Pad
                body.extend_from_slice(&0x0000004F_u32.to_be_bytes()); // Capabilities
                body.extend_from_slice(&0u32.to_be_bytes()); // Reserved
            }
            OfpMessage::EchoRequest { data } | OfpMessage::EchoReply { data } => {
                body.extend_from_slice(data);
            }
            _ => {}
        }

        let total_len = (8 + body.len()) as u16;
        let mut buf = Vec::new();
        buf.push(hdr.version);
        buf.push(hdr.msg_type);
        buf.extend_from_slice(&total_len.to_be_bytes());
        buf.extend_from_slice(&hdr.xid.to_be_bytes());
        buf.extend_from_slice(&body);
        buf
    }

    pub fn parse(data: &[u8]) -> Result<(OfpHeader, Self), OfpError> {
        if data.len() < 8 {
            return Err(OfpError::PacketTooShort(data.len()));
        }

        let version = data[0];
        if version != OFP_VERSION_1_3 {
            return Err(OfpError::UnsupportedVersion(version));
        }

        let msg_type = data[1];
        let length = u16::from_be_bytes([data[2], data[3]]);
        let xid = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        let header = OfpHeader {
            version,
            msg_type,
            length,
            xid,
        };

        let body = if data.len() >= length as usize {
            &data[8..length as usize]
        } else {
            &data[8..]
        };

        let msg = match msg_type {
            OFPT_HELLO => {
                let bmp = if body.len() >= 8 {
                    u32::from_be_bytes([body[4], body[5], body[6], body[7]])
                } else {
                    0x10
                };
                OfpMessage::Hello { version_bitmap: bmp }
            }
            OFPT_FEATURES_REQUEST => OfpMessage::FeaturesRequest,
            OFPT_FEATURES_REPLY if body.len() >= 16 => {
                let dpid = u64::from_be_bytes([
                    body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7],
                ]);
                let n_buf = u32::from_be_bytes([body[8], body[9], body[10], body[11]]);
                let n_tab = body[12];
                OfpMessage::FeaturesReply {
                    datapath_id: dpid,
                    n_buffers: n_buf,
                    n_tables: n_tab,
                }
            }
            OFPT_ECHO_REQUEST => OfpMessage::EchoRequest { data: body.to_vec() },
            OFPT_ECHO_REPLY => OfpMessage::EchoReply { data: body.to_vec() },
            _ => OfpMessage::EchoRequest { data: body.to_vec() },
        };

        Ok((header, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openflow_flow_table_matching_and_actions() {
        let mut table = OfpFlowTable::new();

        // Rule 1: Port 1, IPv4 to 10.0.0.5 -> Output Port 2
        table.add_entry(
            100,
            OfpMatch {
                in_port: Some(1),
                eth_type: Some(0x0800),
                ip_dst: Some(Ipv4Address::new(10, 0, 0, 5)),
            },
            vec![OfpAction::Output(2)],
        );

        // Rule 2: Low priority default drop
        table.add_entry(0, OfpMatch::default(), vec![OfpAction::Drop]);

        // Match Rule 1
        let act1 = table.lookup_and_execute(1, 0x0800, Some(Ipv4Address::new(10, 0, 0, 5)), 64);
        assert_eq!(act1, Some(vec![OfpAction::Output(2)]));
        assert_eq!(table.entries[0].packet_count, 1);
        assert_eq!(table.entries[0].byte_count, 64);

        // Match Rule 2 (Default Drop)
        let act2 = table.lookup_and_execute(1, 0x86DD, None, 128);
        assert_eq!(act2, Some(vec![OfpAction::Drop]));
    }

    #[test]
    fn test_openflow_hello_and_features_codec() {
        let (hdr, hello) = OfpMessage::build_hello(0x12345678);
        let raw = hello.serialize(&hdr);

        let (parsed_hdr, parsed_msg) = OfpMessage::parse(&raw).unwrap();
        assert_eq!(parsed_hdr.version, OFP_VERSION_1_3);
        assert_eq!(parsed_hdr.msg_type, OFPT_HELLO);
        assert_eq!(parsed_hdr.xid, 0x12345678);

        if let OfpMessage::Hello { version_bitmap } = parsed_msg {
            assert_eq!(version_bitmap, 0x10);
        } else {
            panic!("Expected OFPT_HELLO");
        }
    }
}
