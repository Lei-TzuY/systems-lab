//! 3GPP Rel-17 5G NR Multicast and Broadcast Services (MBS) Point-to-Multipoint (PTM) & Radio Delivery Engine.
//!
//! Conforms to:
//! - 3GPP TS 38.300 Rel-17 §16.15: Multicast and Broadcast Services (MBS) overall architecture.
//! - 3GPP TS 38.321 Rel-17 §5.x / §6.1.3: MAC architecture, G-RNTI/G-CS-RNTI group scheduling,
//!   PTM HARQ feedback (Option 1 ACK/NACK & Option 2 NACK-only on shared PUCCH), MBS DRX.
//! - 3GPP TS 38.331 Rel-17 §5.3.5 / §6.3.2: RRC MBS-SessionInfo, MRB (MBS Radio Bearer) and
//!   Split MRB configuration, MCCH (MBS Control Channel) modification & repetition cycles,
//!   MBS Interest Indication (MII).
//! - 3GPP TS 38.213 Rel-17 §16.x: Physical layer procedures for group scheduling and shared PUCCH feedback.
//!
//! Pure standard Rust (`std` / `core` only) with zero external dependencies.

use std::collections::{HashMap, VecDeque};

// ===========================================================================
// 1. Core Data Structures & Identifiers
// ===========================================================================

/// Temporary Mobile Group Identity (TMGI) per 3GPP TS 23.003 §20.5 & TS 38.331.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MbsTmgi {
    /// 24-bit Service ID (3 octets)
    pub service_id: [u8; 3],
    /// Mobile Country Code (3 digits, e.g. "001")
    pub mcc: String,
    /// Mobile Network Code (2 or 3 digits, e.g. "01")
    pub mnc: String,
}

impl MbsTmgi {
    pub fn new(service_id: [u8; 3], mcc: &str, mnc: &str) -> Self {
        Self {
            service_id,
            mcc: mcc.to_string(),
            mnc: mnc.to_string(),
        }
    }

    /// Canonical string representation: e.g. "010203-001-01".
    pub fn to_string_id(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}-{}-{}",
            self.service_id[0], self.service_id[1], self.service_id[2], self.mcc, self.mnc
        )
    }
}

/// 5G MBS Service Type (TS 38.300 §16.15.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NrMbsServiceType {
    /// Broadcast service: open to all UEs in the MBS service area; no individual join required.
    Broadcast,
    /// Multicast service: subscription/group-join based service for UEs in RRC_CONNECTED/INACTIVE.
    Multicast,
}

/// Radio delivery mode for an MBS Radio Bearer (MRB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbsDeliveryMode {
    /// Point-to-Multipoint delivery only via MTCH mapped to DL-SCH.
    PtmOnly,
    /// Point-to-Point delivery only via dedicated DTCH mapped to DL-SCH.
    PtpOnly,
    /// Split MRB: Single PDCP entity driving both PTM and PTP legs dynamically.
    SplitMrb,
}

/// MBS Radio Bearer ID (1..32 per TS 38.331).
pub type MrbId = u8;

/// Logical Channel Identity (LCID) for MBS per TS 38.321 §6.2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MbsLogicalChannel {
    /// MBS Control Channel (MCCH)
    Mcch(u8),
    /// MBS Traffic Channel (MTCH)
    Mtch(u8),
    /// Dedicated Traffic Channel (DTCH) for PTP unicast leg
    Dtch(u8),
}

/// 5G NR Radio Network Temporary Identifier (RNTI) types for MBS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MbsRnti {
    /// Group RNTI for dynamically scheduled PTM transmissions (TS 38.321 §7.1).
    GRnti(u16),
    /// Group Configured Scheduling RNTI for Semi-Persistent Scheduling (SPS).
    GCsRnti(u16),
    /// MCCH RNTI used to scramble PDCCH scheduling MCCH (e.g. 0xFFFD).
    McchRnti(u16),
    /// Unicast C-RNTI for PTP leg.
    CRnti(u16),
}

impl MbsRnti {
    pub fn raw_value(&self) -> u16 {
        match *self {
            MbsRnti::GRnti(v) | MbsRnti::GCsRnti(v) | MbsRnti::McchRnti(v) | MbsRnti::CRnti(v) => v,
        }
    }
}

// ===========================================================================
// 2. MBS Radio Bearer (MRB) & Split MRB Architecture
// ===========================================================================

/// PDCP Sequence Number length for MRB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MrbPdcpSnSize {
    Len12Bits,
    Len18Bits,
}

impl MrbPdcpSnSize {
    pub fn max_sn(&self) -> u32 {
        match self {
            MrbPdcpSnSize::Len12Bits => 4096,
            MrbPdcpSnSize::Len18Bits => 262144,
        }
    }
}

/// Split MRB routing policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitMrbRoutingPolicy {
    /// Primary path is PTM; PTP leg is only used for retransmissions or outlier UEs.
    PrimaryPtm,
    /// Duplicate all packets across both PTM and PTP legs for maximum reliability (URLLC/critical multicast).
    Duplication,
    /// Dynamic switching based on buffer occupancy threshold in bytes.
    ThresholdBased(usize),
}

