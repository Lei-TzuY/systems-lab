//! 3GPP TS 29.544 / TS 23.501 Section 5.17 / TS 23.502 Section 4.11 / TS 29.274 5G-EPS Interworking Engine.
//!
//! Implements seamless inter-system mobility and session continuity between 5GS and 4G EPC:
//! - Combined SMF + PGW-C node session context management (PDU Session <-> PDN Connection).
//! - Combined UPF + PGW-U user plane tunneling (N3 GTP-U <-> S1-U / S5/S8-U GTP-U).
//! - Dynamic EPS Bearer ID (EBI: 5..15) allocation and 5QI <-> QCI QoS parameter mapping.
//! - N26 Interface Handover State Machine:
//!   - Forward Relocation Request (5GS -> EPS) with K_AMF to K_ASME security key derivation (TS 33.501 Annex A.15).
//!   - Forward Relocation Response (EPS -> 5GS) with target eNodeB S1-U F-TEID and bearer admission.
//!   - Direct and Indirect Data Forwarding Tunnels preventing in-flight packet loss.
//!   - Forward Relocation Complete Notification & resource cleanup.
//! - Voice EPS Fallback (TS 23.501 §5.16.4):
//!   - Evaluates 5QI 1 (Conversational Voice) requests against cell VoNR capabilities, triggering
//!     deterministic N26 handover preparation and pre-reserving dedicated QCI 1 bearer in EPC.

use std::collections::HashMap;

use crate::ausf_udm_5g::sha256;
use crate::ipv4::Ipv4Address;
use crate::ngap_5g::Snssai;

// ---------------------------------------------------------------------------
// 4G/5G Interworking Identifiers & Constants (TS 29.274 / TS 23.501)
// ---------------------------------------------------------------------------

/// Valid EPS Bearer ID range (EBI 5..15 per TS 24.007 / TS 29.274).
pub const MIN_EBI: u8 = 5;
pub const MAX_EBI: u8 = 15;

/// GTPv2-C / GTP-U F-TEID Interface Types.
pub const FTEID_S1_U_ENB: u8 = 0;
pub const FTEID_S1_U_SGW: u8 = 1;
pub const FTEID_S5_S8_SGW: u8 = 6;
pub const FTEID_S5_S8_PGW: u8 = 7;
pub const FTEID_S11_MME: u8 = 10;
pub const FTEID_S11_SGW: u8 = 11;
pub const FTEID_S1_U_FORWARDING: u8 = 20;

/// GTPv2-C Cause Codes.
pub const CAUSE_REQUEST_ACCEPTED: u8 = 16;
pub const CAUSE_NO_RESOURCES_AVAILABLE: u8 = 73;
pub const CAUSE_CONTEXT_NOT_FOUND: u8 = 64;

// ---------------------------------------------------------------------------
// F-TEID & QoS Data Structures
// ---------------------------------------------------------------------------

/// Fully Qualified Tunnel Endpoint Identifier (F-TEID - TS 29.274 Section 8.22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fteid {
    pub interface_type: u8,
    pub teid: u32,
    pub ipv4_address: Ipv4Address,
}

impl Fteid {
    pub fn new(interface_type: u8, teid: u32, ipv4_address: Ipv4Address) -> Self {
        Fteid {
            interface_type,
            teid,
            ipv4_address,
        }
    }
}

/// 4G EPS QoS Profile (TS 23.203 / TS 29.274 Section 8.15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsQosProfile {
    pub qci: u8,          // 1..9 standard QCIs
    pub arp_priority: u8, // 1..15
    pub preemption_capability: bool,
    pub preemption_vulnerability: bool,
    pub gbr_dl_kbps: Option<u64>,
    pub gbr_ul_kbps: Option<u64>,
    pub mbr_dl_kbps: Option<u64>,
    pub mbr_ul_kbps: Option<u64>,
}

