//! 3GPP Rel-17 5G NR Paging Early Indication (PEI) & Idle/Inactive Energy Saving Engine
//! (TS 38.213 §10.5, TS 38.304 §7.4, and TS 38.331).

/// Default RNTI for Paging Early Indication (PEI-RNTI).
pub const PEI_RNTI_DEFAULT: u16 = 0xFFFE;

/// Maximum number of radio frames in a 5G NR hyper-system frame / SFN cycle.
pub const MAX_SFN: u32 = 1024;

// ===========================================================================
// 1. PEI Configuration & Subgrouping Schemes
// ===========================================================================

/// Scheme for assigning UEs to paging subgroups (TS 38.304 §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgroupingScheme {
    /// Core Network (CN) assigned paging subgroup ID (0..N-1) provided via NAS.
    CnAssigned,
    /// Autonomous UE-ID based hash subgrouping derived from 5G-S-TMSI.
    UeIdBased,
}

/// SIB1 PEI Configuration Parameters (TS 38.331 `PEI-Config-r17`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeiConfig {
    /// Frame offset in radio frames between PEI monitoring occasion and first associated PF (1..4).
    pub pei_frame_offset: u16,
    /// Total payload size of DCI format 2_7 in bits (e.g. 8..32 bits).
    pub payload_size_dci_2_7: u8,
    /// Number of paging subgroups configured per Paging Occasion (1, 2, 4, 8).
    pub subgroups_per_po: u8,
    /// Number of Paging Occasions associated with a single PEI occasion (1, 2, 4).
    pub pos_per_pei: u8,
    /// Indicates presence of the Short Message field in DCI format 2_7.
    pub short_message_present: bool,
    /// Configured subgrouping scheme.
    pub subgrouping_scheme: SubgroupingScheme,
}

impl Default for PeiConfig {
    fn default() -> Self {
        Self {
            pei_frame_offset: 2,
            payload_size_dci_2_7: 16,
            subgroups_per_po: 4,
            pos_per_pei: 2,
            short_message_present: true,
            subgrouping_scheme: SubgroupingScheme::UeIdBased,
        }
    }
}

// ===========================================================================
// 2. UE Subgrouping Derivation Engine
// ===========================================================================

/// Subgroup calculation engine compliant with TS 38.304 §7.4.
pub struct PeiSubgroupEngine;

impl PeiSubgroupEngine {
    /// Calculates the paging subgroup ID for a UE given its 5G-S-TMSI / UE-ID.
    ///
    /// Per TS 38.304 §7.4:
    /// - If `CnAssigned` and valid CN subgroup ID is present, returns `cn_subgroup`.
    /// - Otherwise (or fallback), uses UE-ID hash formula:
    ///   `SubgroupId = floor((UE_ID mod 4096) * N_subgroup / 4096)`
    pub fn calculate_subgroup_id(
        ue_id: u64,
        scheme: SubgroupingScheme,
        cn_subgroup: Option<u8>,
        num_subgroups: u8,
    ) -> u8 {
        if num_subgroups <= 1 {
            return 0;
        }

        match scheme {
            SubgroupingScheme::CnAssigned => {
                if let Some(sg) = cn_subgroup {
                    if sg < num_subgroups {
                        return sg;
                    }
                }
                // Fallback to UE-ID hashing if CN assignment is absent or out of bounds
                Self::calculate_ue_id_hash_subgroup(ue_id, num_subgroups)
            }
            SubgroupingScheme::UeIdBased => {
                Self::calculate_ue_id_hash_subgroup(ue_id, num_subgroups)
            }
        }
    }

    /// Autonomous UE-ID hash subgroup derivation per TS 38.304 §7.4 formula:
    /// `SubgroupId = floor((UE_ID mod 4096) * N_subgroup / 4096)`
    #[inline]
    pub fn calculate_ue_id_hash_subgroup(ue_id: u64, num_subgroups: u8) -> u8 {
        if num_subgroups <= 1 {
            return 0;
        }
        let hash_input = (ue_id % 4096) as f64;
        let subgroup = (hash_input * (num_subgroups as f64) / 4096.0).floor() as u8;
        subgroup.min(num_subgroups - 1)
    }
}

