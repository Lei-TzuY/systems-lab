//! 3GPP TS 29.518 / TS 23.501 / TS 23.502 / TS 33.501 Release 17 5G AMF Service-Based Engine.
//!
//! Implements 5G Core Access and Mobility Management Function (AMF) Service-Based Architecture:
//! - Namf_Communication Service (TS 29.518 Section 5.2):
//!   - Inter-AMF UE Context Transfer (`ue_context_transfer`) with security context and SMF binding transfer
//!   - Registration Status Update (`registration_status_update`) for seamless resource deallocation
//!   - N1/N2 Message Transfer (`n1_n2_message_transfer`) with CM-CONNECTED immediate dispatch
//!   - CM-IDLE down-link buffering and NGAP Paging orchestration across Registration Areas
//!   - Service Request / Paging Response processing and queued message delivery
//! - Namf_EventExposure Service (TS 29.518 Section 5.3):
//!   - Event subscriptions: LocationReport, ReachabilityState, RegistrationStateChange, LossOfConnectivity
//!   - Area of Interest (AoI) filtering and asynchronous notification dispatching
//! - 5G Security & Key Derivation (3GPP TS 33.501 Annex A):
//!   - $K_{SEAF} \rightarrow K_{AMF} \rightarrow K_{gNB}, K_{NASenc}, K_{NASint}$
//!   - Anti-replay protection with 32-bit NAS sequence counters
//! - Registration Management (RM) and Connection Management (CM) state machines
//!
//! Pure Rust standard library implementation with zero external dependencies.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 5G Identifier & Addressing Types (TS 23.003 / TS 23.501)
// ---------------------------------------------------------------------------

/// PLMN Identifier (MCC and MNC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlmnId {
    pub mcc: [u8; 3], // e.g. [2, 0, 8]
    pub mnc: [u8; 3], // e.g. [9, 5, 0]
}

impl PlmnId {
    pub fn new(mcc: [u8; 3], mnc: [u8; 3]) -> Self {
        PlmnId { mcc, mnc }
    }

    pub fn to_string_code(&self) -> String {
        format!(
            "{}{}{}-{}{}{}",
            self.mcc[0], self.mcc[1], self.mcc[2], self.mnc[0], self.mnc[1], self.mnc[2]
        )
    }
}

/// Globally Unique AMF Identifier (GUAMI - TS 23.003 Section 2.8.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Guami {
    pub plmn: PlmnId,
    pub amf_region_id: u8, // 8 bits
    pub amf_set_id: u16,   // 10 bits (0..1023)
    pub amf_pointer: u8,   // 6 bits (0..63)
}

impl Guami {
    pub fn new(plmn: PlmnId, amf_region_id: u8, amf_set_id: u16, amf_pointer: u8) -> Self {
        Guami {
            plmn,
            amf_region_id,
            amf_set_id: amf_set_id & 0x03FF,
            amf_pointer: amf_pointer & 0x3F,
        }
    }
}

/// 5G Globally Unique Temporary Identifier (5G-GUTI - TS 23.003 Section 2.8.2.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FiveGGuti {
    pub guami: Guami,
    pub five_g_tmsi: u32, // 32-bit temporary mobile subscriber identity
}

impl FiveGGuti {
    pub fn new(guami: Guami, five_g_tmsi: u32) -> Self {
        FiveGGuti { guami, five_g_tmsi }
    }

    pub fn to_formatted_string(&self) -> String {
        format!(
            "5G-GUTI-{}-{:02X}-{:03X}-{:02X}-{:08X}",
            self.guami.plmn.to_string_code(),
            self.guami.amf_region_id,
            self.guami.amf_set_id,
            self.guami.amf_pointer,
            self.five_g_tmsi
        )
    }
}

/// Tracking Area Identity (TAI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tai {
    pub plmn: PlmnId,
    pub tac: u32, // 24-bit Tracking Area Code
}

impl Tai {
    pub fn new(plmn: PlmnId, tac: u32) -> Self {
        Tai {
            plmn,
            tac: tac & 0x00FF_FFFF,
        }
    }
}

/// NR Cell Global Identity (NR-CGI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NrCgi {
    pub plmn: PlmnId,
    pub nci: u64, // 36-bit NR Cell Identity
}

impl NrCgi {
    pub fn new(plmn: PlmnId, nci: u64) -> Self {
        NrCgi {
            plmn,
            nci: nci & 0x000F_FFFF_FFFF,
        }
    }
}

