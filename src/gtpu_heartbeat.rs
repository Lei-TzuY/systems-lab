//! 3GPP TS 29.281 Section 7.2 — GTP-U Path Management & Echo Heartbeat Protocol.
//!
//! GTP-U Path Management verifies the liveness and reachability of GTP-U
//! transport peers (e.g. between gNodeB, UPF, SGW, and ePDG) by periodically
//! exchanging Echo Request and Echo Response messages over UDP port 2152.
//!
//! This module implements:
//! * GTP-U Header framing for Echo Request (Type 1) and Echo Response (Type 2).
//! * Information Elements (IEs):
//!   - `Recovery` (IE Type 14): Contains an 8-bit restart counter to detect peer restarts.
//!   - `Private Extension` (IE Type 255).
//! * Path Health State Machine:
//!   - `Active`: Peer is responding normally.
//!   - `Degraded`: Missed 1 or more Echo Responses.
//!   - `Failed`: Reached $N3\text{-REQUESTS}$ retry limit; triggers path failover alarm.
//! * Peer Table and Sequence Number management.

use crate::ipv4::Ipv4Address;

pub const GTPU_PORT: u16 = 2152;
pub const GTPU_MSG_ECHO_REQUEST: u8 = 1;
pub const GTPU_MSG_ECHO_RESPONSE: u8 = 2;
pub const GTPU_IE_RECOVERY: u8 = 14;

/// Health State of a GTP-U transport path to a remote peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtpuPathState {
    Active,
    Degraded,
    Failed,
}

/// GTP-U Path Echo Message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpuEchoMessage {
    pub message_type: u8,
    pub sequence_number: u16,
    pub restart_counter: u8,
}

impl GtpuEchoMessage {
    /// Creates a new Echo Request.
    pub fn new_request(seq: u16, restart_counter: u8) -> Self {
        GtpuEchoMessage {
            message_type: GTPU_MSG_ECHO_REQUEST,
            sequence_number: seq,
            restart_counter,
        }
    }

    /// Creates a new Echo Response acknowledging a request.
    pub fn new_response(seq: u16, restart_counter: u8) -> Self {
        GtpuEchoMessage {
            message_type: GTPU_MSG_ECHO_RESPONSE,
            sequence_number: seq,
            restart_counter,
        }
    }

    /// Serializes the GTP-U Echo packet into bytes.
    /// Flags: Version 1 (0x20) | Protocol Type GTP (0x10) | Sequence flag (0x02) = 0x32
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        buf.push(0x32); // Flags: v1, GTP, Seq present
        buf.push(self.message_type);
        // Payload length: Sequence Number (2) + N-PDU (1) + Next Ext (1) + Recovery IE (2) = 6 bytes
        buf.extend_from_slice(&6u16.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // TEID = 0 for Path Management
        buf.extend_from_slice(&self.sequence_number.to_be_bytes());
        buf.push(0x00); // N-PDU Number
        buf.push(0x00); // Next Extension Header Type (None)

        // Recovery IE (Type 14, 1-byte value)
        buf.push(GTPU_IE_RECOVERY);
        buf.push(self.restart_counter);
        buf
    }

    /// Parses a GTP-U Echo packet from bytes.
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 12 {
            return Err("GTP-U packet too short for Path Management");
        }
        let flags = data[0];
        if (flags >> 5) != 1 {
            return Err("Unsupported GTP version, expected v1");
        }
        let message_type = data[1];
        if message_type != GTPU_MSG_ECHO_REQUEST && message_type != GTPU_MSG_ECHO_RESPONSE {
            return Err("Not a GTP-U Echo Request or Response message");
        }

        let seq = if flags & 0x02 != 0 {
            u16::from_be_bytes([data[8], data[9]])
        } else {
            0
        };

        let mut restart_counter = 0;
        if data.len() >= 14 && data[12] == GTPU_IE_RECOVERY {
            restart_counter = data[13];
        }

        Ok(GtpuEchoMessage {
            message_type,
            sequence_number: seq,
            restart_counter,
        })
    }
}

/// A tracked GTP-U Peer in the Path Management Table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GtpuPeerEntry {
    pub peer_ip: Ipv4Address,
    pub state: GtpuPathState,
    pub last_seq_sent: u16,
    pub unacked_probes: u8,
    pub max_retries: u8, // N3-REQUESTS (default 3)
    pub peer_restart_counter: Option<u8>,
    pub total_echo_requests_sent: u64,
    pub total_echo_responses_recv: u64,
    pub total_path_failures: u64,
}

impl GtpuPeerEntry {
    pub fn new(peer_ip: Ipv4Address, max_retries: u8) -> Self {
        GtpuPeerEntry {
            peer_ip,
            state: GtpuPathState::Active,
            last_seq_sent: 0,
            unacked_probes: 0,
            max_retries,
            peer_restart_counter: None,
            total_echo_requests_sent: 0,
            total_echo_responses_recv: 0,
            total_path_failures: 0,
        }
    }
}