impl EpsQosProfile {
    pub fn new(qci: u8, arp_priority: u8) -> Self {
        EpsQosProfile {
            qci,
            arp_priority,
            preemption_capability: false,
            preemption_vulnerability: false,
            gbr_dl_kbps: None,
            gbr_ul_kbps: None,
            mbr_dl_kbps: None,
            mbr_ul_kbps: None,
        }
    }
}

/// EPS Bearer Context maintained by Combined SMF+PGW-C / MME.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsBearerContext {
    pub ebi: u8,
    pub linked_ebi: Option<u8>, // None for default bearer, Some(default_ebi) for dedicated bearer
    pub qos: EpsQosProfile,
    pub pgw_u_fteid: Fteid,
    pub enb_fteid: Option<Fteid>,
    pub dl_forwarding_fteid: Option<Fteid>,
}

// ---------------------------------------------------------------------------
// Combined SMF + PGW-C Session Context (TS 29.544 / TS 23.501 §5.17.2)
// ---------------------------------------------------------------------------

/// Combined SMF + PGW-C unified session context.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinedSmfPgwContext {
    pub supi: String,
    pub imsi: String,
    pub pdu_session_id: u8,
    pub dnn_apn: String,
    pub snssai: Snssai,
    pub ue_ipv4_address: Ipv4Address,
    pub pgw_c_fteid: Fteid,
    pub default_ebi: u8,
    pub bearers: HashMap<u8, EpsBearerContext>,
    /// 5G QFI (1..64) -> 4G EBI (5..15) mapping
    pub qfi_to_ebi: HashMap<u8, u8>,
}

// ---------------------------------------------------------------------------
// N26 Signaling Messages & Handover States (TS 29.274 / TS 23.502 §4.11.1.2)
// ---------------------------------------------------------------------------

/// N26 Inter-System Handover State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum N26HandoverState {
    Idle,
    Prepared,
    Executing,
    Completed,
    Failed,
}

/// N26 Forward Relocation Request (5GS -> EPS).
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardRelocationRequest {
    pub imsi: String,
    pub source_amf_id: String,
    pub target_mme_id: String,
    pub target_cell_tai: String,
    pub derived_k_asme: [u8; 32],
    pub combined_context: CombinedSmfPgwContext,
}

/// N26 Forward Relocation Response (EPS -> 5GS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardRelocationResponse {
    pub accepted: bool,
    pub cause: u8,
    pub admitted_ebis: Vec<u8>,
    pub enb_s1u_fteids: HashMap<u8, Fteid>, // EBI -> eNB S1-U F-TEID
    pub dl_forwarding_fteids: HashMap<u8, Fteid>, // EBI -> SGW DL forwarding F-TEID
}

/// Data Forwarding Tunnel buffering in-flight user plane packets during handover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsDataForwardingTunnel {
    pub ebi: u8,
    pub forwarding_fteid: Fteid,
    pub buffered_packets: Vec<Vec<u8>>,
}

/// Errors during 5G-EPS Interworking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpsInterworkingError {
    EbiPoolExhausted,
    SessionNotFound,
    InvalidEbi(u8),
    HandoverFailed(&'static str),
    QosMappingError(&'static str),
}

// ---------------------------------------------------------------------------
// 5QI <-> QCI Mapping Rules (TS 23.501 Table 5.17.2-1)
// ---------------------------------------------------------------------------

/// Map standard 3GPP 5QI to standard 4G QCI.
pub fn map_5qi_to_qci(five_qi: u16) -> u8 {
    match five_qi {
        1 => 1,       // Conversational Voice (GBR)
        2 => 2,       // Conversational Video (GBR)
        3 => 3,       // Real Time Gaming (GBR)
        4 => 4,       // Non-Conversational Video (GBR)
        5 => 5,       // IMS Signalling (Non-GBR)
        6 => 6,       // Video (Non-GBR)
        7 => 7,       // Voice, Video, Interactive (Non-GBR)
        8 => 8,       // Non-GBR
        9 => 9,       // Default Internet (Non-GBR)
        82..=86 => 7, // Delay-Critical TSN GBR mapped to high priority interactive
        _ => 9,       // Default fallback to QCI 9
    }
}

/// Map standard 4G QCI to standard 3GPP 5QI.
pub fn map_qci_to_5qi(qci: u8) -> u16 {
    match qci {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        6 => 6,
        7 => 7,
        8 => 8,
        9 => 9,
        _ => 9,
    }
}

/// Derive 4G K_ASME key from 5G K_AMF (TS 33.501 Annex A.15).
pub fn derive_k_asme_from_k_amf(k_amf: &[u8; 32], nas_ul_count: u32) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.push(0x6F); // FC = 0x6F for K_ASME derivation from K_AMF
    buf.extend_from_slice(&nas_ul_count.to_be_bytes());
    buf.extend_from_slice(k_amf);
    sha256(&buf)
}

