//! 3GPP TS 38.331 Rel-17 5G NR RRC Inactive State & RAN Notification Area (RNA) Paging Engine.
//!
//! Implements 5G NR Layer 3 Control Plane procedures for `RRC_INACTIVE` state (TS 38.331 §5.3.8, §5.3.13, §5.3.14):
//! - RRC Connection Suspension: `RRCRelease` carrying `InactiveSuspendConfig` (Full I-RNTI 40-bit, Short I-RNTI 24-bit, RNA, T380 timer, NCC).
//! - RAN Notification Area (RNA) evaluation: autonomous mobility within cell list / RAN area codes without signaling.
//! - RAN Notification Area Update (RNAU): triggered by boundary crossing or periodic `T380` expiration.
//! - RRC Connection Resume: Msg3 `RrcResumeRequestMessage` with 16-bit ShortMAC-I authentication.
//! - Xn-AP Inter-gNodeB UE Context Retrieval (TS 38.423 §8.2) across Anchor and Serving gNodeBs over Xn-C.
//! - RAN Paging (TS 38.300 §8.2): Anchor gNodeB triggers RNA-wide paging upon downlink user-plane data arrival.

use std::collections::HashMap;

/// 40-bit Full I-RNTI (TS 38.331 Section 6.3.2).
///
/// Encodes:
/// - Anchor gNodeB Identifier (24 bits)
/// - UE Context Index (16 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FullIRnti(pub u64);

impl FullIRnti {
    /// Mask for 40-bit integer.
    pub const MASK_40_BIT: u64 = 0x00FF_FFFF_FFFF;

    /// Create a new Full I-RNTI from Anchor gNodeB ID (24-bit) and UE Context Index (16-bit).
    pub fn new(anchor_gnb_id: u32, ue_index: u16) -> Self {
        let val = (((anchor_gnb_id as u64) & 0x00FF_FFFF) << 16) | ((ue_index as u64) & 0xFFFF);
        Self(val & Self::MASK_40_BIT)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    /// Extract 24-bit Anchor gNodeB Identifier.
    pub fn anchor_gnb_id(&self) -> u32 {
        ((self.0 >> 16) & 0x00FF_FFFF) as u32
    }

    /// Extract 16-bit UE Context Index.
    pub fn ue_index(&self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// Derive 24-bit Short I-RNTI used in RRCResumeRequest:
    /// Encodes top 16 bits of gNodeB ID + top 8 bits of UE Index.
    pub fn to_short_i_rnti(&self) -> ShortIRnti {
        let gnb_slice = ((self.anchor_gnb_id() >> 8) & 0xFFFF) as u32;
        let ue_slice = ((self.ue_index() >> 8) & 0xFF) as u32;
        let short_val = (gnb_slice << 8) | ue_slice;
        ShortIRnti(short_val & 0x00FF_FFFF)
    }
}

/// 24-bit Short I-RNTI used in Msg3 RrcResumeRequest (TS 38.331 §6.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortIRnti(pub u32);

impl ShortIRnti {
    pub const MASK_24_BIT: u32 = 0x00FF_FFFF;

    pub fn new(val: u32) -> Self {
        Self(val & Self::MASK_24_BIT)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Extract anchor gNodeB slice (upper 16 bits).
    pub fn anchor_slice(&self) -> u16 {
        ((self.0 >> 8) & 0xFFFF) as u16
    }
}

/// RAN Notification Area (RNA) configuration (TS 38.331 §5.3.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RanNotificationArea {
    /// Explicit list of NR Cell Identities (36-bit NCIs).
    CellList(Vec<u64>),
    /// List of RAN Area Codes within a Tracking Area.
    RanAreaCodes { tac: u32, ranac_list: Vec<u8> },
}

impl RanNotificationArea {
    /// Check whether a cell belongs to this RAN Notification Area.
    pub fn contains_cell(&self, nci: u64, tac: u32, ranac: Option<u8>) -> bool {
        match self {
            Self::CellList(cells) => cells.contains(&nci),
            Self::RanAreaCodes {
                tac: area_tac,
                ranac_list,
            } => {
                if *area_tac != tac {
                    return false;
                }
                if let Some(code) = ranac {
                    ranac_list.contains(&code)
                } else {
                    false
                }
            }
        }
    }
}

/// Suspend Configuration carried in `RRCRelease` message (TS 38.331 §5.3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveSuspendConfig {
    pub full_i_rnti: FullIRnti,
    pub short_i_rnti: ShortIRnti,
    pub rna: RanNotificationArea,
    /// Periodic RNA update timer in minutes (5, 10, 20, 30, 60, 120 min).
    pub t380_period_mins: u32,
    /// Next Hop Chaining Count (NCC: 0..7) for key derivation.
    pub next_hop_chaining_count: u8,
}

/// Resume Cause carried in `RRCResumeRequest` (TS 38.331 §6.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveResumeCause {
    Emergency = 0,
    HighPriorityAccess = 1,
    MtAccess = 2,
    MoSignalling = 3,
    MoData = 4,
    MoVoiceCall = 5,
    MoVideoCall = 6,
    MoSms = 7,
    RnaUpdate = 8,
    MpsPriorityAccess = 9,
    McsPriorityAccess = 10,
}