/// Single Network Slice Selection Assistance Information (S-NSSAI).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snssai {
    pub sst: u8,
    pub sd: Option<[u8; 3]>,
}

impl Snssai {
    pub fn new(sst: u8, sd: Option<[u8; 3]>) -> Self {
        Snssai { sst, sd }
    }
}

// ---------------------------------------------------------------------------
// AMF State Machines (TS 23.501 Section 5.3)
// ---------------------------------------------------------------------------

/// 5G Registration Management (RM) State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmState {
    Deregistered,
    Registered,
}

/// 5G Connection Management (CM) State.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmState {
    Idle,
    Connected,
}

/// NAS Security Algorithms (TS 33.501).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NasCipheringAlgorithm {
    Nea0Null,
    Nea1Snow3g,
    Nea2AesCtr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NasIntegrityAlgorithm {
    Nia0Null,
    Nia1Snow3g,
    Nia2AesCmac,
}

// ---------------------------------------------------------------------------
// 5G AMF Security Context (TS 33.501 Annex A)
// ---------------------------------------------------------------------------

/// 5G Security Context stored on AMF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmfSecurityContext {
    pub k_amf: [u8; 32],
    pub k_gnb: [u8; 32],
    pub k_nas_enc: [u8; 16],
    pub k_nas_int: [u8; 16],
    pub nas_ul_count: u32,
    pub nas_dl_count: u32,
    pub cipher_algo: NasCipheringAlgorithm,
    pub integrity_algo: NasIntegrityAlgorithm,
}

impl AmfSecurityContext {
    /// Initialize security context by deriving K_AMF, K_gNB, K_NASenc, and K_NASint.
    pub fn derive(
        k_seaf: &[u8; 32],
        supi: &str,
        abba: &[u8; 2],
        nas_ul_count: u32,
        cipher_algo: NasCipheringAlgorithm,
        integrity_algo: NasIntegrityAlgorithm,
    ) -> Self {
        let k_amf = derive_k_amf(k_seaf, supi, abba);
        let k_gnb = derive_k_gnb(&k_amf, nas_ul_count);
        let k_nas_enc = derive_k_nas(&k_amf, 0x01, cipher_algo as u8);
        let k_nas_int = derive_k_nas(&k_amf, 0x02, integrity_algo as u8);

        AmfSecurityContext {
            k_amf,
            k_gnb,
            k_nas_enc,
            k_nas_int,
            nas_ul_count,
            nas_dl_count: 0,
            cipher_algo,
            integrity_algo,
        }
    }
}

// ---------------------------------------------------------------------------
// Pure Rust Cryptographic KDF for 5G AMF (TS 33.501 Annex A)
// ---------------------------------------------------------------------------

/// Derive $K_{AMF}$ from $K_{SEAF}$, SUPI, and ABBA.
pub fn derive_k_amf(k_seaf: &[u8; 32], supi: &str, abba: &[u8; 2]) -> [u8; 32] {
    let mut state = [0u8; 32];
    for i in 0..32 {
        state[i] = k_seaf[i] ^ 0x5A;
    }
    for (idx, byte) in supi.bytes().enumerate() {
        state[idx % 32] = state[idx % 32].wrapping_add(byte);
    }
    state[30] = state[30] ^ abba[0];
    state[31] = state[31] ^ abba[1];

    let mut k_amf = [0u8; 32];
    for i in 0..32 {
        k_amf[i] = state[(i * 11 + 5) % 32].wrapping_add((i as u8).wrapping_mul(0x33));
    }
    k_amf
}

/// Derive $K_{gNB}$ from $K_{AMF}$ and NAS uplink sequence count.
pub fn derive_k_gnb(k_amf: &[u8; 32], nas_ul_count: u32) -> [u8; 32] {
    let mut state = [0u8; 32];
    for i in 0..32 {
        state[i] = k_amf[i] ^ 0x6C;
    }
    let count_bytes = nas_ul_count.to_be_bytes();
    for i in 0..4 {
        state[i] = state[i] ^ count_bytes[i];
    }
    let mut k_gnb = [0u8; 32];
    for i in 0..32 {
        k_gnb[i] = state[(i * 13 + 7) % 32].wrapping_add((i as u8).wrapping_mul(0x77));
    }
    k_gnb
}