// ---------------------------------------------------------------------------
// Top-Level 5G-EPS Interworking Engine
// ---------------------------------------------------------------------------

/// 5G-EPS Interworking Engine orchestrating combined node sessions and N26 handovers.
pub struct EpsInterworkingEngine {
    pub engine_id: String,
    pub pgw_c_ip: Ipv4Address,
    pub pgw_u_ip: Ipv4Address,
    /// supi -> CombinedSmfPgwContext
    pub sessions: HashMap<String, CombinedSmfPgwContext>,
    /// N26 Handover states: supi -> state
    pub handover_states: HashMap<String, N26HandoverState>,
    /// Active data forwarding tunnels: ebi -> tunnel
    pub forwarding_tunnels: HashMap<u8, EpsDataForwardingTunnel>,
    pub next_teid: u32,
}

impl EpsInterworkingEngine {
    /// Create a new 5G-EPS Interworking Engine.
    pub fn new(engine_id: &str, pgw_c_ip: Ipv4Address, pgw_u_ip: Ipv4Address) -> Self {
        EpsInterworkingEngine {
            engine_id: engine_id.to_string(),
            pgw_c_ip,
            pgw_u_ip,
            sessions: HashMap::new(),
            handover_states: HashMap::new(),
            forwarding_tunnels: HashMap::new(),
            next_teid: 0x1000_0001,
        }
    }

    /// Allocate next unique TEID.
    fn allocate_teid(&mut self) -> u32 {
        let teid = self.next_teid;
        self.next_teid += 1;
        teid
    }

    // -----------------------------------------------------------------------
    // Combined Session Management (TS 29.544 / TS 23.501 §5.17.2)
    // -----------------------------------------------------------------------

    /// Initialize a combined 5GS PDU Session + 4G PDN Connection context.
    /// Allocates default EPS Bearer ID (EBI: 5).
    pub fn establish_combined_session(
        &mut self,
        supi: &str,
        imsi: &str,
        pdu_session_id: u8,
        dnn_apn: &str,
        snssai: Snssai,
        ue_ipv4: Ipv4Address,
        default_5qi: u16,
    ) -> Result<&CombinedSmfPgwContext, EpsInterworkingError> {
        let default_ebi = MIN_EBI; // EBI 5 for default bearer
        let default_qci = map_5qi_to_qci(default_5qi);

        let pgw_c_teid = self.allocate_teid();
        let pgw_c_fteid = Fteid::new(FTEID_S5_S8_PGW, pgw_c_teid, self.pgw_c_ip);

        let pgw_u_teid = self.allocate_teid();
        let pgw_u_fteid = Fteid::new(FTEID_S5_S8_PGW, pgw_u_teid, self.pgw_u_ip);

        let default_qos = EpsQosProfile::new(default_qci, 9);
        let default_bearer = EpsBearerContext {
            ebi: default_ebi,
            linked_ebi: None,
            qos: default_qos,
            pgw_u_fteid,
            enb_fteid: None,
            dl_forwarding_fteid: None,
        };

        let mut bearers = HashMap::new();
        bearers.insert(default_ebi, default_bearer);

        let mut qfi_to_ebi = HashMap::new();
        qfi_to_ebi.insert(1, default_ebi); // Default QoS flow (QFI 1) -> EBI 5

        let context = CombinedSmfPgwContext {
            supi: supi.to_string(),
            imsi: imsi.to_string(),
            pdu_session_id,
            dnn_apn: dnn_apn.to_string(),
            snssai,
            ue_ipv4_address: ue_ipv4,
            pgw_c_fteid,
            default_ebi,
            bearers,
            qfi_to_ebi,
        };

        self.sessions.insert(supi.to_string(), context);
        Ok(self.sessions.get(supi).unwrap())
    }