/// GTP-U Path Management Engine managing peer heartbeats and path failure alarms.
#[derive(Debug, Clone, Default)]
pub struct GtpuPathEngine {
    pub local_restart_counter: u8,
    pub peers: Vec<GtpuPeerEntry>,
}

impl GtpuPathEngine {
    pub fn new(local_restart_counter: u8) -> Self {
        GtpuPathEngine {
            local_restart_counter,
            peers: Vec::new(),
        }
    }

    /// Registers a new peer for heartbeat tracking.
    pub fn add_peer(&mut self, peer_ip: Ipv4Address, max_retries: u8) {
        if !self.peers.iter().any(|p| p.peer_ip == peer_ip) {
            self.peers.push(GtpuPeerEntry::new(peer_ip, max_retries));
        }
    }

    /// Generates the next Echo Request for a given peer.
    pub fn send_echo_request(&mut self, peer_ip: Ipv4Address) -> Option<GtpuEchoMessage> {
        let restart = self.local_restart_counter;
        let peer = self.peers.iter_mut().find(|p| p.peer_ip == peer_ip)?;

        peer.last_seq_sent = peer.last_seq_sent.wrapping_add(1);
        peer.unacked_probes += 1;
        peer.total_echo_requests_sent += 1;

        if peer.unacked_probes > 1 && peer.unacked_probes <= peer.max_retries {
            peer.state = GtpuPathState::Degraded;
        } else if peer.unacked_probes > peer.max_retries {
            if peer.state != GtpuPathState::Failed {
                peer.state = GtpuPathState::Failed;
                peer.total_path_failures += 1;
            }
        }

        Some(GtpuEchoMessage::new_request(peer.last_seq_sent, restart))
    }

    /// Processes an incoming Echo Response from a peer.
    pub fn handle_echo_response(&mut self, peer_ip: Ipv4Address, resp: &GtpuEchoMessage) -> bool {
        if let Some(peer) = self.peers.iter_mut().find(|p| p.peer_ip == peer_ip) {
            if resp.sequence_number == peer.last_seq_sent {
                peer.unacked_probes = 0;
                peer.state = GtpuPathState::Active;
                peer.total_echo_responses_recv += 1;

                // Check for peer restart
                if let Some(prev) = peer.peer_restart_counter {
                    if resp.restart_counter != prev {
                        // Peer restarted! Update counter
                        peer.peer_restart_counter = Some(resp.restart_counter);
                    }
                } else {
                    peer.peer_restart_counter = Some(resp.restart_counter);
                }
                return true;
            }
        }
        false
    }

    /// Generates an Echo Response to an incoming Echo Request.
    pub fn handle_echo_request(&self, req: &GtpuEchoMessage) -> GtpuEchoMessage {
        GtpuEchoMessage::new_response(req.sequence_number, self.local_restart_counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gtpu_echo_packet_serialization_roundtrip() {
        let req = GtpuEchoMessage::new_request(0x1234, 42);
        let bytes = req.serialize();
        assert_eq!(bytes.len(), 14);

        let parsed = GtpuEchoMessage::parse(&bytes).unwrap();
        assert_eq!(parsed.message_type, GTPU_MSG_ECHO_REQUEST);
        assert_eq!(parsed.sequence_number, 0x1234);
        assert_eq!(parsed.restart_counter, 42);
    }

    #[test]
    fn test_gtpu_path_engine_heartbeat_flow() {
        let mut engine = GtpuPathEngine::new(1);
        let peer_ip = Ipv4Address::new(10, 100, 1, 50);
        engine.add_peer(peer_ip, 3);

        // 1. Send Echo Request
        let req = engine.send_echo_request(peer_ip).unwrap();
        assert_eq!(req.sequence_number, 1);
        assert_eq!(engine.peers[0].unacked_probes, 1);

        // 2. Peer replies with Echo Response
        let resp = GtpuEchoMessage::new_response(1, 10);
        assert!(engine.handle_echo_response(peer_ip, &resp));
        assert_eq!(engine.peers[0].state, GtpuPathState::Active);
        assert_eq!(engine.peers[0].unacked_probes, 0);
        assert_eq!(engine.peers[0].peer_restart_counter, Some(10));
    }

    #[test]
    fn test_gtpu_path_failure_detection() {
        let mut engine = GtpuPathEngine::new(1);
        let peer_ip = Ipv4Address::new(10, 100, 1, 99);
        engine.add_peer(peer_ip, 3);

        // Send 1st probe
        engine.send_echo_request(peer_ip);
        assert_eq!(engine.peers[0].state, GtpuPathState::Active);

        // Send 2nd probe without ack -> Degraded
        engine.send_echo_request(peer_ip);
        assert_eq!(engine.peers[0].state, GtpuPathState::Degraded);

        // Send 3rd probe
        engine.send_echo_request(peer_ip);
        assert_eq!(engine.peers[0].state, GtpuPathState::Degraded);

        // Send 4th probe -> exceeds max_retries 3 -> Failed!
        engine.send_echo_request(peer_ip);
        assert_eq!(engine.peers[0].state, GtpuPathState::Failed);
        assert_eq!(engine.peers[0].total_path_failures, 1);
    }
}