/// Configuration of an MBS Radio Bearer (MRB).
#[derive(Debug, Clone)]
pub struct MrbConfig {
    pub mrb_id: MrbId,
    pub tmgi: MbsTmgi,
    pub delivery_mode: MbsDeliveryMode,
    pub sn_size: MrbPdcpSnSize,
    pub split_policy: SplitMrbRoutingPolicy,
    pub g_rnti: u16,
    pub mtch_lcid: u8,
}

/// Transmit packet destined for lower layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MrbPdu {
    pub sn: u32,
    pub payload: Vec<u8>,
    pub leg: MbsDeliveryLeg,
}

/// Delivery leg for MRB packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MbsDeliveryLeg {
    PtmMtch {
        g_rnti: u16,
        lcid: u8,
    },
    PtpDtch {
        c_rnti: u16,
        lcid: u8,
    },
    BothPtmAndPtp {
        g_rnti: u16,
        ptm_lcid: u8,
        c_rnti: u16,
        ptp_lcid: u8,
    },
}

/// PDCP entity for an MRB supporting PTM, PTP, and Split MRB.
#[derive(Debug)]
pub struct MrbEntity {
    pub config: MrbConfig,
    next_tx_sn: u32,
    next_rx_sn: u32,
    /// Connected UEs receiving this MRB via PTP unicast leg: Map<C-RNTI, DTCH LCID>
    pub ptp_subscribers: HashMap<u16, u8>,
    /// TX queue for PTM
    pub ptm_tx_queue: VecDeque<MrbPdu>,
    /// TX queues for PTP: Map<C-RNTI, Queue>
    pub ptp_tx_queues: HashMap<u16, VecDeque<MrbPdu>>,
    /// RX reassembly / reordering buffer: Map<SN, Payload>
    rx_buffer: HashMap<u32, Vec<u8>>,
    /// Statistics
    pub stats_tx_ptm_bytes: usize,
    pub stats_tx_ptp_bytes: usize,
    pub stats_duplicated_packets: usize,
}

impl MrbEntity {
    pub fn new(config: MrbConfig) -> Self {
        Self {
            config,
            next_tx_sn: 0,
            next_rx_sn: 0,
            ptp_subscribers: HashMap::new(),
            ptm_tx_queue: VecDeque::new(),
            ptp_tx_queues: HashMap::new(),
            rx_buffer: HashMap::new(),
            stats_tx_ptm_bytes: 0,
            stats_tx_ptp_bytes: 0,
            stats_duplicated_packets: 0,
        }
    }

    /// Register a connected UE's dedicated PTP leg (C-RNTI, DTCH LCID).
    pub fn add_ptp_subscriber(&mut self, c_rnti: u16, dtch_lcid: u8) {
        self.ptp_subscribers.insert(c_rnti, dtch_lcid);
        self.ptp_tx_queues.entry(c_rnti).or_default();
    }

    /// Remove a connected UE's dedicated PTP leg.
    pub fn remove_ptp_subscriber(&mut self, c_rnti: u16) {
        self.ptp_subscribers.remove(&c_rnti);
        self.ptp_tx_queues.remove(&c_rnti);
    }

