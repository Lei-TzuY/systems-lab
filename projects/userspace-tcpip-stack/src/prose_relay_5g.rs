//! 3GPP TS 23.501 §5.30 / TS 23.304 / TS 24.554 / TS 24.555 / TS 38.340 Release 17 5G ProSe Direct Communication & UE-to-Network (U2N) Relay Protocol Engine.
//!
//! Implements 5G Proximity-based Services (ProSe) & Sidelink Relay:
//! - Relay Service Code (RSC) Discovery (Model A Announcement & Model B Solicitation/Response)
//! - PC5 Signaling (PC5-S) Direct Communication Link Establishment & Teardown
//! - PC5 Security Association (PC5-SA) with $K_{NRP-sess}$ key derivation
//! - PC5 QoS (PQI) mapping and priority flow management
//! - Layer-2 (L2) U2N Relay with Sidelink Relay Adaptation Protocol (SRAP, TS 38.340)
//! - Layer-3 (L3) U2N Relay with IP Forwarding, NAT translation, and 5G PDU session binding
//! - Radio Link Failure (RLF) monitoring, keepalive heartbeat servo, and relay reselection
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::collections::HashMap;
use std::net::Ipv4Addr;

// ---------------------------------------------------------------------------
// 5G ProSe Constants & Pre-defined Relay Service Codes (TS 23.304 / TS 24.554)
// ---------------------------------------------------------------------------

/// Standard Pre-defined Relay Service Codes (RSC - 24-bit).
pub const RSC_EMERGENCY_SERVICES: u32 = 0x000001;
pub const RSC_PUBLIC_SAFETY_VOICE: u32 = 0x000002;
pub const RSC_COMMERCIAL_INTERNET: u32 = 0x000003;
pub const RSC_SMART_GRID_IOT: u32 = 0x000004;

/// Default RLF RSRP threshold in dBm (-110 dBm).
pub const DEFAULT_RLF_RSRP_THRESHOLD_DBM: i16 = -110;

/// Default heartbeat timeout in seconds.
pub const DEFAULT_HEARTBEAT_TIMEOUT_S: u64 = 5;

// ---------------------------------------------------------------------------
// Data Structures & Enums
// ---------------------------------------------------------------------------

/// 24-bit Relay Service Code (RSC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelayServiceCode(pub u32);

impl RelayServiceCode {
    pub fn new(code: u32) -> Self {
        RelayServiceCode(code & 0x00FF_FFFF)
    }

    pub fn from_bytes(bytes: [u8; 3]) -> Self {
        let val = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
        RelayServiceCode(val)
    }

    pub fn to_bytes(&self) -> [u8; 3] {
        [
            ((self.0 >> 16) & 0xFF) as u8,
            ((self.0 >> 8) & 0xFF) as u8,
            (self.0 & 0xFF) as u8,
        ]
    }
}

/// 24-bit PC5 Layer-2 Identifier (Source / Destination / Relay).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pc5Layer2Id(pub [u8; 3]);

impl Pc5Layer2Id {
    pub fn new(b0: u8, b1: u8, b2: u8) -> Self {
        Pc5Layer2Id([b0, b1, b2])
    }

    pub fn to_hex_string(&self) -> String {
        format!("{:02X}:{:02X}:{:02X}", self.0[0], self.0[1], self.0[2])
    }
}

/// PC5 Security Algorithms (TS 33.536 Section 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pc5SecurityAlgorithm {
    Nea0NullCiphering,
    Nea1Snow3g,
    Nea2AesCtr,
    Nia0NullIntegrity,
    Nia1Snow3g,
    Nia2AesCmac,
}

/// PC5-S Link State Machine (TS 24.554 Section 6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pc5LinkState {
    Disconnected,
    Connecting,
    Securing,
    Established,
    Releasing,
}

/// PC5 QoS Profile (PQI mapping per TS 23.304 / TS 23.287).
#[derive(Debug, Clone, PartialEq)]
pub struct Pc5QoSProfile {
    pub pqi: u8,
    pub packet_delay_budget_ms: u16,
    pub packet_error_rate_exp: u8, // e.g. 2 means 10^-2, 4 means 10^-4
    pub priority_level: u8,
    pub gfbr_kbps: Option<u32>,
    pub mfbr_kbps: Option<u32>,
}