/// Calculate 16-bit ShortMAC-I authentication tag over `VarResumeMAC-Input` (TS 38.331 §5.3.13.3).
///
/// Inputs:
/// - Source PCI (16-bit)
/// - Target Cell ID (36-bit NCI)
/// - Short I-RNTI (24-bit)
/// - K_RRCint integrity key (128-bit)
pub fn calculate_short_mac_i(
    source_pci: u16,
    target_cell_id: u64,
    short_i_rnti: u32,
    k_rrc_int: &[u8; 16],
) -> u16 {
    // 3GPP VarResumeMAC-Input layout:
    // [0..2]: source_pci (BE)
    // [2..10]: target_cell_id (BE)
    // [10..14]: short_i_rnti (BE)
    let mut buffer = [0u8; 14];
    buffer[0..2].copy_from_slice(&source_pci.to_be_bytes());
    buffer[2..10].copy_from_slice(&target_cell_id.to_be_bytes());
    buffer[10..14].copy_from_slice(&short_i_rnti.to_be_bytes());

    // SipHash / polynomial MAC simulation over K_RRCint
    let mut h: u32 = 0x811C_9DC5;
    for &b in buffer.iter() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }

    for &k in k_rrc_int.iter() {
        h ^= k as u32;
        h = h.wrapping_mul(0x0100_0193);
    }

    // 16-bit ShortMAC-I
    ((h ^ (h >> 16)) & 0xFFFF) as u16
}

/// Msg3 RrcResumeRequest message sent by UE over CCCH1 (TS 38.331 §5.3.13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RrcResumeRequestMessage {
    pub short_i_rnti: ShortIRnti,
    pub resume_cause: InactiveResumeCause,
    pub short_mac_i: u16,
    pub source_pci: u16,
    pub target_cell_id: u64,
}

/// RrcResume message sent by gNodeB to UE (TS 38.331 §5.3.13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RrcResumeMessage {
    pub allocated_c_rnti: u16,
    pub restored_drb_ids: Vec<u8>,
    pub next_hop_chaining_count: u8,
    pub new_suspend_config: Option<InactiveSuspendConfig>,
}

/// Xn-AP Retrieve UE Context Request message (TS 38.423 §8.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnUeContextRetrieveRequest {
    pub target_gnb_id: u32,
    pub anchor_gnb_id: u32,
    pub short_i_rnti: ShortIRnti,
    pub resume_cause: InactiveResumeCause,
    pub short_mac_i: u16,
    pub target_cell_id: u64,
    pub source_pci: u16,
}

/// Xn-AP Retrieve UE Context Response message (TS 38.423 §8.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XnUeContextRetrieveResponse {
    pub ue_id: String,
    pub full_i_rnti: FullIRnti,
    pub k_gnb: [u8; 32],
    pub next_hop_chaining_count: u8,
    pub active_drb_ids: Vec<u8>,
    pub pdu_session_id: u32,
    pub amf_ue_ngap_id: u64,
}