    /// Process and route an incoming MBS Service Data Unit (SDU).
    pub fn transmit_sdu(&mut self, sdu: &[u8]) -> Vec<MrbPdu> {
        let sn = self.next_tx_sn;
        let max_sn = self.config.sn_size.max_sn();
        self.next_tx_sn = (self.next_tx_sn + 1) % max_sn;

        // Build PDCP PDU with SN header
        let mut pdu_bytes = Vec::with_capacity(sdu.len() + 3);
        match self.config.sn_size {
            MrbPdcpSnSize::Len12Bits => {
                // 12-bit SN: 4 reserved bits + 12-bit SN (2 bytes)
                pdu_bytes.push(((sn >> 8) & 0x0F) as u8);
                pdu_bytes.push((sn & 0xFF) as u8);
            }
            MrbPdcpSnSize::Len18Bits => {
                // 18-bit SN: 6 reserved bits + 18-bit SN (3 bytes)
                pdu_bytes.push(((sn >> 16) & 0x03) as u8);
                pdu_bytes.push(((sn >> 8) & 0xFF) as u8);
                pdu_bytes.push((sn & 0xFF) as u8);
            }
        }
        pdu_bytes.extend_from_slice(sdu);

        let mut generated = Vec::new();

        match self.config.delivery_mode {
            MbsDeliveryMode::PtmOnly => {
                let pdu = MrbPdu {
                    sn,
                    payload: pdu_bytes.clone(),
                    leg: MbsDeliveryLeg::PtmMtch {
                        g_rnti: self.config.g_rnti,
                        lcid: self.config.mtch_lcid,
                    },
                };
                self.stats_tx_ptm_bytes += pdu.payload.len();
                self.ptm_tx_queue.push_back(pdu.clone());
                generated.push(pdu);
            }
            MbsDeliveryMode::PtpOnly => {
                for (&c_rnti, &dtch_lcid) in &self.ptp_subscribers {
                    let pdu = MrbPdu {
                        sn,
                        payload: pdu_bytes.clone(),
                        leg: MbsDeliveryLeg::PtpDtch {
                            c_rnti,
                            lcid: dtch_lcid,
                        },
                    };
                    self.stats_tx_ptp_bytes += pdu.payload.len();
                    self.ptp_tx_queues
                        .entry(c_rnti)
                        .or_default()
                        .push_back(pdu.clone());
                    generated.push(pdu);
                }
            }
            MbsDeliveryMode::SplitMrb => match self.config.split_policy {
                SplitMrbRoutingPolicy::PrimaryPtm => {
                    let pdu = MrbPdu {
                        sn,
                        payload: pdu_bytes.clone(),
                        leg: MbsDeliveryLeg::PtmMtch {
                            g_rnti: self.config.g_rnti,
                            lcid: self.config.mtch_lcid,
                        },
                    };
                    self.stats_tx_ptm_bytes += pdu.payload.len();
                    self.ptm_tx_queue.push_back(pdu.clone());
                    generated.push(pdu);
                }
                SplitMrbRoutingPolicy::Duplication => {
                    self.stats_duplicated_packets += 1;
                    // Send over PTM
                    let ptm_pdu = MrbPdu {
                        sn,
                        payload: pdu_bytes.clone(),
                        leg: MbsDeliveryLeg::PtmMtch {
                            g_rnti: self.config.g_rnti,
                            lcid: self.config.mtch_lcid,
                        },
                    };
                    self.stats_tx_ptm_bytes += ptm_pdu.payload.len();
                    self.ptm_tx_queue.push_back(ptm_pdu.clone());
                    generated.push(ptm_pdu);

                    // Duplicate over PTP for all subscribers
                    for (&c_rnti, &dtch_lcid) in &self.ptp_subscribers {
                        let ptp_pdu = MrbPdu {
                            sn,
                            payload: pdu_bytes.clone(),
                            leg: MbsDeliveryLeg::PtpDtch {
                                c_rnti,
                                lcid: dtch_lcid,
                            },
                        };
                        self.stats_tx_ptp_bytes += ptp_pdu.payload.len();
                        self.ptp_tx_queues
                            .entry(c_rnti)
                            .or_default()
                            .push_back(ptp_pdu.clone());
                        generated.push(ptp_pdu);
                    }
                }
                SplitMrbRoutingPolicy::ThresholdBased(threshold) => {
                    let ptm_q_len: usize = self.ptm_tx_queue.iter().map(|p| p.payload.len()).sum();
                    if ptm_q_len > threshold && !self.ptp_subscribers.is_empty() {
                        // Overflow routed to PTP
                        for (&c_rnti, &dtch_lcid) in &self.ptp_subscribers {
                            let pdu = MrbPdu {
                                sn,
                                payload: pdu_bytes.clone(),
                                leg: MbsDeliveryLeg::PtpDtch {
                                    c_rnti,
                                    lcid: dtch_lcid,
                                },
                            };
                            self.stats_tx_ptp_bytes += pdu.payload.len();
                            self.ptp_tx_queues
                                .entry(c_rnti)
                                .or_default()
                                .push_back(pdu.clone());
                            generated.push(pdu);
                        }
                    } else {
                        let pdu = MrbPdu {
                            sn,
                            payload: pdu_bytes.clone(),
                            leg: MbsDeliveryLeg::PtmMtch {
                                g_rnti: self.config.g_rnti,
                                lcid: self.config.mtch_lcid,
                            },
                        };
                        self.stats_tx_ptm_bytes += pdu.payload.len();
                        self.ptm_tx_queue.push_back(pdu.clone());
                        generated.push(pdu);
                    }
                }
            },
        }

        generated
    }

    /// Process received PDCP PDU from lower layer (UE reception or gNodeB reassembly).
    /// Extracts payload and handles sequence reordering and deduplication.
    pub fn receive_pdu(&mut self, pdu: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let (sn, header_len) = match self.config.sn_size {
            MrbPdcpSnSize::Len12Bits => {
                if pdu.len() < 2 {
                    return Err("PDU too short for 12-bit PDCP SN".to_string());
                }
                let sn = (((pdu[0] & 0x0F) as u32) << 8) | (pdu[1] as u32);
                (sn, 2)
            }
            MrbPdcpSnSize::Len18Bits => {
                if pdu.len() < 3 {
                    return Err("PDU too short for 18-bit PDCP SN".to_string());
                }
                let sn =
                    (((pdu[0] & 0x03) as u32) << 16) | ((pdu[1] as u32) << 8) | (pdu[2] as u32);
                (sn, 3)
            }
        };

        let sdu_payload = pdu[header_len..].to_vec();

        // Check if duplicate
        let max_sn = self.config.sn_size.max_sn();
        if sn == self.next_rx_sn {
            self.next_rx_sn = (self.next_rx_sn + 1) % max_sn;
            Ok(Some(sdu_payload))
        } else if self.is_sn_in_reorder_window(sn) {
            // Buffer out-of-order packet
            self.rx_buffer.insert(sn, sdu_payload);
            Ok(None)
        } else {
            // Out of window / duplicate already delivered
            Ok(None)
        }
    }