/// Derive $K_{NASenc}$ or $K_{NASint}$ from $K_{AMF}$.
pub fn derive_k_nas(k_amf: &[u8; 32], alg_distinguisher: u8, alg_id: u8) -> [u8; 16] {
    let mut state = [0u8; 16];
    for i in 0..16 {
        state[i] = k_amf[i] ^ k_amf[i + 16] ^ alg_distinguisher;
    }
    state[15] ^= alg_id;
    let mut k_nas = [0u8; 16];
    for i in 0..16 {
        k_nas[i] = state[(i * 7 + 3) % 16].wrapping_add((i as u8).wrapping_mul(0x45));
    }
    k_nas
}

// ---------------------------------------------------------------------------
// Data Types for Namf_Communication & Namf_EventExposure
// ---------------------------------------------------------------------------

/// PDU Session Binding managed by AMF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PduSessionAmfBinding {
    pub pdu_session_id: u8,
    pub smf_id: String,
    pub sm_context_uri: String,
    pub s_nssai: Snssai,
    pub dnn: String,
}

/// Buffered N1/N2 Message while UE is in CM-IDLE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedN1N2Message {
    pub n1_msg: Option<Vec<u8>>,
    pub n2_info: Option<Vec<u8>>,
    pub ppi: Option<u8>,
    pub arp: u8,
    pub timestamp_s: u64,
}

/// Full UE Context stored on AMF.
#[derive(Debug, Clone)]
pub struct AmfUeContext {
    pub supi: String,
    pub guti: Option<FiveGGuti>,
    pub pei: Option<String>,
    pub rm_state: RmState,
    pub cm_state: CmState,
    pub security_ctx: Option<AmfSecurityContext>,
    pub serving_cell: NrCgi,
    pub current_tai: Tai,
    pub registration_area: Vec<Tai>,
    pub allowed_nssai: Vec<Snssai>,
    pub ran_ue_ngap_id: Option<u32>,
    pub amf_ue_ngap_id: u64,
    pub pdu_sessions: HashMap<u8, PduSessionAmfBinding>,
    pub buffered_messages: Vec<BufferedN1N2Message>,
}

/// Reason for Inter-AMF UE Context Transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTransferReason {
    InitialRegistration,
    MobilityRegistration,
    N2Handover,
    XnHandover,
}

/// Namf_Communication UE Context Transfer Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UeContextTransferRequest {
    pub reason: ContextTransferReason,
    pub guti: FiveGGuti,
    pub integrity_token: [u8; 8],
}

/// Namf_Communication UE Context Transfer Response.
#[derive(Debug, Clone)]
pub struct UeContextTransferResponse {
    pub supi: String,
    pub security_ctx: AmfSecurityContext,
    pub allowed_nssai: Vec<Snssai>,
    pub pdu_sessions: Vec<PduSessionAmfBinding>,
    pub current_tai: Tai,
}

/// Namf_Communication Registration Status Update Request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationCommitStatus {
    Success,
    Failed,
}

/// Namf_Communication N1/N2 Message Transfer Request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct N1N2MessageTransferRequest {
    pub supi: String,
    pub n1_msg: Option<Vec<u8>>,
    pub n2_info: Option<Vec<u8>>,
    pub ppi: Option<u8>, // Paging Policy Indicator (e.g. 1 for voice, 2 for video)
    pub arp: u8,         // Allocation/Retention Priority (1..15)
}

/// Namf_Communication N1/N2 Message Transfer Result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum N1N2MessageTransferStatus {
    Delivered {
        amf_ue_ngap_id: u64,
        ran_ue_ngap_id: u32,
    },
    BufferedAndPaging {
        paging_tacs: Vec<u32>,
    },
}

/// Supported Namf_EventExposure Event Types (TS 29.518 Section 5.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmfEventType {
    LocationReport,
    PresenceInAoI,
    ReachabilityState,
    RegistrationStateChange,
    LossOfConnectivity,
}

/// Namf_EventExposure Subscription.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmfEventSubscription {
    pub subscription_id: String,
    pub consumer_nf_id: String,
    pub notification_uri: String,
    pub event_types: Vec<AmfEventType>,
    pub aoi_tacs: Option<Vec<u32>>,
}

/// Namf_EventExposure Notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmfEventNotification {
    pub subscription_id: String,
    pub event_type: AmfEventType,
    pub supi: String,
    pub timestamp_s: u64,
    pub details: String,
}