    /// Allocate a dedicated EPS Bearer for a secondary 5G QoS Flow (e.g. VoNR / IMS voice).
    pub fn allocate_dedicated_bearer(
        &mut self,
        supi: &str,
        qfi: u8,
        five_qi: u16,
        arp: u8,
    ) -> Result<u8, EpsInterworkingError> {
        let default_ebi;
        let ebi;
        {
            let session = self
                .sessions
                .get(supi)
                .ok_or(EpsInterworkingError::SessionNotFound)?;

            // Find next free EBI in range 5..15
            let mut allocated_ebi = None;
            for candidate in MIN_EBI..=MAX_EBI {
                if !session.bearers.contains_key(&candidate) {
                    allocated_ebi = Some(candidate);
                    break;
                }
            }

            ebi = allocated_ebi.ok_or(EpsInterworkingError::EbiPoolExhausted)?;
            default_ebi = session.default_ebi;
        }

        let qci = map_5qi_to_qci(five_qi);
        let pgw_u_teid = self.allocate_teid();
        let pgw_u_fteid = Fteid::new(FTEID_S5_S8_PGW, pgw_u_teid, self.pgw_u_ip);

        let qos = EpsQosProfile::new(qci, arp);
        let bearer = EpsBearerContext {
            ebi,
            linked_ebi: Some(default_ebi),
            qos,
            pgw_u_fteid,
            enb_fteid: None,
            dl_forwarding_fteid: None,
        };

        let session = self.sessions.get_mut(supi).unwrap();
        session.bearers.insert(ebi, bearer);
        session.qfi_to_ebi.insert(qfi, ebi);

        Ok(ebi)
    }

    // -----------------------------------------------------------------------
    // N26 Handover Operations (5GS -> EPS) (TS 23.502 §4.11.1.2)
    // -----------------------------------------------------------------------

    /// Prepare N26 Forward Relocation Request (5GS -> EPS).
    /// Derives K_ASME security key and packages combined session state.
    pub fn prepare_n26_handover_to_eps(
        &mut self,
        supi: &str,
        source_amf_id: &str,
        target_mme_id: &str,
        target_cell_tai: &str,
        k_amf: &[u8; 32],
        nas_ul_count: u32,
    ) -> Result<ForwardRelocationRequest, EpsInterworkingError> {
        let session = self
            .sessions
            .get(supi)
            .ok_or(EpsInterworkingError::SessionNotFound)?;

        let derived_k_asme = derive_k_asme_from_k_amf(k_amf, nas_ul_count);

        let req = ForwardRelocationRequest {
            imsi: session.imsi.clone(),
            source_amf_id: source_amf_id.to_string(),
            target_mme_id: target_mme_id.to_string(),
            target_cell_tai: target_cell_tai.to_string(),
            derived_k_asme,
            combined_context: session.clone(),
        };

        self.handover_states
            .insert(supi.to_string(), N26HandoverState::Prepared);

        Ok(req)
    }