    fn is_sn_in_reorder_window(&self, sn: u32) -> bool {
        let max_sn = self.config.sn_size.max_sn();
        let window = max_sn / 2;
        let diff = (sn + max_sn - self.next_rx_sn) % max_sn;
        diff < window
    }
}

// ===========================================================================
// 3. Dual-Scheme HARQ Feedback for PTM
// ===========================================================================

/// PTM HARQ Feedback Scheme (3GPP TS 38.300 §16.15.3 & TS 38.213 §16.x).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtmHarqScheme {
    /// Option 1: Individual ACK/NACK feedback on dedicated PUCCH per UE.
    Option1IndividualAckNack,
    /// Option 2: Shared NACK-only feedback on common PUCCH resource.
    Option2SharedNackOnly,
}

/// State of an MBS HARQ Process.
#[derive(Debug, Clone)]
pub struct MbsHarqProcess {
    pub process_id: u8,
    pub g_rnti: u16,
    pub tb_payload: Vec<u8>,
    pub ndi: bool,
    pub rv: u8,
    pub retransmission_count: u8,
    pub max_retransmissions: u8,
    pub is_active: bool,
    /// Option 1: Tracking individual UE feedback (C-RNTI -> ACK bool)
    pub individual_feedback: HashMap<u16, bool>,
}

impl MbsHarqProcess {
    pub fn new(process_id: u8, g_rnti: u16, max_retransmissions: u8) -> Self {
        Self {
            process_id,
            g_rnti,
            tb_payload: Vec::new(),
            ndi: false,
            rv: 0,
            retransmission_count: 0,
            max_retransmissions,
            is_active: false,
            individual_feedback: HashMap::new(),
        }
    }

    /// Load a new transport block into this HARQ process.
    pub fn load_tb(&mut self, payload: Vec<u8>) {
        self.tb_payload = payload;
        self.ndi = !self.ndi;
        self.rv = 0;
        self.retransmission_count = 0;
        self.is_active = true;
        self.individual_feedback.clear();
    }
}

/// PTM HARQ Manager governing group retransmissions.
#[derive(Debug)]
pub struct PtmHarqManager {
    pub scheme: PtmHarqScheme,
    pub processes: Vec<MbsHarqProcess>,
    /// Energy detection threshold for Option 2 shared PUCCH (linear scale).
    /// If energy >= threshold => NACK detected => Retransmit.
    /// If energy < threshold  => DTX (all UEs decoded OK) => ACK assumed.
    pub shared_pucch_energy_threshold: f32,
    pub stats_total_transmissions: usize,
    pub stats_retransmissions: usize,
    pub stats_successful_tbs: usize,
    pub stats_dropped_tbs: usize,
}

impl PtmHarqManager {
    pub fn new(
        scheme: PtmHarqScheme,
        num_processes: u8,
        g_rnti: u16,
        max_retransmissions: u8,
    ) -> Self {
        let mut processes = Vec::with_capacity(num_processes as usize);
        for pid in 0..num_processes {
            processes.push(MbsHarqProcess::new(pid, g_rnti, max_retransmissions));
        }
        Self {
            scheme,
            processes,
            shared_pucch_energy_threshold: 0.25, // default normalized energy threshold
            stats_total_transmissions: 0,
            stats_retransmissions: 0,
            stats_successful_tbs: 0,
            stats_dropped_tbs: 0,
        }
    }

    /// Process Option 1 feedback: individual ACK/NACK from a specific UE.
    /// Returns true if all registered UEs have reported and all reported ACK.
    pub fn handle_option1_feedback(
        &mut self,
        process_id: u8,
        ue_c_rnti: u16,
        is_ack: bool,
        expected_subscribers: &[u16],
    ) -> Option<bool> {
        let proc = self.processes.get_mut(process_id as usize)?;
        if !proc.is_active {
            return None;
        }

        proc.individual_feedback.insert(ue_c_rnti, is_ack);

        // Check if any UE reported NACK
        if !is_ack {
            // Immediate retransmission required once scheduled
            return Some(false);
        }

        // Check if all expected UEs reported
        let all_reported = expected_subscribers
            .iter()
            .all(|ue| proc.individual_feedback.get(ue).copied() == Some(true));

        if all_reported {
            Some(true)
        } else {
            None // Still waiting for other UEs
        }
    }

    /// Process Option 2 feedback: energy detection on shared PUCCH resource.
    /// Returns true if ACK (DTX), false if NACK (energy >= threshold).
    pub fn handle_option2_feedback(
        &mut self,
        process_id: u8,
        measured_energy_linear: f32,
    ) -> Option<bool> {
        let proc = self.processes.get_mut(process_id as usize)?;
        if !proc.is_active {
            return None;
        }

        if measured_energy_linear >= self.shared_pucch_energy_threshold {
            // At least one UE sent NACK
            Some(false)
        } else {
            // No UE transmitted NACK => All decoded successfully
            Some(true)
        }
    }

