//! Spanning Tree Protocol (STP - IEEE 802.1D).
//!
//! Layer 2 loop prevention, Bridge Protocol Data Unit (BPDU) framing,
//! Root Bridge election, and port state machine (Blocking, Listening, Learning, Forwarding).

use crate::ethernet::MacAddress;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

pub const STP_MULTICAST_MAC: MacAddress = MacAddress([0x01, 0x80, 0xC2, 0x00, 0x00, 0x00]);
pub const STP_PROTOCOL_ID: u16 = 0x0000;
pub const STP_VERSION_ID: u8 = 0x00;
pub const STP_BPDU_CONFIG: u8 = 0x00;
pub const STP_BPDU_TCN: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeId {
    pub priority: u16,
    pub mac: MacAddress,
}

impl BridgeId {
    pub fn new(priority: u16, mac: MacAddress) -> Self {
        BridgeId { priority, mac }
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[0..2].copy_from_slice(&self.priority.to_be_bytes());
        b[2..8].copy_from_slice(&self.mac.0);
        b
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let priority = u16::from_be_bytes([b[0], b[1]]);
        let mac = MacAddress([b[2], b[3], b[4], b[5], b[6], b[7]]);
        BridgeId { priority, mac }
    }
}

impl Ord for BridgeId {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.mac.0.cmp(&other.mac.0),
            ord => ord,
        }
    }
}

impl PartialOrd for BridgeId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for BridgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.priority, self.mac)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StpPortRole {
    RootPort,
    DesignatedPort,
    BlockedPort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StpPortState {
    Blocking,
    Listening,
    Learning,
    Forwarding,
}

impl fmt::Display for StpPortState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StpPortState::Blocking => write!(f, "BLOCKING"),
            StpPortState::Listening => write!(f, "LISTENING"),
            StpPortState::Learning => write!(f, "LEARNING"),
            StpPortState::Forwarding => write!(f, "FORWARDING"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StpBpdu {
    pub protocol_id: u16,
    pub version: u8,
    pub bpdu_type: u8,
    pub flags: u8,
    pub root_id: BridgeId,
    pub root_path_cost: u32,
    pub bridge_id: BridgeId,
    pub port_id: u16,
    pub message_age: u16,
    pub max_age: u16,
    pub hello_time: u16,
    pub forward_delay: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StpError {
    PacketTooShort(usize),
    InvalidProtocol(u16),
}

impl fmt::Display for StpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StpError::PacketTooShort(l) => write!(f, "STP BPDU too short ({} bytes, min 35)", l),
            StpError::InvalidProtocol(p) => {
                write!(f, "Invalid STP protocol identifier: 0x{:04x}", p)
            }
        }
    }
}

impl std::error::Error for StpError {}