// ===========================================================================
// 3. DCI Format 2_7 Wire Codec (TS 38.212 §7.3.1.3.5 & TS 38.213 §10.5)
// ===========================================================================

/// DCI Format 2_7 Representation for Paging Early Indication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DciFormat2_7 {
    /// Total bit length of the DCI payload
    pub total_bits: usize,
    /// Bit vector representing raw payload (MSB first)
    pub bits: Vec<bool>,
}

impl DciFormat2_7 {
    /// Creates an empty DCI 2_7 payload initialized to 0.
    pub fn new(total_bits: usize) -> Self {
        Self {
            total_bits,
            bits: vec![false; total_bits],
        }
    }

    /// Calculates bit offset for a specific PO and Subgroup index.
    #[inline]
    fn get_subgroup_bit_offset(po_idx: usize, subgroup_id: u8, subgroups_per_po: u8) -> usize {
        (po_idx * (subgroups_per_po as usize)) + (subgroup_id as usize)
    }

    /// Sets the paging indication bit for a specific PO index and Subgroup ID.
    pub fn set_subgroup_indication(
        &mut self,
        po_idx: usize,
        subgroup_id: u8,
        subgroups_per_po: u8,
        indicate_paging: bool,
    ) -> Result<(), String> {
        if subgroup_id >= subgroups_per_po {
            return Err(format!(
                "Subgroup ID {} exceeds subgroups per PO {}",
                subgroup_id, subgroups_per_po
            ));
        }
        let bit_offset = Self::get_subgroup_bit_offset(po_idx, subgroup_id, subgroups_per_po);
        if bit_offset >= self.total_bits {
            return Err(format!(
                "Bit offset {} exceeds total DCI 2_7 bits {}",
                bit_offset, self.total_bits
            ));
        }
        self.bits[bit_offset] = indicate_paging;
        Ok(())
    }

    /// Queries the paging indication bit for a specific PO index and Subgroup ID.
    pub fn get_subgroup_indication(
        &self,
        po_idx: usize,
        subgroup_id: u8,
        subgroups_per_po: u8,
    ) -> Result<bool, String> {
        if subgroup_id >= subgroups_per_po {
            return Err(format!(
                "Subgroup ID {} exceeds subgroups per PO {}",
                subgroup_id, subgroups_per_po
            ));
        }
        let bit_offset = Self::get_subgroup_bit_offset(po_idx, subgroup_id, subgroups_per_po);
        if bit_offset >= self.total_bits {
            return Err(format!(
                "Bit offset {} exceeds total DCI 2_7 bits {}",
                bit_offset, self.total_bits
            ));
        }
        Ok(self.bits[bit_offset])
    }

    /// Sets the optional Short Message indication bit and 8-bit message payload.
    ///
    /// Bit layout at `start_bit_offset`:
    /// - Bit 0: Short Message Indicator (1 = short message present)
    /// - Bits 1..8: Short Message octet (TS 38.331 Table 6.5-1)
    pub fn set_short_message(
        &mut self,
        start_bit_offset: usize,
        short_msg_indicator: bool,
        message: u8,
    ) -> Result<(), String> {
        if start_bit_offset + 9 > self.total_bits {
            return Err("Short message fields exceed DCI 2_7 payload capacity".to_string());
        }
        self.bits[start_bit_offset] = short_msg_indicator;
        for i in 0..8 {
            self.bits[start_bit_offset + 1 + i] = ((message >> (7 - i)) & 1) == 1;
        }
        Ok(())
    }

    /// Extracts the Short Message indicator and optional 8-bit message payload.
    pub fn get_short_message(&self, start_bit_offset: usize) -> Result<(bool, u8), String> {
        if start_bit_offset + 9 > self.total_bits {
            return Err("Short message fields exceed DCI 2_7 payload capacity".to_string());
        }
        let indicator = self.bits[start_bit_offset];
        let mut msg = 0u8;
        for i in 0..8 {
            if self.bits[start_bit_offset + 1 + i] {
                msg |= 1 << (7 - i);
            }
        }
        Ok((indicator, msg))
    }