    /// Trigger evaluation and advance HARQ process state after feedback.
    /// Returns `(needs_retransmit, next_rv)`.
    pub fn evaluate_and_advance(&mut self, process_id: u8, is_success: bool) -> (bool, u8) {
        let proc = match self.processes.get_mut(process_id as usize) {
            Some(p) => p,
            None => return (false, 0),
        };

        self.stats_total_transmissions += 1;

        if is_success {
            proc.is_active = false;
            self.stats_successful_tbs += 1;
            (false, 0)
        } else {
            // NACK occurred
            proc.retransmission_count += 1;
            if proc.retransmission_count > proc.max_retransmissions {
                // Exceeded limit, drop TB
                proc.is_active = false;
                self.stats_dropped_tbs += 1;
                (false, 0)
            } else {
                // Next RV in standard sequence: 0 -> 2 -> 3 -> 1
                proc.rv = match proc.rv {
                    0 => 2,
                    2 => 3,
                    3 => 1,
                    _ => 0,
                };
                self.stats_retransmissions += 1;
                (true, proc.rv)
            }
        }
    }
}

// ===========================================================================
// 4. Dynamic PTM / PTP Switching Controller
// ===========================================================================

/// Channel quality and BLER telemetry for a connected UE.
#[derive(Debug, Clone)]
pub struct UeTelemetry {
    pub c_rnti: u16,
    pub cqi: u8, // 0..15
    pub sinr_db: f32,
    pub nack_count: u32,
    pub ack_count: u32,
}

impl UeTelemetry {
    pub fn new(c_rnti: u16, cqi: u8, sinr_db: f32) -> Self {
        Self {
            c_rnti,
            cqi,
            sinr_db,
            nack_count: 0,
            ack_count: 0,
        }
    }

    pub fn bler(&self) -> f32 {
        let total = self.ack_count + self.nack_count;
        if total == 0 {
            0.0
        } else {
            self.nack_count as f32 / total as f32
        }
    }
}

/// Switching decision produced by the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchingDecision {
    /// Serve all UEs via common PTM transmission.
    AllPtm,
    /// Serve all UEs via dedicated PTP unicast transmissions.
    AllPtp,
    /// Split topology: outlier UEs moved to PTP legs, healthy UEs on PTM.
    SplitSelective {
        ptm_ues: Vec<u16>,
        ptp_isolated_ues: Vec<u16>,
    },
}

/// Dynamic PTM/PTP Controller configuration parameters.
#[derive(Debug, Clone)]
pub struct PtmPtpControllerConfig {
    /// Minimum connected UEs required to warrant PTM group transmission (e.g. 3).
    pub min_ptm_ue_count: usize,
    /// Maximum allowable group BLER before falling back to PTP (e.g. 0.20 = 20%).
    pub ptm_bler_threshold: f32,
    /// CQI threshold below which an individual UE is isolated to a PTP leg (e.g. 4).
    pub outlier_cqi_threshold: u8,
    /// Minimum duration (in evaluation cycles) before altering topology (hysteresis).
    pub hysteresis_cycles: u32,
}

/// Dynamic PTM/PTP Controller optimizing spectral efficiency and reliability.
#[derive(Debug)]
pub struct PtmPtpController {
    pub config: PtmPtpControllerConfig,
    pub ues: HashMap<u16, UeTelemetry>,
    pub current_decision: SwitchingDecision,
    cycles_in_current_state: u32,
}

impl PtmPtpController {
    pub fn new(config: PtmPtpControllerConfig) -> Self {
        Self {
            config,
            ues: HashMap::new(),
            current_decision: SwitchingDecision::AllPtm,
            cycles_in_current_state: 0,
        }
    }

    pub fn update_ue_telemetry(&mut self, telemetry: UeTelemetry) {
        self.ues.insert(telemetry.c_rnti, telemetry);
    }

    pub fn remove_ue(&mut self, c_rnti: u16) {
        self.ues.remove(&c_rnti);
    }

    /// Evaluate current UE fleet and calculate optimal delivery topology.
    pub fn evaluate(&mut self) -> SwitchingDecision {
        self.cycles_in_current_state += 1;

        let total_ues = self.ues.len();

        // 1. If UE count is below minimum threshold, PTP unicast is more resource efficient
        if total_ues < self.config.min_ptm_ue_count {
            let decision = SwitchingDecision::AllPtp;
            return self.apply_with_hysteresis(decision);
        }

        // 2. Separate healthy UEs and outlier UEs based on CQI threshold
        let mut ptm_candidates = Vec::new();
        let mut outlier_ues = Vec::new();

        for (&c_rnti, ue) in &self.ues {
            if ue.cqi <= self.config.outlier_cqi_threshold {
                outlier_ues.push(c_rnti);
            } else {
                ptm_candidates.push(c_rnti);
            }
        }

        // 3. Evaluate group BLER
        let total_nack: u32 = self.ues.values().map(|u| u.nack_count).sum();
        let total_trans: u32 = self.ues.values().map(|u| u.ack_count + u.nack_count).sum();
        let group_bler = if total_trans == 0 {
            0.0
        } else {
            total_nack as f32 / total_trans as f32
        };

        if group_bler > self.config.ptm_bler_threshold {
            // Group channel degraded, switch to AllPtp
            return self.apply_with_hysteresis(SwitchingDecision::AllPtp);
        }

        // 4. If there are outliers but still enough UEs for PTM, do SplitSelective
        if !outlier_ues.is_empty() && ptm_candidates.len() >= self.config.min_ptm_ue_count {
            ptm_candidates.sort();
            outlier_ues.sort();
            let decision = SwitchingDecision::SplitSelective {
                ptm_ues: ptm_candidates,
                ptp_isolated_ues: outlier_ues,
            };
            return self.apply_with_hysteresis(decision);
        }

        // 5. If outliers leave fewer than min_ptm_ue_count, fall back to AllPtp
        if ptm_candidates.len() < self.config.min_ptm_ue_count {
            return self.apply_with_hysteresis(SwitchingDecision::AllPtp);
        }

        // 6. Otherwise, everyone is healthy for AllPtm
        self.apply_with_hysteresis(SwitchingDecision::AllPtm)
    }