impl StpBpdu {
    pub fn parse(data: &[u8]) -> Result<Self, StpError> {
        if data.len() < 35 {
            return Err(StpError::PacketTooShort(data.len()));
        }

        let protocol_id = u16::from_be_bytes([data[0], data[1]]);
        if protocol_id != STP_PROTOCOL_ID {
            return Err(StpError::InvalidProtocol(protocol_id));
        }

        let version = data[2];
        let bpdu_type = data[3];
        let flags = data[4];

        let root_id = BridgeId::from_bytes(&data[5..13]);
        let root_path_cost = u32::from_be_bytes([data[13], data[14], data[15], data[16]]);
        let bridge_id = BridgeId::from_bytes(&data[17..25]);
        let port_id = u16::from_be_bytes([data[25], data[26]]);
        let message_age = u16::from_be_bytes([data[27], data[28]]);
        let max_age = u16::from_be_bytes([data[29], data[30]]);
        let hello_time = u16::from_be_bytes([data[31], data[32]]);
        let forward_delay = u16::from_be_bytes([data[33], data[34]]);

        Ok(StpBpdu {
            protocol_id,
            version,
            bpdu_type,
            flags,
            root_id,
            root_path_cost,
            bridge_id,
            port_id,
            message_age,
            max_age,
            hello_time,
            forward_delay,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 35];
        buf[0..2].copy_from_slice(&self.protocol_id.to_be_bytes());
        buf[2] = self.version;
        buf[3] = self.bpdu_type;
        buf[4] = self.flags;
        buf[5..13].copy_from_slice(&self.root_id.to_bytes());
        buf[13..17].copy_from_slice(&self.root_path_cost.to_be_bytes());
        buf[17..25].copy_from_slice(&self.bridge_id.to_bytes());
        buf[25..27].copy_from_slice(&self.port_id.to_be_bytes());
        buf[27..29].copy_from_slice(&self.message_age.to_be_bytes());
        buf[29..31].copy_from_slice(&self.max_age.to_be_bytes());
        buf[31..33].copy_from_slice(&self.hello_time.to_be_bytes());
        buf[33..35].copy_from_slice(&self.forward_delay.to_be_bytes());
        buf
    }

    pub fn build_config_bpdu(
        bridge_id: BridgeId,
        root_id: BridgeId,
        cost: u32,
        port_id: u16,
    ) -> Self {
        StpBpdu {
            protocol_id: STP_PROTOCOL_ID,
            version: STP_VERSION_ID,
            bpdu_type: STP_BPDU_CONFIG,
            flags: 0,
            root_id,
            root_path_cost: cost,
            bridge_id,
            port_id,
            message_age: 0,
            max_age: 20 * 256,
            hello_time: 2 * 256,
            forward_delay: 15 * 256,
        }
    }
}

/// Spanning Tree Switch Bridge Engine
pub struct StpBridgeEngine {
    pub bridge_id: BridgeId,
    pub root_id: BridgeId,
    pub root_path_cost: u32,
    pub root_port: Option<u8>,
    pub port_states: HashMap<u8, (StpPortRole, StpPortState)>,
}

impl StpBridgeEngine {
    pub fn new(priority: u16, mac: MacAddress) -> Self {
        let bridge_id = BridgeId::new(priority, mac);
        let mut port_states = HashMap::new();
        port_states.insert(1, (StpPortRole::DesignatedPort, StpPortState::Forwarding));
        port_states.insert(2, (StpPortRole::DesignatedPort, StpPortState::Forwarding));

        StpBridgeEngine {
            bridge_id,
            root_id: bridge_id,
            root_path_cost: 0,
            root_port: None,
            port_states,
        }
    }

    pub fn process_bpdu(&mut self, port: u8, bpdu: &StpBpdu) -> bool {
        if bpdu.root_id < self.root_id {
            // New superior root discovered!
            self.root_id = bpdu.root_id;
            self.root_path_cost = bpdu.root_path_cost + 19; // Standard 100Mbps link cost
            self.root_port = Some(port);

            // Update port roles
            self.port_states
                .insert(port, (StpPortRole::RootPort, StpPortState::Forwarding));
            true
        } else if bpdu.root_id == self.root_id
            && bpdu.bridge_id < self.bridge_id
            && self.root_port != Some(port)
        {
            // Alternate link with higher bridge -> Block to prevent loop!
            self.port_states
                .insert(port, (StpPortRole::BlockedPort, StpPortState::Blocking));
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stp_bpdu_roundtrip() {
        let root = BridgeId::new(4096, MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));
        let sender = BridgeId::new(32768, MacAddress([0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]));
        let bpdu = StpBpdu::build_config_bpdu(sender, root, 19, 0x8001);

        let raw = bpdu.serialize();
        assert_eq!(raw.len(), 35);

        let parsed = StpBpdu::parse(&raw).unwrap();
        assert_eq!(parsed.root_id, root);
        assert_eq!(parsed.bridge_id, sender);
        assert_eq!(parsed.root_path_cost, 19);
        assert_eq!(parsed.port_id, 0x8001);
    }

    #[test]
    fn test_stp_root_election_and_port_blocking() {
        let mut bridge =
            StpBridgeEngine::new(32768, MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x02]));
        let root_candidate = BridgeId::new(4096, MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x01]));

        let bpdu = StpBpdu::build_config_bpdu(root_candidate, root_candidate, 0, 0x8001);
        bridge.process_bpdu(1, &bpdu);

        assert_eq!(bridge.root_id, root_candidate);
        assert_eq!(bridge.root_path_cost, 19);
        assert_eq!(
            bridge.port_states.get(&1),
            Some(&(StpPortRole::RootPort, StpPortState::Forwarding))
        );
    }
}