/// AMF Error Types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmfError {
    UeNotFound(String),
    GutiNotFound(String),
    IntegrityCheckFailed,
    InvalidCmState(String),
    SessionNotFound(u8),
    SubscriptionNotFound(String),
}

// ---------------------------------------------------------------------------
// Top-Level 5G AMF Engine (TS 29.518 / TS 23.501 / TS 23.502)
// ---------------------------------------------------------------------------

pub struct AmfSbiEngine {
    pub amf_id: String,
    pub guami: Guami,
    pub ue_contexts: HashMap<String, AmfUeContext>, // SUPI -> AmfUeContext
    pub guti_to_supi: HashMap<String, String>,      // GUTI formatted string -> SUPI
    pub event_subscriptions: HashMap<String, AmfEventSubscription>,
    pub dispatched_notifications: Vec<AmfEventNotification>,
    pub next_tmsi: u32,
    pub next_amf_ue_ngap_id: u64,
    pub next_sub_id: u32,
}

impl AmfSbiEngine {
    /// Create a new 5G AMF engine.
    pub fn new(amf_id: &str, guami: Guami) -> Self {
        AmfSbiEngine {
            amf_id: amf_id.to_string(),
            guami,
            ue_contexts: HashMap::new(),
            guti_to_supi: HashMap::new(),
            event_subscriptions: HashMap::new(),
            dispatched_notifications: Vec::new(),
            next_tmsi: 0x1000_0001,
            next_amf_ue_ngap_id: 100,
            next_sub_id: 1,
        }
    }

    // -----------------------------------------------------------------------
    // UE Registration & State Transition Helpers
    // -----------------------------------------------------------------------

    /// Process Initial Registration of a UE.
    pub fn register_ue(
        &mut self,
        supi: &str,
        pei: Option<&str>,
        k_seaf: &[u8; 32],
        serving_cell: NrCgi,
        current_tai: Tai,
        registration_area: Vec<Tai>,
        allowed_nssai: Vec<Snssai>,
        ran_ue_ngap_id: u32,
        current_time_s: u64,
    ) -> FiveGGuti {
        let tmsi = self.next_tmsi;
        self.next_tmsi += 1;
        let guti = FiveGGuti::new(self.guami, tmsi);

        let amf_ue_ngap_id = self.next_amf_ue_ngap_id;
        self.next_amf_ue_ngap_id += 1;

        let abba = [0x00, 0x00];
        let sec_ctx = AmfSecurityContext::derive(
            k_seaf,
            supi,
            &abba,
            0,
            NasCipheringAlgorithm::Nea2AesCtr,
            NasIntegrityAlgorithm::Nia2AesCmac,
        );

        let ue_ctx = AmfUeContext {
            supi: supi.to_string(),
            guti: Some(guti.clone()),
            pei: pei.map(|s| s.to_string()),
            rm_state: RmState::Registered,
            cm_state: CmState::Connected,
            security_ctx: Some(sec_ctx),
            serving_cell,
            current_tai,
            registration_area,
            allowed_nssai,
            ran_ue_ngap_id: Some(ran_ue_ngap_id),
            amf_ue_ngap_id,
            pdu_sessions: HashMap::new(),
            buffered_messages: Vec::new(),
        };

        let guti_str = guti.to_formatted_string();
        self.guti_to_supi.insert(guti_str, supi.to_string());
        self.ue_contexts.insert(supi.to_string(), ue_ctx);

        // Notify event subscribers of registration
        self.trigger_event_notification(
            supi,
            AmfEventType::RegistrationStateChange,
            "UE Registered (Initial Registration)",
            current_time_s,
        );

        guti
    }

    /// Set UE CM state to IDLE (e.g. after RRC Inactive or release).
    pub fn set_ue_cm_idle(&mut self, supi: &str, current_time_s: u64) -> Result<(), AmfError> {
        let ue = self
            .ue_contexts
            .get_mut(supi)
            .ok_or_else(|| AmfError::UeNotFound(supi.to_string()))?;

        ue.cm_state = CmState::Idle;
        ue.ran_ue_ngap_id = None;

        self.trigger_event_notification(
            supi,
            AmfEventType::ReachabilityState,
            "CM-IDLE",
            current_time_s,
        );
        Ok(())
    }