    fn apply_with_hysteresis(&mut self, proposed: SwitchingDecision) -> SwitchingDecision {
        if proposed == self.current_decision {
            self.current_decision.clone()
        } else if self.cycles_in_current_state >= self.config.hysteresis_cycles {
            self.current_decision = proposed.clone();
            self.cycles_in_current_state = 0;
            proposed
        } else {
            // Retain previous state until hysteresis timer satisfies
            self.current_decision.clone()
        }
    }
}

// ===========================================================================
// 5. MCCH State Machine & Information Broadcast
// ===========================================================================

/// MCCH configuration parameters (3GPP TS 38.331 §6.3.2).
#[derive(Debug, Clone)]
pub struct McchConfig {
    /// Repetition period in radio frames (10ms each, e.g. 32 frames = 320ms)
    pub repetition_period_frames: u32,
    /// Modification period in radio frames (e.g. 512 frames = 5.12s)
    pub modification_period_frames: u32,
    /// Radio frame offset (0..repetition_period - 1)
    pub offset_frames: u32,
    /// Subframe allocation bitmap within the radio frame
    pub subframe_allocation: u16,
}

/// Service session metadata transmitted on MCCH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbsSessionInfo {
    pub tmgi: MbsTmgi,
    pub session_id: Option<u8>,
    pub g_rnti: u16,
    pub g_cs_rnti: Option<u16>,
    pub mtch_lcid: u8,
    pub service_type: NrMbsServiceType,
    pub fsai_list: Vec<u32>, // Frequency Selective Area Identifiers
}

/// MCCH State Machine governing broadcast modification and short message signaling.
#[derive(Debug)]
pub struct McchStateMachine {
    pub config: McchConfig,
    /// Active sessions currently broadcast
    pub active_sessions: Vec<MbsSessionInfo>,
    /// Pending sessions staged for next modification period boundary
    pub pending_sessions: Option<Vec<MbsSessionInfo>>,
    /// SFN boundary at which pending sessions become active
    pub target_activation_sfn: Option<u32>,
    /// Current system frame number (SFN: 0..1023)
    pub current_sfn: u32,
    /// Short Message notification flag (TS 38.331 §6.5)
    pub short_message_mcch_indication: bool,
    /// Count of MCCH transmissions
    pub stats_mcch_transmissions: usize,
}

impl McchStateMachine {
    pub fn new(config: McchConfig) -> Self {
        Self {
            config,
            active_sessions: Vec::new(),
            pending_sessions: None,
            target_activation_sfn: None,
            current_sfn: 0,
            short_message_mcch_indication: false,
            stats_mcch_transmissions: 0,
        }
    }

    /// Schedule an update to the broadcast configuration.
    /// Updates only take effect at the next modification period boundary.
    pub fn update_sessions(&mut self, new_sessions: Vec<MbsSessionInfo>) {
        let current_period = self.current_sfn / self.config.modification_period_frames;
        let next_boundary = (current_period + 1) * self.config.modification_period_frames;
        self.target_activation_sfn = Some(next_boundary % 1024);
        self.pending_sessions = Some(new_sessions);
        // Set Short Message MCCH indication bit to notify UEs in advance
        self.short_message_mcch_indication = true;
    }

    /// Advance frame clock by 1 SFN (10ms).
    /// Returns `Some(serialized_mcch)` if an MCCH transmission occurs in this frame.
    pub fn step_frame(&mut self) -> Option<Vec<u8>> {
        let sfn = self.current_sfn;
        self.current_sfn = (self.current_sfn + 1) % 1024;

        // Check modification period boundary
        if let Some(target_sfn) = self.target_activation_sfn {
            if sfn == target_sfn {
                if let Some(staged) = self.pending_sessions.take() {
                    self.active_sessions = staged;
                    self.short_message_mcch_indication = false;
                    self.target_activation_sfn = None;
                }
            }
        }

        // Check repetition period boundary
        let is_repetition_frame =
            (sfn % self.config.repetition_period_frames) == self.config.offset_frames;
        if is_repetition_frame {
            self.stats_mcch_transmissions += 1;
            Some(self.serialize_mcch_pdu())
        } else {
            None
        }
    }