    /// Encodes bit payload into a byte array (MSB first in each octet).
    pub fn to_bytes(&self) -> Vec<u8> {
        let byte_len = (self.total_bits + 7) / 8;
        let mut bytes = vec![0u8; byte_len];
        for (i, &bit) in self.bits.iter().enumerate() {
            if bit {
                let byte_idx = i / 8;
                let bit_in_byte = 7 - (i % 8);
                bytes[byte_idx] |= 1 << bit_in_byte;
            }
        }
        bytes
    }

    /// Decodes a byte stream into `DciFormat2_7`.
    pub fn from_bytes(bytes: &[u8], total_bits: usize) -> Result<Self, String> {
        let required_bytes = (total_bits + 7) / 8;
        if bytes.len() < required_bytes {
            return Err(format!(
                "Payload length {} bytes insufficient for {} bits",
                bytes.len(),
                total_bits
            ));
        }

        let mut bits = Vec::with_capacity(total_bits);
        for i in 0..total_bits {
            let byte_idx = i / 8;
            let bit_in_byte = 7 - (i % 8);
            let bit = (bytes[byte_idx] & (1 << bit_in_byte)) != 0;
            bits.push(bit);
        }

        Ok(Self { total_bits, bits })
    }
}

// ===========================================================================
// 4. PEI Occasion Timing Calculator (TS 38.304 §7.1 / TS 38.213 §10.5)
// ===========================================================================

/// Timing Coordinator for Paging Frames (PF) and PEI Monitoring Occasions.
pub struct PeiTimingCalculator;

impl PeiTimingCalculator {
    /// Computes Paging Frame (PF) SFN per TS 38.304 §7.1:
    /// `(SFN + PF_offset) mod T = (T div N) * (UE_ID mod N)`
    pub fn calculate_paging_frame(
        ue_id: u64,
        drx_cycle_frames: u32,
        n_param: u32,
        pf_offset: u32,
    ) -> u32 {
        let t = drx_cycle_frames.max(1);
        let n = n_param.max(1);
        let target_val = (t / n) * ((ue_id % (n as u64)) as u32);
        let offset = pf_offset % t;

        // Solve (SFN + offset) % T = target_val
        let sfn_within_cycle = (target_val + t - offset) % t;
        sfn_within_cycle % MAX_SFN
    }

    /// Calculates Paging Occasion index `i_s` within the Paging Frame:
    /// `i_s = floor(UE_ID / N) mod Ns` (TS 38.304 §7.1)
    pub fn calculate_po_index(ue_id: u64, n_param: u32, ns_param: u32) -> usize {
        let n = (n_param as u64).max(1);
        let ns = (ns_param as u64).max(1);
        ((ue_id / n) % ns) as usize
    }

    /// Computes PEI Monitoring Frame SFN per TS 38.213 §10.5:
    /// `SFN_PEI = (SFN_PF - pei_frame_offset + 1024) mod 1024`
    pub fn calculate_pei_frame(pf_sfn: u32, pei_frame_offset: u16) -> u32 {
        let offset = (pei_frame_offset as u32) % MAX_SFN;
        (pf_sfn + MAX_SFN - offset) % MAX_SFN
    }
}

// ===========================================================================
// 5. PEI Decision & Energy Savings Simulator
// ===========================================================================

/// Wakeup Decision issued to the UE PHY / MAC receiver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeiWakeupDecision {
    /// Subgroup indication bit is 1: UE must wake up to decode PDCCH for paging.
    WakeUpPaging { po_index: usize, subgroup_id: u8 },
    /// Subgroup indication bit is 0: UE skips PDCCH monitoring on PO (retains deep sleep).
    SkipPaging { po_index: usize, subgroup_id: u8 },
    /// Short Message indication is present: All UEs wake up regardless of subgroup.
    WakeUpShortMessage { short_message: u8 },
    /// PEI DTX or CRC decode failure: Fallback to conventional legacy wake-up.
    WakeUpFallback { reason: String },
}