/// RAN Paging Record broadcast by gNodeBs within RNA (TS 38.300 §8.2 / TS 38.331 §5.3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RanPagingRecord {
    pub ue_identity: FullIRnti,
    pub paging_drx_slots: u16,
    pub ran_paging_area: RanNotificationArea,
    pub paging_priority: Option<u8>,
}

/// UE Inactive Context retained at the Anchor gNodeB while UE is in RRC_INACTIVE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveUeContext {
    pub ue_id: String,
    pub full_i_rnti: FullIRnti,
    pub short_i_rnti: ShortIRnti,
    pub anchor_gnb_id: u32,
    pub anchor_cell_id: u64,
    pub source_pci: u16,
    pub rna: RanNotificationArea,
    pub k_gnb: [u8; 32],
    pub k_rrc_int: [u8; 16],
    pub ncc: u8,
    pub active_drb_ids: Vec<u8>,
    pub pdu_session_id: u32,
    pub amf_ue_ngap_id: u64,
    pub is_paging_pending: bool,
}

/// 3GPP Rel-17 5G NR RRC Inactive State & RAN Paging Engine.
#[derive(Debug)]
pub struct NrRrcInactiveEngine {
    pub local_gnb_id: u32,
    /// Anchor storage of Inactive UE contexts: FullIRnti -> Context.
    pub suspended_contexts: HashMap<FullIRnti, InactiveUeContext>,
    /// Short I-RNTI to Full I-RNTI lookup table.
    pub short_to_full_index: HashMap<ShortIRnti, FullIRnti>,
    /// Next available C-RNTI on this gNodeB for resuming UEs.
    next_c_rnti: u16,
    // Simulated UE entity state for client testing
    pub ue_state_inactive: bool,
    pub ue_suspend_config: Option<InactiveSuspendConfig>,
    pub ue_k_rrc_int: [u8; 16],
    pub ue_source_pci: u16,
    pub ue_current_cell_id: u64,
    pub ue_current_pci: u16,
    pub ue_t380_remaining_minutes: u32,
}

impl NrRrcInactiveEngine {
    /// Create a new RRC Inactive Engine for a gNodeB instance.
    pub fn new(local_gnb_id: u32) -> Self {
        Self {
            local_gnb_id,
            suspended_contexts: HashMap::new(),
            short_to_full_index: HashMap::new(),
            next_c_rnti: 0x4000,
            ue_state_inactive: false,
            ue_suspend_config: None,
            ue_k_rrc_int: [0u8; 16],
            ue_source_pci: 0,
            ue_current_cell_id: 0,
            ue_current_pci: 0,
            ue_t380_remaining_minutes: 0,
        }
    }

    /// Allocate next available C-RNTI on local cell.
    fn allocate_c_rnti(&mut self) -> u16 {
        let rnti = self.next_c_rnti;
        self.next_c_rnti = self.next_c_rnti.wrapping_add(1);
        if self.next_c_rnti < 0x4000 {
            self.next_c_rnti = 0x4000;
        }
        rnti
    }

    // -----------------------------------------------------------------------
    // Anchor gNodeB Control Plane Procedures
    // -----------------------------------------------------------------------

    /// Suspend an active UE connection and store context in Anchor storage (TS 38.331 §5.3.8).
    pub fn suspend_ue_connection(
        &mut self,
        ue_id: &str,
        ue_index: u16,
        anchor_cell_id: u64,
        source_pci: u16,
        rna: RanNotificationArea,
        k_gnb: [u8; 32],
        k_rrc_int: [u8; 16],
        ncc: u8,
        active_drb_ids: Vec<u8>,
        pdu_session_id: u32,
        amf_ue_ngap_id: u64,
        t380_period_mins: u32,
    ) -> InactiveSuspendConfig {
        let full_i_rnti = FullIRnti::new(self.local_gnb_id, ue_index);
        let short_i_rnti = full_i_rnti.to_short_i_rnti();

        let context = InactiveUeContext {
            ue_id: ue_id.to_string(),
            full_i_rnti,
            short_i_rnti,
            anchor_gnb_id: self.local_gnb_id,
            anchor_cell_id,
            source_pci,
            rna: rna.clone(),
            k_gnb,
            k_rrc_int,
            ncc,
            active_drb_ids,
            pdu_session_id,
            amf_ue_ngap_id,
            is_paging_pending: false,
        };

        self.suspended_contexts.insert(full_i_rnti, context);
        self.short_to_full_index.insert(short_i_rnti, full_i_rnti);

        InactiveSuspendConfig {
            full_i_rnti,
            short_i_rnti,
            rna,
            t380_period_mins,
            next_hop_chaining_count: ncc,
        }
    }