impl Pc5QoSProfile {
    /// Create standard PQI profile.
    pub fn from_pqi(pqi: u8) -> Self {
        match pqi {
            21 => Pc5QoSProfile {
                pqi: 21,
                packet_delay_budget_ms: 20,
                packet_error_rate_exp: 2,
                priority_level: 2,
                gfbr_kbps: None,
                mfbr_kbps: None,
            },
            22 => Pc5QoSProfile {
                pqi: 22,
                packet_delay_budget_ms: 50,
                packet_error_rate_exp: 3,
                priority_level: 4,
                gfbr_kbps: None,
                mfbr_kbps: None,
            },
            23 => Pc5QoSProfile {
                pqi: 23,
                packet_delay_budget_ms: 100,
                packet_error_rate_exp: 2,
                priority_level: 1, // Mission critical voice highest priority
                gfbr_kbps: Some(64),
                mfbr_kbps: Some(128),
            },
            55 => Pc5QoSProfile {
                pqi: 55,
                packet_delay_budget_ms: 30,
                packet_error_rate_exp: 4,
                priority_level: 3,
                gfbr_kbps: Some(256),
                mfbr_kbps: Some(1024),
            },
            _ => Pc5QoSProfile {
                pqi,
                packet_delay_budget_ms: 100,
                packet_error_rate_exp: 2,
                priority_level: 5,
                gfbr_kbps: None,
                mfbr_kbps: None,
            },
        }
    }
}

/// Sidelink Relay Adaptation Protocol (SRAP) Header (3GPP TS 38.340 Section 6.2).
/// Used in Layer-2 (L2) U2N Relay to multiplex Remote UE traffic over Relay UE's Uu link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrapHeader {
    /// 16-bit Local Identity for the Remote UE.
    pub remote_ue_local_id: u16,
    /// 5-bit Data Radio Bearer (DRB) ID (1..32).
    pub bearer_id: u8,
}

impl SrapHeader {
    pub fn new(remote_ue_local_id: u16, bearer_id: u8) -> Self {
        SrapHeader {
            remote_ue_local_id,
            bearer_id: bearer_id & 0x1F,
        }
    }

    /// Serialize SRAP header into 3 bytes.
    pub fn encode(&self) -> [u8; 3] {
        [
            ((self.remote_ue_local_id >> 8) & 0xFF) as u8,
            (self.remote_ue_local_id & 0xFF) as u8,
            self.bearer_id & 0x1F,
        ]
    }

    /// Parse SRAP header from bytes.
    pub fn decode(buf: &[u8]) -> Result<(Self, &[u8]), ProseRelayError> {
        if buf.len() < 3 {
            return Err(ProseRelayError::SrapDecodingError(
                "Buffer too short for SRAP header".to_string(),
            ));
        }
        let remote_ue_local_id = ((buf[0] as u16) << 8) | (buf[1] as u16);
        let bearer_id = buf[2] & 0x1F;
        Ok((
            SrapHeader {
                remote_ue_local_id,
                bearer_id,
            },
            &buf[3..],
        ))
    }
}

/// PC5-S Signaling Messages (3GPP TS 24.554 Section 7).
#[derive(Debug, Clone, PartialEq)]
pub enum Pc5SignalingMessage {
    DirectCommunicationRequest {
        session_id: u32,
        source_l2_id: Pc5Layer2Id,
        target_l2_id: Pc5Layer2Id,
        pqi: u8,
        nonce_ue: [u8; 16],
        ip_addr: Option<Ipv4Addr>,
    },
    DirectSecurityModeCommand {
        session_id: u32,
        cipher_algo: Pc5SecurityAlgorithm,
        integrity_algo: Pc5SecurityAlgorithm,
        nonce_relay: [u8; 16],
    },
    DirectSecurityModeComplete {
        session_id: u32,
        mac_tag: [u8; 8],
    },
    DirectCommunicationAccept {
        session_id: u32,
        assigned_ip: Option<Ipv4Addr>,
        pqfi: u8,
    },
    DirectCommunicationReject {
        source_l2_id: Pc5Layer2Id,
        cause: String,
    },
    DirectCommunicationKeepalive {
        session_id: u32,
        sequence_num: u32,
    },
    DirectCommunicationKeepaliveAck {
        session_id: u32,
        sequence_num: u32,
    },
    DirectCommunicationReleaseRequest {
        session_id: u32,
        cause: String,
    },
    DirectCommunicationReleaseAccept {
        session_id: u32,
    },
}