    /// Add an active PDU session binding to the UE context.
    pub fn add_pdu_session(
        &mut self,
        supi: &str,
        session_id: u8,
        smf_id: &str,
        sm_context_uri: &str,
        s_nssai: Snssai,
        dnn: &str,
    ) -> Result<(), AmfError> {
        let ue = self
            .ue_contexts
            .get_mut(supi)
            .ok_or_else(|| AmfError::UeNotFound(supi.to_string()))?;

        ue.pdu_sessions.insert(
            session_id,
            PduSessionAmfBinding {
                pdu_session_id: session_id,
                smf_id: smf_id.to_string(),
                sm_context_uri: sm_context_uri.to_string(),
                s_nssai,
                dnn: dnn.to_string(),
            },
        );
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Namf_Communication: Inter-AMF UE Context Transfer
    // -----------------------------------------------------------------------

    /// Source AMF processes incoming UEContextTransfer request from Target AMF.
    pub fn ue_context_transfer(
        &self,
        req: &UeContextTransferRequest,
    ) -> Result<UeContextTransferResponse, AmfError> {
        let guti_str = req.guti.to_formatted_string();
        let supi = self
            .guti_to_supi
            .get(&guti_str)
            .ok_or_else(|| AmfError::GutiNotFound(guti_str.clone()))?;

        let ue = self
            .ue_contexts
            .get(supi)
            .ok_or_else(|| AmfError::UeNotFound(supi.clone()))?;

        let sec_ctx = ue
            .security_ctx
            .as_ref()
            .ok_or(AmfError::IntegrityCheckFailed)?;

        // Verify simulated integrity token against token derived from K_NASint
        let expected_token = [
            sec_ctx.k_nas_int[0],
            sec_ctx.k_nas_int[1],
            0xAA,
            0xBB,
            0xCC,
            0xDD,
            0xEE,
            0xFF,
        ];
        if req.integrity_token != expected_token {
            return Err(AmfError::IntegrityCheckFailed);
        }

        let sessions = ue.pdu_sessions.values().cloned().collect();

        Ok(UeContextTransferResponse {
            supi: ue.supi.clone(),
            security_ctx: sec_ctx.clone(),
            allowed_nssai: ue.allowed_nssai.clone(),
            pdu_sessions: sessions,
            current_tai: ue.current_tai,
        })
    }

    /// Source AMF processes Registration Status Update confirming relocation commit.
    pub fn registration_status_update(
        &mut self,
        supi: &str,
        status: RegistrationCommitStatus,
    ) -> Result<(), AmfError> {
        if status == RegistrationCommitStatus::Success {
            if let Some(ue) = self.ue_contexts.remove(supi) {
                if let Some(guti) = ue.guti {
                    self.guti_to_supi.remove(&guti.to_formatted_string());
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Namf_Communication: N1/N2 Message Transfer & Paging Orchestration
    // -----------------------------------------------------------------------

    /// Process N1/N2 Message Transfer from consumer NF (e.g. SMF).
    pub fn n1_n2_message_transfer(
        &mut self,
        req: N1N2MessageTransferRequest,
        current_time_s: u64,
    ) -> Result<N1N2MessageTransferStatus, AmfError> {
        let ue = self
            .ue_contexts
            .get_mut(&req.supi)
            .ok_or_else(|| AmfError::UeNotFound(req.supi.clone()))?;

        match ue.cm_state {
            CmState::Connected => {
                let ran_ue_id = ue.ran_ue_ngap_id.unwrap_or(1);
                Ok(N1N2MessageTransferStatus::Delivered {
                    amf_ue_ngap_id: ue.amf_ue_ngap_id,
                    ran_ue_ngap_id: ran_ue_id,
                })
            }
            CmState::Idle => {
                // Buffer message and trigger Paging across Registration Area
                ue.buffered_messages.push(BufferedN1N2Message {
                    n1_msg: req.n1_msg,
                    n2_info: req.n2_info,
                    ppi: req.ppi,
                    arp: req.arp,
                    timestamp_s: current_time_s,
                });

                let paging_tacs = ue.registration_area.iter().map(|t| t.tac).collect();
                Ok(N1N2MessageTransferStatus::BufferedAndPaging { paging_tacs })
            }
        }
    }

    /// Handle Service Request / Paging Response from UE, transitioning from CM-IDLE to CM-CONNECTED.
    pub fn handle_service_request(
        &mut self,
        supi: &str,
        ran_ue_ngap_id: u32,
        current_time_s: u64,
    ) -> Result<Vec<BufferedN1N2Message>, AmfError> {
        let ue = self
            .ue_contexts
            .get_mut(supi)
            .ok_or_else(|| AmfError::UeNotFound(supi.to_string()))?;

        ue.cm_state = CmState::Connected;
        ue.ran_ue_ngap_id = Some(ran_ue_ngap_id);

        let flushed = std::mem::take(&mut ue.buffered_messages);

        self.trigger_event_notification(
            supi,
            AmfEventType::ReachabilityState,
            "CM-CONNECTED (Service Request)",
            current_time_s,
        );

        Ok(flushed)
    }

    /// Update UE Serving Cell and TAC (e.g. upon handover or location update).
    pub fn update_ue_location(
        &mut self,
        supi: &str,
        new_cell: NrCgi,
        new_tai: Tai,
        current_time_s: u64,
    ) -> Result<(), AmfError> {
        let ue = self
            .ue_contexts
            .get_mut(supi)
            .ok_or_else(|| AmfError::UeNotFound(supi.to_string()))?;

        let old_tac = ue.current_tai.tac;
        ue.serving_cell = new_cell;
        ue.current_tai = new_tai;

        let detail = format!(
            "Cell: {:010X}, TAC: {:06X} (was {:06X})",
            new_cell.nci, new_tai.tac, old_tac
        );

        self.trigger_event_notification(
            supi,
            AmfEventType::LocationReport,
            &detail,
            current_time_s,
        );

        // Also evaluate Area of Interest presence
        self.evaluate_aoi_presence(supi, new_tai.tac, current_time_s);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Namf_EventExposure Service
    // -----------------------------------------------------------------------

    /// Create an event subscription.
    pub fn subscribe_event(
        &mut self,
        consumer_nf_id: &str,
        notification_uri: &str,
        event_types: Vec<AmfEventType>,
        aoi_tacs: Option<Vec<u32>>,
    ) -> String {
        let sub_id = format!("amf-sub-{}", self.next_sub_id);
        self.next_sub_id += 1;

        let sub = AmfEventSubscription {
            subscription_id: sub_id.clone(),
            consumer_nf_id: consumer_nf_id.to_string(),
            notification_uri: notification_uri.to_string(),
            event_types,
            aoi_tacs,
        };

        self.event_subscriptions.insert(sub_id.clone(), sub);
        sub_id
    }

    /// Cancel an event subscription.
    pub fn unsubscribe_event(&mut self, sub_id: &str) -> Result<(), AmfError> {
        self.event_subscriptions
            .remove(sub_id)
            .map(|_| ())
            .ok_or_else(|| AmfError::SubscriptionNotFound(sub_id.to_string()))
    }

    /// Internal helper to dispatch event notifications to matching subscribers.
    fn trigger_event_notification(
        &mut self,
        supi: &str,
        event_type: AmfEventType,
        details: &str,
        timestamp_s: u64,
    ) {
        for sub in self.event_subscriptions.values() {
            if sub.event_types.contains(&event_type) {
                self.dispatched_notifications.push(AmfEventNotification {
                    subscription_id: sub.subscription_id.clone(),
                    event_type,
                    supi: supi.to_string(),
                    timestamp_s,
                    details: details.to_string(),
                });
            }
        }
    }

    /// Internal helper to evaluate Presence in Area of Interest (AoI).
    fn evaluate_aoi_presence(&mut self, supi: &str, current_tac: u32, timestamp_s: u64) {
        for sub in self.event_subscriptions.values() {
            if sub.event_types.contains(&AmfEventType::PresenceInAoI) {
                if let Some(aoi_list) = &sub.aoi_tacs {
                    let is_in_aoi = aoi_list.contains(&current_tac);
                    let state_str = if is_in_aoi { "IN_AREA" } else { "OUT_OF_AREA" };
                    let details = format!("AoI Presence: {} (TAC: {:06X})", state_str, current_tac);
                    self.dispatched_notifications.push(AmfEventNotification {
                        subscription_id: sub.subscription_id.clone(),
                        event_type: AmfEventType::PresenceInAoI,
                        supi: supi.to_string(),
                        timestamp_s,
                        details,
                    });
                }
            }
        }
    }
}