    /// Trigger RAN Paging across the UE's configured RNA upon downlink data arrival at Anchor gNodeB.
    pub fn trigger_ran_paging(
        &mut self,
        full_i_rnti: FullIRnti,
    ) -> Result<RanPagingRecord, &'static str> {
        let ctx = self
            .suspended_contexts
            .get_mut(&full_i_rnti)
            .ok_or("UE context not found in Anchor gNodeB storage")?;

        ctx.is_paging_pending = true;

        Ok(RanPagingRecord {
            ue_identity: full_i_rnti,
            paging_drx_slots: 128,
            ran_paging_area: ctx.rna.clone(),
            paging_priority: Some(1),
        })
    }

    /// Process an Xn-AP Retrieve UE Context Request from a different Serving gNodeB (TS 38.423 §8.2.1).
    pub fn process_xn_retrieve_context(
        &mut self,
        req: &XnUeContextRetrieveRequest,
    ) -> Result<XnUeContextRetrieveResponse, &'static str> {
        let full_i_rnti = *self
            .short_to_full_index
            .get(&req.short_i_rnti)
            .ok_or("Short I-RNTI does not map to any active suspended context")?;

        let ctx = self
            .suspended_contexts
            .get(&full_i_rnti)
            .ok_or("Suspended context not found")?;

        // Authenticate ShortMAC-I
        let expected_mac = calculate_short_mac_i(
            ctx.source_pci,
            req.target_cell_id,
            req.short_i_rnti.as_u32(),
            &ctx.k_rrc_int,
        );

        if req.short_mac_i != expected_mac {
            return Err("ShortMAC-I integrity verification failed on Anchor gNodeB");
        }

        // Increment NCC for key refresh
        let updated_ncc = (ctx.ncc + 1) % 8;

        let resp = XnUeContextRetrieveResponse {
            ue_id: ctx.ue_id.clone(),
            full_i_rnti: ctx.full_i_rnti,
            k_gnb: ctx.k_gnb,
            next_hop_chaining_count: updated_ncc,
            active_drb_ids: ctx.active_drb_ids.clone(),
            pdu_session_id: ctx.pdu_session_id,
            amf_ue_ngap_id: ctx.amf_ue_ngap_id,
        };

        // Context relocated: remove from old anchor storage
        self.suspended_contexts.remove(&full_i_rnti);
        self.short_to_full_index.remove(&req.short_i_rnti);

        Ok(resp)
    }

    /// Process an RrcResumeRequest arriving directly on the Anchor gNodeB (TS 38.331 §5.3.13).
    pub fn process_local_resume_request(
        &mut self,
        req: &RrcResumeRequestMessage,
    ) -> Result<RrcResumeMessage, &'static str> {
        let full_i_rnti = *self
            .short_to_full_index
            .get(&req.short_i_rnti)
            .ok_or("Unknown Short I-RNTI on local gNodeB")?;

        let (expected_mac, restored_drb_ids, ncc, full_i_rnti_val, short_i_rnti_val, rna_val) = {
            let ctx = self
                .suspended_contexts
                .get_mut(&full_i_rnti)
                .ok_or("Context not found in suspended storage")?;

            let mac = calculate_short_mac_i(
                ctx.source_pci,
                req.target_cell_id,
                req.short_i_rnti.as_u32(),
                &ctx.k_rrc_int,
            );
            ctx.ncc = (ctx.ncc + 1) % 8;
            (
                mac,
                ctx.active_drb_ids.clone(),
                ctx.ncc,
                ctx.full_i_rnti,
                ctx.short_i_rnti,
                ctx.rna.clone(),
            )
        };

        if req.short_mac_i != expected_mac {
            return Err("ShortMAC-I verification failed on local gNodeB");
        }

        let allocated_c_rnti = self.allocate_c_rnti();

        let response = if req.resume_cause == InactiveResumeCause::RnaUpdate {
            // RNA Update only: keep UE suspended in RRC_INACTIVE!
            RrcResumeMessage {
                allocated_c_rnti,
                restored_drb_ids: Vec::new(),
                next_hop_chaining_count: ncc,
                new_suspend_config: Some(InactiveSuspendConfig {
                    full_i_rnti: full_i_rnti_val,
                    short_i_rnti: short_i_rnti_val,
                    rna: rna_val,
                    t380_period_mins: 30,
                    next_hop_chaining_count: ncc,
                }),
            }
        } else {
            // Full connection resume: transition UE to RRC_CONNECTED!
            self.suspended_contexts.remove(&full_i_rnti);
            self.short_to_full_index.remove(&req.short_i_rnti);

            RrcResumeMessage {
                allocated_c_rnti,
                restored_drb_ids,
                next_hop_chaining_count: ncc,
                new_suspend_config: None,
            }
        };

        Ok(response)
    }

    // -----------------------------------------------------------------------
    // UE Side State Machine & Helper Routines
    // -----------------------------------------------------------------------

    /// Enter RRC_INACTIVE state on UE with received SuspendConfig.
    pub fn ue_enter_inactive(
        &mut self,
        config: InactiveSuspendConfig,
        k_rrc_int: [u8; 16],
        cell_id: u64,
        pci: u16,
    ) {
        self.ue_state_inactive = true;
        self.ue_t380_remaining_minutes = config.t380_period_mins;
        self.ue_suspend_config = Some(config);
        self.ue_k_rrc_int = k_rrc_int;
        self.ue_source_pci = pci;
        self.ue_current_cell_id = cell_id;
        self.ue_current_pci = pci;
    }

    /// Move UE to a new cell and evaluate whether an RNA Update is required (TS 38.331 §5.3.14).
    pub fn ue_move_to_cell(
        &mut self,
        new_cell_id: u64,
        new_pci: u16,
        tac: u32,
        ranac: Option<u8>,
    ) -> Option<InactiveResumeCause> {
        self.ue_current_cell_id = new_cell_id;
        self.ue_current_pci = new_pci;

        if let Some(ref config) = self.ue_suspend_config {
            if !config.rna.contains_cell(new_cell_id, tac, ranac) {
                // Moved outside configured RNA -> Trigger Mobility RNA Update!
                return Some(InactiveResumeCause::RnaUpdate);
            }
        }

        None
    }

    /// Periodic minute tick for UE's T380 timer.
    pub fn ue_tick_minute(&mut self) -> Option<InactiveResumeCause> {
        if !self.ue_state_inactive {
            return None;
        }

        if self.ue_t380_remaining_minutes > 0 {
            self.ue_t380_remaining_minutes -= 1;
            if self.ue_t380_remaining_minutes == 0 {
                // Timer expired -> Trigger Periodic RNA Update!
                if let Some(ref config) = self.ue_suspend_config {
                    self.ue_t380_remaining_minutes = config.t380_period_mins;
                }
                return Some(InactiveResumeCause::RnaUpdate);
            }
        }

        None
    }

    /// Construct an Msg3 RrcResumeRequest with computed ShortMAC-I authentication tag.
    pub fn ue_create_resume_request(
        &self,
        cause: InactiveResumeCause,
    ) -> Result<RrcResumeRequestMessage, &'static str> {
        let config = self
            .ue_suspend_config
            .as_ref()
            .ok_or("UE is not in RRC_INACTIVE state or missing SuspendConfig")?;

        let short_mac_i = calculate_short_mac_i(
            self.ue_source_pci,
            self.ue_current_cell_id,
            config.short_i_rnti.as_u32(),
            &self.ue_k_rrc_int,
        );

        Ok(RrcResumeRequestMessage {
            short_i_rnti: config.short_i_rnti,
            resume_cause: cause,
            short_mac_i,
            source_pci: self.ue_source_pci,
            target_cell_id: self.ue_current_cell_id,
        })
    }
}