/// Statistics and energy consumption tracking for PEI operations.
#[derive(Debug, Clone, Default)]
pub struct PeiPerformanceMetrics {
    pub total_pei_evaluated: u64,
    pub paging_wakeups: u64,
    pub paging_skipped: u64,
    pub short_message_wakeups: u64,
    pub fallback_wakeups: u64,
}

/// UE Receiver model evaluating PEI and tracking power consumption.
#[derive(Debug, Clone)]
pub struct PeiUeReceiver {
    pub ue_id: u64,
    pub cn_subgroup: Option<u8>,
    pub metrics: PeiPerformanceMetrics,
}

impl PeiUeReceiver {
    pub fn new(ue_id: u64, cn_subgroup: Option<u8>) -> Self {
        Self {
            ue_id,
            cn_subgroup,
            metrics: PeiPerformanceMetrics::default(),
        }
    }

    /// Evaluates incoming PEI DCI 2_7 and determines whether to wake up for PO.
    pub fn evaluate_pei(
        &mut self,
        dci_option: Option<&DciFormat2_7>,
        config: &PeiConfig,
        po_idx: usize,
    ) -> PeiWakeupDecision {
        self.metrics.total_pei_evaluated += 1;

        let dci = match dci_option {
            Some(d) => d,
            None => {
                self.metrics.fallback_wakeups += 1;
                return PeiWakeupDecision::WakeUpFallback {
                    reason: "PEI DCI 2_7 not detected / DTX".to_string(),
                };
            }
        };

        // 1. Check Short Message Indication first if configured
        if config.short_message_present {
            // Short message is located after all subgroup bits
            let total_subgroup_bits =
                (config.pos_per_pei as usize) * (config.subgroups_per_po as usize);
            if let Ok((has_short_msg, msg_val)) = dci.get_short_message(total_subgroup_bits) {
                if has_short_msg {
                    self.metrics.short_message_wakeups += 1;
                    return PeiWakeupDecision::WakeUpShortMessage {
                        short_message: msg_val,
                    };
                }
            }
        }

        // 2. Determine UE subgroup ID
        let subgroup_id = PeiSubgroupEngine::calculate_subgroup_id(
            self.ue_id,
            config.subgrouping_scheme,
            self.cn_subgroup,
            config.subgroups_per_po,
        );

        // 3. Query subgroup bit
        match dci.get_subgroup_indication(po_idx, subgroup_id, config.subgroups_per_po) {
            Ok(true) => {
                self.metrics.paging_wakeups += 1;
                PeiWakeupDecision::WakeUpPaging {
                    po_index: po_idx,
                    subgroup_id,
                }
            }
            Ok(false) => {
                self.metrics.paging_skipped += 1;
                PeiWakeupDecision::SkipPaging {
                    po_index: po_idx,
                    subgroup_id,
                }
            }
            Err(err) => {
                self.metrics.fallback_wakeups += 1;
                PeiWakeupDecision::WakeUpFallback {
                    reason: format!("DCI query error: {}", err),
                }
            }
        }
    }

    /// Calculates energy savings percentage relative to legacy Rel-15/16 paging
    /// (where the UE must wake up on EVERY PO).
    ///
    /// `pei_cost_uj`: Energy to decode PEI occasion in micro-Joules (e.g. 10 uJ).
    /// `po_cost_uj`: Energy to decode legacy PO PDCCH search space (e.g. 100 uJ).
    pub fn calculate_energy_savings_percentage(&self, pei_cost_uj: f64, po_cost_uj: f64) -> f64 {
        if self.metrics.total_pei_evaluated == 0 {
            return 0.0;
        }

        let total = self.metrics.total_pei_evaluated as f64;
        let legacy_energy = total * po_cost_uj;

        let pei_wakeups = (self.metrics.paging_wakeups
            + self.metrics.short_message_wakeups
            + self.metrics.fallback_wakeups) as f64;

        let pei_total_energy = (total * pei_cost_uj) + (pei_wakeups * po_cost_uj);

        if pei_total_energy >= legacy_energy {
            0.0
        } else {
            ((legacy_energy - pei_total_energy) / legacy_energy) * 100.0
        }
    }
}