    /// Process Target MME Forward Relocation Response (EPS -> 5GS).
    /// Configures admitted eNodeB S1-U F-TEIDs and establishes DL Data Forwarding Tunnels.
    pub fn process_n26_handover_response(
        &mut self,
        supi: &str,
        response: &ForwardRelocationResponse,
    ) -> Result<(), EpsInterworkingError> {
        if !response.accepted {
            self.handover_states
                .insert(supi.to_string(), N26HandoverState::Failed);
            return Err(EpsInterworkingError::HandoverFailed(
                "Target MME rejected relocation request",
            ));
        }

        let session = self
            .sessions
            .get_mut(supi)
            .ok_or(EpsInterworkingError::SessionNotFound)?;

        // Attach eNB S1-U F-TEIDs and configure Data Forwarding Tunnels
        for &ebi in &response.admitted_ebis {
            if let Some(bearer) = session.bearers.get_mut(&ebi) {
                if let Some(enb_fteid) = response.enb_s1u_fteids.get(&ebi) {
                    bearer.enb_fteid = Some(enb_fteid.clone());
                }

                // If target MME provided a DL Forwarding F-TEID, establish forwarding tunnel
                if let Some(fwd_fteid) = response.dl_forwarding_fteids.get(&ebi) {
                    bearer.dl_forwarding_fteid = Some(fwd_fteid.clone());
                    let tunnel = EpsDataForwardingTunnel {
                        ebi,
                        forwarding_fteid: fwd_fteid.clone(),
                        buffered_packets: Vec::new(),
                    };
                    self.forwarding_tunnels.insert(ebi, tunnel);
                }
            }
        }

        self.handover_states
            .insert(supi.to_string(), N26HandoverState::Executing);

        Ok(())
    }

    /// Buffer an in-flight packet during handover execution into the DL Data Forwarding Tunnel.
    pub fn forward_in_flight_packet(
        &mut self,
        ebi: u8,
        packet_payload: Vec<u8>,
    ) -> Result<(), EpsInterworkingError> {
        let tunnel = self
            .forwarding_tunnels
            .get_mut(&ebi)
            .ok_or(EpsInterworkingError::InvalidEbi(ebi))?;

        tunnel.buffered_packets.push(packet_payload);
        Ok(())
    }

    /// Complete N26 Handover when UE synchronizes to target eNodeB.
    /// Flushes all forwarded packets and transitions state to Completed.
    pub fn complete_n26_handover(
        &mut self,
        supi: &str,
    ) -> Result<HashMap<u8, Vec<Vec<u8>>>, EpsInterworkingError> {
        let state = self
            .handover_states
            .get_mut(supi)
            .ok_or(EpsInterworkingError::SessionNotFound)?;

        if *state != N26HandoverState::Executing {
            return Err(EpsInterworkingError::HandoverFailed(
                "Handover not in Executing state",
            ));
        }

        *state = N26HandoverState::Completed;

        // Drain forwarded packets per admitted EBI
        let mut delivered_packets = HashMap::new();
        let session = self.sessions.get(supi).unwrap();
        for &ebi in session.bearers.keys() {
            if let Some(tunnel) = self.forwarding_tunnels.remove(&ebi) {
                delivered_packets.insert(ebi, tunnel.buffered_packets);
            }
        }

        Ok(delivered_packets)
    }

    // -----------------------------------------------------------------------
    // Voice EPS Fallback (TS 23.501 §5.16.4)
    // -----------------------------------------------------------------------

    /// Evaluate a 5QI 1 (Conversational Voice) request.
    /// If serving 5G cell does not support VoNR, triggers Voice EPS Fallback.
    pub fn handle_voice_call_request(
        &mut self,
        supi: &str,
        vonr_supported_in_cell: bool,
    ) -> Result<VoiceCallAction, EpsInterworkingError> {
        if vonr_supported_in_cell {
            // Allocate 5G VoNR flow (QFI 2, 5QI 1)
            let ebi = self.allocate_dedicated_bearer(supi, 2, 1, 1)?;
            Ok(VoiceCallAction::Maintain5gVoNr { dedicated_ebi: ebi })
        } else {
            // VoNR not supported: trigger EPS Fallback
            // Pre-allocate dedicated QCI 1 bearer for target 4G EPC
            let ebi = self.allocate_dedicated_bearer(supi, 2, 1, 1)?;
            Ok(VoiceCallAction::TriggerEpsFallback {
                target_qci: 1,
                dedicated_ebi: ebi,
            })
        }
    }
}

/// Action resulting from a voice call request evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceCallAction {
    Maintain5gVoNr { dedicated_ebi: u8 },
    TriggerEpsFallback { target_qci: u8, dedicated_ebi: u8 },
}