    /// Pure Rust serialization of MCCH `MBSBroadcastConfiguration` wire message.
    pub fn serialize_mcch_pdu(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Byte 0: Magic / Protocol Discriminator 0x5B (MBS)
        buf.push(0x5B);
        // Byte 1: Version and active session count
        buf.push(self.active_sessions.len() as u8);

        for session in &self.active_sessions {
            // TMGI service ID (3 bytes)
            buf.extend_from_slice(&session.tmgi.service_id);
            // G-RNTI (2 bytes)
            buf.extend_from_slice(&session.g_rnti.to_be_bytes());
            // MTCH LCID (1 byte)
            buf.push(session.mtch_lcid);
            // Service Type (1 byte: 0 = Broadcast, 1 = Multicast)
            buf.push(match session.service_type {
                NrMbsServiceType::Broadcast => 0x00,
                NrMbsServiceType::Multicast => 0x01,
            });
            // FSAI count (1 byte)
            buf.push(session.fsai_list.len() as u8);
            for &fsai in &session.fsai_list {
                buf.extend_from_slice(&fsai.to_be_bytes());
            }
        }

        buf
    }

    /// Pure Rust deserialization of MCCH `MBSBroadcastConfiguration` wire message.
    pub fn deserialize_mcch_pdu(bytes: &[u8]) -> Result<Vec<MbsSessionInfo>, String> {
        if bytes.len() < 2 {
            return Err("MCCH PDU too short".to_string());
        }
        if bytes[0] != 0x5B {
            return Err(format!("Invalid MCCH magic: 0x{:02x}", bytes[0]));
        }

        let session_count = bytes[1] as usize;
        let mut offset = 2;
        let mut sessions = Vec::with_capacity(session_count);

        for _ in 0..session_count {
            if offset + 8 > bytes.len() {
                return Err("Truncated MCCH session item".to_string());
            }

            let service_id = [bytes[offset], bytes[offset + 1], bytes[offset + 2]];
            offset += 3;

            let g_rnti = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
            offset += 2;

            let mtch_lcid = bytes[offset];
            offset += 1;

            let service_type = if bytes[offset] == 0 {
                NrMbsServiceType::Broadcast
            } else {
                NrMbsServiceType::Multicast
            };
            offset += 1;

            let fsai_count = bytes[offset] as usize;
            offset += 1;

            if offset + fsai_count * 4 > bytes.len() {
                return Err("Truncated FSAI list in MCCH".to_string());
            }

            let mut fsai_list = Vec::with_capacity(fsai_count);
            for _ in 0..fsai_count {
                let fsai = u32::from_be_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                fsai_list.push(fsai);
                offset += 4;
            }

            sessions.push(MbsSessionInfo {
                tmgi: MbsTmgi::new(service_id, "001", "01"),
                session_id: None,
                g_rnti,
                g_cs_rnti: None,
                mtch_lcid,
                service_type,
                fsai_list,
            });
        }

        Ok(sessions)
    }
}

/// MBS Interest Indication (MII) message sent by UE (3GPP TS 38.331 §5.3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbsInterestIndication {
    pub ue_c_rnti: u16,
    pub interested_tmgis: Vec<MbsTmgi>,
    pub priority: u8,
}

// ===========================================================================
// 6. MBS Discontinuous Reception (DRX) Engine
// ===========================================================================

/// MBS DRX configuration for PTM reception (3GPP TS 38.321 §5.7).
#[derive(Debug, Clone)]
pub struct MbsDrxConfig {
    /// Duration that UE monitors PDCCH with G-RNTI at beginning of cycle (in slots)
    pub on_duration_slots: u16,
    /// Duration UE remains active after receiving PDCCH grant (in slots)
    pub inactivity_slots: u16,
    /// HARQ Round Trip Time timer (in slots)
    pub harq_rtt_slots: u16,
    /// Retransmission monitoring timer (in slots)
    pub retransmission_slots: u16,
    /// DRX Cycle length (in slots)
    pub cycle_slots: u16,
    /// Slot offset for cycle start
    pub slot_offset: u16,
}

/// MBS DRX Engine tracking active time.
#[derive(Debug)]
pub struct MbsDrxEngine {
    pub config: MbsDrxConfig,
    pub current_slot: u32,
    on_duration_timer: u16,
    inactivity_timer: u16,
    harq_rtt_timer: u16,
    retransmission_timer: u16,
}

impl MbsDrxEngine {
    pub fn new(config: MbsDrxConfig) -> Self {
        Self {
            config,
            current_slot: 0,
            on_duration_timer: 0,
            inactivity_timer: 0,
            harq_rtt_timer: 0,
            retransmission_timer: 0,
        }
    }