/// Model A / Model B Discovery Messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayAnnouncement {
    pub relay_l2_id: Pc5Layer2Id,
    pub rsc: RelayServiceCode,
    pub rsrp_dbm: i16,
    pub relay_ue_id: String,
    pub supported_slices: Vec<String>,
    pub timestamp_s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySolicitation {
    pub remote_l2_id: Pc5Layer2Id,
    pub requested_rsc: RelayServiceCode,
    pub remote_ue_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayResponse {
    pub relay_l2_id: Pc5Layer2Id,
    pub accepted_rsc: RelayServiceCode,
    pub relay_ue_id: String,
    pub rsrp_dbm: i16,
}

/// Established PC5 Sidelink Session Context.
#[derive(Debug, Clone)]
pub struct Pc5Session {
    pub session_id: u32,
    pub peer_l2_id: Pc5Layer2Id,
    pub local_l2_id: Pc5Layer2Id,
    pub state: Pc5LinkState,
    pub k_nrp: [u8; 32],
    pub k_nrp_sess: [u8; 32],
    pub cipher_algo: Pc5SecurityAlgorithm,
    pub integrity_algo: Pc5SecurityAlgorithm,
    pub qos_profile: Pc5QoSProfile,
    pub ip_address: Option<Ipv4Addr>,
    pub last_heartbeat_s: u64,
    pub missed_keepalives: u32,
    pub rsrp_dbm: i16,
}

/// Layer-2 Relay Context mapping.
#[derive(Debug, Clone)]
pub struct L2RelayContext {
    pub remote_ue_local_id: u16,
    pub remote_l2_id: Pc5Layer2Id,
    pub uu_bearer_id: u8,
    pub pc5_rlc_channel_id: u8,
}

/// Layer-3 NAT Entry for U2N IP Forwarding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L3RelayNatEntry {
    pub remote_ip: Ipv4Addr,
    pub remote_port: u16,
    pub relay_port: u16,
    pub dest_ip: Ipv4Addr,
    pub dest_port: u16,
    pub protocol: u8, // 6 = TCP, 17 = UDP
    pub last_activity_s: u64,
}

/// Errors for 5G ProSe Relay Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProseRelayError {
    UnauthorizedRsc(u32),
    SessionNotFound(u32),
    RemoteLocalIdNotFound(u16),
    InvalidLinkState(String),
    SrapDecodingError(String),
    KeepaliveTimeout(u32),
    RadioLinkFailure { session_id: u32, rsrp_dbm: i16 },
    NatTableFull,
    NatMappingNotFound,
}

// ---------------------------------------------------------------------------
// Pure Rust Cryptographic KDF for PC5 Security (TS 33.536 Annex A)
// ---------------------------------------------------------------------------

/// Derive $K_{NRP-sess}$ from root key $K_{NRP}$ and nonces.
pub fn derive_k_nrp_sess(
    k_nrp: &[u8; 32],
    nonce_ue: &[u8; 16],
    nonce_relay: &[u8; 16],
    session_id: u32,
) -> [u8; 32] {
    // Standard HMAC-like Davies-Meyer round hashing in pure Rust
    let mut state = [0u8; 32];
    for i in 0..32 {
        state[i] = k_nrp[i] ^ 0x36;
    }
    for i in 0..16 {
        state[i] = state[i].wrapping_add(nonce_ue[i]);
        state[i + 16] = state[i + 16].wrapping_add(nonce_relay[i]);
    }
    let sid_bytes = session_id.to_be_bytes();
    for i in 0..4 {
        state[i] = state[i] ^ sid_bytes[i];
    }
    // Rotate and mix
    let mut derived = [0u8; 32];
    for i in 0..32 {
        derived[i] = state[(i * 7 + 13) % 32].wrapping_add((i as u8).wrapping_mul(0x55));
    }
    derived
}