    /// Advance time by 1 slot.
    pub fn step_slot(&mut self) {
        let slot_in_cycle = (self.current_slot % self.config.cycle_slots as u32) as u16;

        // Check if cycle starts
        if slot_in_cycle == self.config.slot_offset {
            self.on_duration_timer = self.config.on_duration_slots;
        } else if self.on_duration_timer > 0 {
            self.on_duration_timer -= 1;
        }

        if self.inactivity_timer > 0 {
            self.inactivity_timer -= 1;
        }

        if self.harq_rtt_timer > 0 {
            self.harq_rtt_timer -= 1;
            if self.harq_rtt_timer == 0 {
                // RTT expired, start retransmission timer
                self.retransmission_timer = self.config.retransmission_slots;
            }
        }

        if self.retransmission_timer > 0 {
            self.retransmission_timer -= 1;
        }

        self.current_slot = self.current_slot.wrapping_add(1);
    }

    /// Event: PDCCH grant received on G-RNTI.
    pub fn on_pdcch_grant(&mut self, is_new_transmission: bool) {
        if is_new_transmission {
            self.inactivity_timer = self.config.inactivity_slots;
            self.harq_rtt_timer = self.config.harq_rtt_slots;
            self.retransmission_timer = 0;
        } else {
            // Retransmission received
            self.retransmission_timer = 0;
        }
    }

    /// Evaluates if the UE is in Active Time during the current slot.
    /// In Active Time, UE must monitor PDCCH for G-RNTI.
    pub fn is_active_time(&self) -> bool {
        self.on_duration_timer > 0 || self.inactivity_timer > 0 || self.retransmission_timer > 0
    }
}

// ===========================================================================
// 7. MBS MAC PDU Framing & Multiplexing
// ===========================================================================

/// MAC Subheader LCID constants for MBS (3GPP TS 38.321 §6.2.1).
pub const LCID_MCCH: u8 = 0x20;
pub const LCID_PADDING: u8 = 0x3F;

/// Individual SDU entry in an MBS MAC PDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MbsMacSdu {
    pub lcid: u8,
    pub data: Vec<u8>,
}

/// MBS MAC PDU Multiplexer & Demultiplexer.
pub struct MbsMacMultiplexer;

impl MbsMacMultiplexer {
    /// Formats an array of SDUs into a single MAC PDU with variable length subheaders.
    /// TS 38.321 §6.1.2:
    /// Subheader format:
    /// - 1-byte header: R/F/LCID (if length <= 255: [0, 0, LCID], followed by 1-byte L)
    /// - 2-byte header: R/F/LCID (if length > 255: [0, 1, LCID], followed by 2-byte L)
    pub fn encode_mac_pdu(sdus: &[MbsMacSdu], padding_bytes: usize) -> Vec<u8> {
        let mut header_bytes = Vec::new();
        let mut payload_bytes = Vec::new();

        for sdu in sdus {
            let len = sdu.data.len();
            if len <= 255 {
                // F = 0 (8-bit length field)
                let octet0 = sdu.lcid & 0x3F;
                header_bytes.push(octet0);
                header_bytes.push(len as u8);
            } else {
                // F = 1 (16-bit length field)
                let octet0 = 0x40 | (sdu.lcid & 0x3F);
                header_bytes.push(octet0);
                header_bytes.push(((len >> 8) & 0xFF) as u8);
                header_bytes.push((len & 0xFF) as u8);
            }
            payload_bytes.extend_from_slice(&sdu.data);
        }

        // Add padding subheader if requested
        if padding_bytes > 0 {
            header_bytes.push(LCID_PADDING & 0x3F);
            // Padding data (0x00)
            payload_bytes.extend(std::iter::repeat(0x00).take(padding_bytes.saturating_sub(1)));
        }

        let mut mac_pdu = Vec::with_capacity(header_bytes.len() + payload_bytes.len());
        mac_pdu.extend(header_bytes);
        mac_pdu.extend(payload_bytes);
        mac_pdu
    }

    /// Parses a raw MAC PDU into discrete SDUs.
    pub fn decode_mac_pdu(raw: &[u8]) -> Result<Vec<MbsMacSdu>, String> {
        let mut sdus = Vec::new();
        let mut offset = 0;

        // Pass 1: Parse subheaders
        struct Subheader {
            lcid: u8,
            length: usize,
        }
        let mut subheaders = Vec::new();

        while offset < raw.len() {
            let octet0 = raw[offset];
            offset += 1;
            let lcid = octet0 & 0x3F;

            if lcid == LCID_PADDING {
                // Remainder of PDU is padding
                break;
            }

            let is_16bit = (octet0 & 0x40) != 0;
            let length = if is_16bit {
                if offset + 2 > raw.len() {
                    return Err("Truncated 16-bit subheader length".to_string());
                }
                let len = ((raw[offset] as usize) << 8) | (raw[offset + 1] as usize);
                offset += 2;
                len
            } else {
                if offset >= raw.len() {
                    return Err("Truncated 8-bit subheader length".to_string());
                }
                let len = raw[offset] as usize;
                offset += 1;
                len
            };

            subheaders.push(Subheader { lcid, length });
        }

        // Pass 2: Extract SDU payloads
        for sh in subheaders {
            if offset + sh.length > raw.len() {
                return Err("Truncated MAC SDU payload in PDU".to_string());
            }
            let data = raw[offset..offset + sh.length].to_vec();
            offset += sh.length;
            sdus.push(MbsMacSdu {
                lcid: sh.lcid,
                data,
            });
        }

        Ok(sdus)
    }
}