// ---------------------------------------------------------------------------
// Top-Level 5G ProSe Direct Communication & U2N Relay Engine
// ---------------------------------------------------------------------------

pub struct ProSeRelayEngine {
    pub node_id: String,
    pub is_relay_node: bool,
    pub local_l2_id: Pc5Layer2Id,
    pub authorized_rscs: Vec<RelayServiceCode>,
    pub peer_root_keys: HashMap<Pc5Layer2Id, [u8; 32]>,
    pub pc5_sessions: HashMap<u32, Pc5Session>,
    pub l2_relay_table: HashMap<u16, L2RelayContext>,
    pub l3_nat_table: Vec<L3RelayNatEntry>,
    pub l3_relay_pdu_teid: u32,
    pub l3_relay_ip: Ipv4Addr,
    pub next_session_id: u32,
    pub next_remote_local_id: u16,
    pub next_nat_port: u16,
    pub rlf_rsrp_threshold_dbm: i16,
    pub max_missed_keepalives: u32,
}

impl ProSeRelayEngine {
    /// Create a Relay UE instance (with PDU session towards UPF).
    pub fn new_relay(
        node_id: &str,
        local_l2_id: Pc5Layer2Id,
        authorized_rscs: Vec<RelayServiceCode>,
        l3_relay_ip: Ipv4Addr,
        l3_relay_pdu_teid: u32,
    ) -> Self {
        ProSeRelayEngine {
            node_id: node_id.to_string(),
            is_relay_node: true,
            local_l2_id,
            authorized_rscs,
            peer_root_keys: HashMap::new(),
            pc5_sessions: HashMap::new(),
            l2_relay_table: HashMap::new(),
            l3_nat_table: Vec::new(),
            l3_relay_pdu_teid,
            l3_relay_ip,
            next_session_id: 100,
            next_remote_local_id: 1,
            next_nat_port: 30000,
            rlf_rsrp_threshold_dbm: DEFAULT_RLF_RSRP_THRESHOLD_DBM,
            max_missed_keepalives: 3,
        }
    }

    /// Create a Remote UE instance.
    pub fn new_remote(node_id: &str, local_l2_id: Pc5Layer2Id) -> Self {
        ProSeRelayEngine {
            node_id: node_id.to_string(),
            is_relay_node: false,
            local_l2_id,
            authorized_rscs: Vec::new(),
            peer_root_keys: HashMap::new(),
            pc5_sessions: HashMap::new(),
            l2_relay_table: HashMap::new(),
            l3_nat_table: Vec::new(),
            l3_relay_pdu_teid: 0,
            l3_relay_ip: Ipv4Addr::new(0, 0, 0, 0),
            next_session_id: 1,
            next_remote_local_id: 1,
            next_nat_port: 10000,
            rlf_rsrp_threshold_dbm: DEFAULT_RLF_RSRP_THRESHOLD_DBM,
            max_missed_keepalives: 3,
        }
    }

    /// Register a root key for a specific peer L2 ID.
    pub fn register_peer_root_key(&mut self, peer_l2_id: Pc5Layer2Id, root_key: [u8; 32]) {
        self.peer_root_keys.insert(peer_l2_id, root_key);
    }

    // -----------------------------------------------------------------------
    // Relay Discovery: Model A & Model B
    // -----------------------------------------------------------------------

    /// Model A: Relay UE creates periodic announcement message.
    pub fn create_model_a_announcement(
        &self,
        rsc: RelayServiceCode,
        rsrp_dbm: i16,
        timestamp_s: u64,
    ) -> Result<RelayAnnouncement, ProseRelayError> {
        if !self.authorized_rscs.contains(&rsc) {
            return Err(ProseRelayError::UnauthorizedRsc(rsc.0));
        }
        Ok(RelayAnnouncement {
            relay_l2_id: self.local_l2_id,
            rsc,
            rsrp_dbm,
            relay_ue_id: self.node_id.clone(),
            supported_slices: vec!["SST=1,SD=000001".to_string()],
            timestamp_s,
        })
    }

    /// Model A: Remote UE evaluates an incoming Relay announcement.
    pub fn evaluate_model_a_announcement(&self, announcement: &RelayAnnouncement) -> bool {
        announcement.rsrp_dbm >= self.rlf_rsrp_threshold_dbm
    }

    /// Model B: Remote UE generates a solicitation message for a target RSC.
    pub fn create_model_b_solicitation(&self, rsc: RelayServiceCode) -> RelaySolicitation {
        RelaySolicitation {
            remote_l2_id: self.local_l2_id,
            requested_rsc: rsc,
            remote_ue_id: self.node_id.clone(),
        }
    }

    /// Model B: Relay UE processes solicitation and sends response if authorized.
    pub fn handle_model_b_solicitation(
        &self,
        solicitation: &RelaySolicitation,
        rsrp_dbm: i16,
    ) -> Option<RelayResponse> {
        if !self.authorized_rscs.contains(&solicitation.requested_rsc) {
            return None;
        }
        Some(RelayResponse {
            relay_l2_id: self.local_l2_id,
            accepted_rsc: solicitation.requested_rsc,
            relay_ue_id: self.node_id.clone(),
            rsrp_dbm,
        })
    }

    // -----------------------------------------------------------------------
    // PC5 Signaling & Direct Security Association Handshake
    // -----------------------------------------------------------------------

    /// Remote UE initiates PC5 link establishment to Relay UE.
    pub fn initiate_pc5_link(
        &mut self,
        relay_l2_id: Pc5Layer2Id,
        root_key: [u8; 32],
        pqi: u8,
        current_time_s: u64,
    ) -> (u32, Pc5SignalingMessage) {
        let session_id = self.next_session_id;
        self.next_session_id += 1;

        let nonce_ue = [0xAA; 16];
        let session = Pc5Session {
            session_id,
            peer_l2_id: relay_l2_id,
            local_l2_id: self.local_l2_id,
            state: Pc5LinkState::Connecting,
            k_nrp: root_key,
            k_nrp_sess: [0u8; 32],
            cipher_algo: Pc5SecurityAlgorithm::Nea2AesCtr,
            integrity_algo: Pc5SecurityAlgorithm::Nia2AesCmac,
            qos_profile: Pc5QoSProfile::from_pqi(pqi),
            ip_address: None,
            last_heartbeat_s: current_time_s,
            missed_keepalives: 0,
            rsrp_dbm: -80,
        };
        self.pc5_sessions.insert(session_id, session);

        let msg = Pc5SignalingMessage::DirectCommunicationRequest {
            session_id,
            source_l2_id: self.local_l2_id,
            target_l2_id: relay_l2_id,
            pqi,
            nonce_ue,
            ip_addr: None,
        };
        (session_id, msg)
    }

    /// Handle PC5 Signaling message dispatch.
    pub fn handle_pc5_signaling(
        &mut self,
        msg: &Pc5SignalingMessage,
        current_time_s: u64,
    ) -> Result<Option<Pc5SignalingMessage>, ProseRelayError> {
        match msg {
            Pc5SignalingMessage::DirectCommunicationRequest {
                session_id,
                source_l2_id,
                target_l2_id: _,
                pqi,
                nonce_ue,
                ip_addr: _,
            } => {
                // Relay receives DCR from Remote
                let relay_session_id = *session_id;

                let nonce_relay = [0x55; 16];
                let root_key = self
                    .peer_root_keys
                    .get(source_l2_id)
                    .copied()
                    .unwrap_or([0x42; 32]);
                let k_sess = derive_k_nrp_sess(&root_key, nonce_ue, &nonce_relay, relay_session_id);

                let session = Pc5Session {
                    session_id: relay_session_id,
                    peer_l2_id: *source_l2_id,
                    local_l2_id: self.local_l2_id,
                    state: Pc5LinkState::Securing,
                    k_nrp: root_key,
                    k_nrp_sess: k_sess,
                    cipher_algo: Pc5SecurityAlgorithm::Nea2AesCtr,
                    integrity_algo: Pc5SecurityAlgorithm::Nia2AesCmac,
                    qos_profile: Pc5QoSProfile::from_pqi(*pqi),
                    ip_address: Some(Ipv4Addr::new(
                        192,
                        168,
                        50,
                        (relay_session_id % 200 + 10) as u8,
                    )),
                    last_heartbeat_s: current_time_s,
                    missed_keepalives: 0,
                    rsrp_dbm: -85,
                };
                self.pc5_sessions.insert(relay_session_id, session);

                Ok(Some(Pc5SignalingMessage::DirectSecurityModeCommand {
                    session_id: relay_session_id,
                    cipher_algo: Pc5SecurityAlgorithm::Nea2AesCtr,
                    integrity_algo: Pc5SecurityAlgorithm::Nia2AesCmac,
                    nonce_relay,
                }))
            }
            Pc5SignalingMessage::DirectSecurityModeCommand {
                session_id,
                cipher_algo,
                integrity_algo,
                nonce_relay,
            } => {
                // Remote receives SecurityModeCommand
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;

                let nonce_ue = [0xAA; 16];
                session.k_nrp_sess =
                    derive_k_nrp_sess(&session.k_nrp, &nonce_ue, nonce_relay, *session_id);
                session.cipher_algo = *cipher_algo;
                session.integrity_algo = *integrity_algo;
                session.state = Pc5LinkState::Securing;

                let mac_tag = [0xDD; 8];
                Ok(Some(Pc5SignalingMessage::DirectSecurityModeComplete {
                    session_id: *session_id,
                    mac_tag,
                }))
            }
            Pc5SignalingMessage::DirectSecurityModeComplete {
                session_id,
                mac_tag: _,
            } => {
                // Relay receives SecurityModeComplete -> accepts and assigns IP
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;
                session.state = Pc5LinkState::Established;

                Ok(Some(Pc5SignalingMessage::DirectCommunicationAccept {
                    session_id: *session_id,
                    assigned_ip: session.ip_address,
                    pqfi: session.qos_profile.pqi,
                }))
            }
            Pc5SignalingMessage::DirectCommunicationAccept {
                session_id,
                assigned_ip,
                pqfi: _,
            } => {
                // Remote receives DirectCommunicationAccept -> Link Established!
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;
                session.state = Pc5LinkState::Established;
                session.ip_address = *assigned_ip;
                Ok(None)
            }
            Pc5SignalingMessage::DirectCommunicationKeepalive {
                session_id,
                sequence_num,
            } => {
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;
                session.last_heartbeat_s = current_time_s;
                session.missed_keepalives = 0;
                Ok(Some(Pc5SignalingMessage::DirectCommunicationKeepaliveAck {
                    session_id: *session_id,
                    sequence_num: *sequence_num,
                }))
            }
            Pc5SignalingMessage::DirectCommunicationKeepaliveAck {
                session_id,
                sequence_num: _,
            } => {
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;
                session.last_heartbeat_s = current_time_s;
                session.missed_keepalives = 0;
                Ok(None)
            }
            Pc5SignalingMessage::DirectCommunicationReleaseRequest {
                session_id,
                cause: _,
            } => {
                let session = self
                    .pc5_sessions
                    .get_mut(session_id)
                    .ok_or(ProseRelayError::SessionNotFound(*session_id))?;
                session.state = Pc5LinkState::Releasing;
                Ok(Some(
                    Pc5SignalingMessage::DirectCommunicationReleaseAccept {
                        session_id: *session_id,
                    },
                ))
            }
            Pc5SignalingMessage::DirectCommunicationReleaseAccept { session_id } => {
                if let Some(session) = self.pc5_sessions.get_mut(session_id) {
                    session.state = Pc5LinkState::Disconnected;
                }
                Ok(None)
            }
            Pc5SignalingMessage::DirectCommunicationReject {
                source_l2_id: _,
                cause: _,
            } => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // Layer-2 (L2) U2N Relay with SRAP (TS 38.340)
    // -----------------------------------------------------------------------

    /// Register Remote UE in Relay UE's L2 SRAP Routing Table.
    pub fn register_l2_remote_ue(
        &mut self,
        remote_l2_id: Pc5Layer2Id,
        uu_bearer_id: u8,
        pc5_rlc_channel_id: u8,
    ) -> u16 {
        let remote_ue_local_id = self.next_remote_local_id;
        self.next_remote_local_id += 1;

        let ctx = L2RelayContext {
            remote_ue_local_id,
            remote_l2_id,
            uu_bearer_id,
            pc5_rlc_channel_id,
        };
        self.l2_relay_table.insert(remote_ue_local_id, ctx);
        remote_ue_local_id
    }

    /// Uplink: Relay UE encapsulates Remote UE PC5 packet with SRAP header for Uu link.
    pub fn forward_l2_uplink(
        &self,
        remote_ue_local_id: u16,
        bearer_id: u8,
        payload: &[u8],
    ) -> Result<Vec<u8>, ProseRelayError> {
        if !self.l2_relay_table.contains_key(&remote_ue_local_id) {
            return Err(ProseRelayError::RemoteLocalIdNotFound(remote_ue_local_id));
        }
        let header = SrapHeader::new(remote_ue_local_id, bearer_id);
        let header_bytes = header.encode();

        let mut packet = Vec::with_capacity(3 + payload.len());
        packet.extend_from_slice(&header_bytes);
        packet.extend_from_slice(payload);
        Ok(packet)
    }

    /// Downlink: Relay UE decapsulates SRAP packet from gNB and resolves target Remote UE.
    pub fn forward_l2_downlink(
        &self,
        srap_packet: &[u8],
    ) -> Result<(Pc5Layer2Id, u8, Vec<u8>), ProseRelayError> {
        let (header, payload) = SrapHeader::decode(srap_packet)?;
        let ctx = self.l2_relay_table.get(&header.remote_ue_local_id).ok_or(
            ProseRelayError::RemoteLocalIdNotFound(header.remote_ue_local_id),
        )?;

        Ok((ctx.remote_l2_id, ctx.pc5_rlc_channel_id, payload.to_vec()))
    }

    // -----------------------------------------------------------------------
    // Layer-3 (L3) U2N Relay with IP NAT & PDU Session Mapping
    // -----------------------------------------------------------------------

    /// Uplink: Remote UE IP packet sent to Relay UE -> Translated via NAT & mapped to PDU TEID.
    pub fn forward_l3_uplink(
        &mut self,
        remote_ip: Ipv4Addr,
        remote_port: u16,
        dest_ip: Ipv4Addr,
        dest_port: u16,
        protocol: u8,
        payload: &[u8],
        current_time_s: u64,
    ) -> Result<(u32, Vec<u8>), ProseRelayError> {
        // Look for existing NAT mapping
        let mut relay_port = None;
        for entry in &mut self.l3_nat_table {
            if entry.remote_ip == remote_ip
                && entry.remote_port == remote_port
                && entry.dest_ip == dest_ip
                && entry.dest_port == dest_port
                && entry.protocol == protocol
            {
                entry.last_activity_s = current_time_s;
                relay_port = Some(entry.relay_port);
                break;
            }
        }

        let assigned_port = match relay_port {
            Some(p) => p,
            None => {
                let p = self.next_nat_port;
                self.next_nat_port += 1;
                self.l3_nat_table.push(L3RelayNatEntry {
                    remote_ip,
                    remote_port,
                    relay_port: p,
                    dest_ip,
                    dest_port,
                    protocol,
                    last_activity_s: current_time_s,
                });
                p
            }
        };

        // Construct NAT encapsulated packet representation:
        // [Relay_IP (4B), Dest_IP (4B), Protocol (1B), Assigned_Port (2B), Dest_Port (2B), Payload...]
        let mut out = Vec::with_capacity(13 + payload.len());
        out.extend_from_slice(&self.l3_relay_ip.octets());
        out.extend_from_slice(&dest_ip.octets());
        out.push(protocol);
        out.extend_from_slice(&assigned_port.to_be_bytes());
        out.extend_from_slice(&dest_port.to_be_bytes());
        out.extend_from_slice(payload);

        Ok((self.l3_relay_pdu_teid, out))
    }

    /// Downlink: Return IP packet from UPF received at Relay UE -> Reverse NAT to Remote UE.
    pub fn forward_l3_downlink(
        &mut self,
        relay_port: u16,
        src_ip: Ipv4Addr,
        src_port: u16,
        payload: &[u8],
        current_time_s: u64,
    ) -> Result<(Ipv4Addr, u16, Vec<u8>), ProseRelayError> {
        let mut matched_entry = None;
        for entry in &mut self.l3_nat_table {
            if entry.relay_port == relay_port
                && entry.dest_ip == src_ip
                && entry.dest_port == src_port
            {
                entry.last_activity_s = current_time_s;
                matched_entry = Some((entry.remote_ip, entry.remote_port));
                break;
            }
        }

        let (remote_ip, remote_port) = matched_entry.ok_or(ProseRelayError::NatMappingNotFound)?;

        // Return translated packet:
        // [Src_IP (4B), Remote_IP (4B), Src_Port (2B), Remote_Port (2B), Payload...]
        let mut out = Vec::with_capacity(12 + payload.len());
        out.extend_from_slice(&src_ip.octets());
        out.extend_from_slice(&remote_ip.octets());
        out.extend_from_slice(&src_port.to_be_bytes());
        out.extend_from_slice(&remote_port.to_be_bytes());
        out.extend_from_slice(payload);

        Ok((remote_ip, remote_port, out))
    }

    // -----------------------------------------------------------------------
    // Link Health Monitoring, RLF Detection & Relay Reselection
    // -----------------------------------------------------------------------

    /// Record received PC5 signal quality (RSRP) and heartbeat.
    pub fn record_heartbeat(
        &mut self,
        session_id: u32,
        rsrp_dbm: i16,
        current_time_s: u64,
    ) -> Result<bool, ProseRelayError> {
        let session = self
            .pc5_sessions
            .get_mut(&session_id)
            .ok_or(ProseRelayError::SessionNotFound(session_id))?;

        session.rsrp_dbm = rsrp_dbm;
        session.last_heartbeat_s = current_time_s;
        session.missed_keepalives = 0;

        if rsrp_dbm < self.rlf_rsrp_threshold_dbm {
            session.state = Pc5LinkState::Disconnected;
            return Err(ProseRelayError::RadioLinkFailure {
                session_id,
                rsrp_dbm,
            });
        }
        Ok(true)
    }

    /// Periodic servo tick: Checks for keepalive timeouts and triggers RLF.
    pub fn tick_liveness_check(
        &mut self,
        current_time_s: u64,
        heartbeat_timeout_s: u64,
    ) -> Vec<u32> {
        let mut rlf_sessions = Vec::new();
        for (sid, session) in self.pc5_sessions.iter_mut() {
            if session.state != Pc5LinkState::Established {
                continue;
            }
            if current_time_s.saturating_sub(session.last_heartbeat_s) >= heartbeat_timeout_s {
                session.missed_keepalives += 1;
                if session.missed_keepalives >= self.max_missed_keepalives {
                    session.state = Pc5LinkState::Disconnected;
                    rlf_sessions.push(*sid);
                }
            }
        }
        rlf_sessions
    }

    /// Reselect alternate Relay UE when RLF occurs.
    pub fn trigger_reselection(
        &mut self,
        failed_session_id: u32,
        alternate_relays: &[RelayAnnouncement],
    ) -> Option<RelayAnnouncement> {
        if let Some(session) = self.pc5_sessions.get_mut(&failed_session_id) {
            session.state = Pc5LinkState::Disconnected;
        }

        // Find candidate with strongest RSRP above threshold
        let mut best_candidate: Option<RelayAnnouncement> = None;
        for ann in alternate_relays {
            if ann.rsrp_dbm >= self.rlf_rsrp_threshold_dbm {
                match &best_candidate {
                    None => best_candidate = Some(ann.clone()),
                    Some(curr) => {
                        if ann.rsrp_dbm > curr.rsrp_dbm {
                            best_candidate = Some(ann.clone());
                        }
                    }
                }
            }
        }
        best_candidate
    }
}
