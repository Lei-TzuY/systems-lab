//! 3GPP Rel-17 5G NR Beam Failure Detection (BFD) & Beam Failure Recovery (BFR) Engine.
//!
//! Compliant with 3GPP TS 38.321 Rel-17 Section 5.17, TS 38.214 §5.1.1, and TS 38.331.
//!
//! Essential for 5G NR FR2 (mmWave 24-71 GHz) and FR1 Massive MIMO:
//! - Sub-millisecond Beam Failure Instance (BFI) tracking when link quality falls below Q_out.
//! - BFI Counter & soak timer (beamFailureDetectionTimer) management.
//! - Candidate beam identification ($q_1$) meeting L1-RSRP threshold Q_in.
//! - Dual recovery signaling: Contention-Free RACH (CFRA) and BFR MAC CE (TS 38.321 §6.1.3.23).
//! - Dedicated recovery search space PDCCH confirmation and active TCI beam switchover.
//! - Fallback to Radio Link Failure (RLF) on recovery timer expiration.

// ---------------------------------------------------------------------------
// Types & Configuration (TS 38.331 `BeamFailureRecoveryConfig`)
// ---------------------------------------------------------------------------

/// Type of reference signal used for beam tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceSignalType {
    /// Synchronization Signal Block (SSB / PBCH block).
    Ssb,
    /// Channel State Information Reference Signal (CSI-RS).
    CsiRs,
}

/// Unique Beam Identifier (SSB index 0..63 or CSI-RS Resource Indicator CRI 0..63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BeamIdentifier {
    pub signal_type: ReferenceSignalType,
    pub id: u8,
}

impl BeamIdentifier {
    pub fn ssb(id: u8) -> Self {
        Self {
            signal_type: ReferenceSignalType::Ssb,
            id,
        }
    }

    pub fn csi_rs(id: u8) -> Self {
        Self {
            signal_type: ReferenceSignalType::CsiRs,
            id,
        }
    }
}

/// L1 Radio Link Measurement for a specific beam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeamMeasurement {
    pub beam: BeamIdentifier,
    /// L1-RSRP in dBm (-140 dBm .. -40 dBm).
    pub rsrp_dbm: i16,
}

/// Configured Candidate Beam in set q_1 (TS 38.321 §5.17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateBeamConfig {
    pub beam: BeamIdentifier,
    /// Dedicated Contention-Free PRACH preamble for this candidate beam (if configured).
    pub dedicated_preamble_index: Option<u8>,
    /// Target PRACH occasion slot offset.
    pub prach_occasion_slot: Option<u16>,
}

/// Beam Failure Recovery Configuration (TS 38.331 `BeamFailureRecoveryConfig`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeamFailureRecoveryConfig {
    /// Maximum consecutive BFI events before declaring failure (1..8, typically 3 or 4).
    pub bfi_max_count: u16,
    /// Soak timer duration in slots resetting BFI counter if healthy (e.g. 20 slots).
    pub bfd_timer_slots: u16,
    /// Max duration in slots awaiting gNodeB response in recovery search space (e.g. 40 slots).
    pub bfr_timer_slots: u16,
    /// Q_out threshold in dBm (serving link quality below which BFI is triggered, e.g. -110 dBm).
    pub q_out_threshold_dbm: i16,
    /// Q_in threshold in dBm (minimum L1-RSRP for viable candidate beam, e.g. -90 dBm).
    pub q_in_threshold_dbm: i16,
    /// Set q_0: Active serving beam reference signals.
    pub q0_serving_beams: Vec<BeamIdentifier>,
    /// Set q_1: Configured candidate beam set.
    pub q1_candidate_beams: Vec<CandidateBeamConfig>,
    /// Dedicated search space ID monitored for recovery response.
    pub recovery_search_space_id: u8,
}

/// Beam Failure Recovery Engine Operational State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfrState {
    /// Normal: Serving beam radio link quality healthy.
    Normal,
    /// Evaluating: Consecutive BFI occurrences accumulating.
    EvaluatingFailure,
    /// AwaitingResponse: BFR request transmitted, monitoring recovery search space.
    AwaitingResponse {
        candidate_beam: BeamIdentifier,
        transmission_type: BfrTransmissionType,
    },
    /// Recovered: Target beam confirmed by gNodeB; TCI state updated.
    Recovered { active_tci_beam: BeamIdentifier },
    /// RadioLinkFailure: Recovery timer expired without confirmation.
    RadioLinkFailure,
}

/// Transmission method chosen for the Beam Failure Recovery Request (BFRQ).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfrTransmissionType {
    /// Dedicated Contention-Free Random Access (CFRA) on candidate beam.
    CfraPreamble { preamble_index: u8, prach_slot: u16 },
    /// BFR MAC Control Element (TS 38.321 §6.1.3.23).
    BfrMacCe { mac_ce_payload: Vec<u8> },
    /// Contention-Based Random Access fallback.
    CbraFallback,
}

/// Event emitted by the BFR state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BfrEvent {
    /// Beam Failure Instance (BFI) detected by PHY.
    BfiDetected { counter: u16 },
    /// Beam Failure declared across all serving beams.
    BeamFailureDeclared,
    /// Recovery request dispatched to PHY/MAC layer.
    RecoveryRequestDispatched {
        candidate_beam: BeamIdentifier,
        transmission: BfrTransmissionType,
    },
    /// Successful recovery confirmed; active beam switched.
    RecoverySuccess {
        old_beam: BeamIdentifier,
        new_beam: BeamIdentifier,
    },
    /// Recovery timer expired; Radio Link Failure declared.
    RadioLinkFailureDeclared,
}

// ---------------------------------------------------------------------------
// 5G NR Beam Failure Recovery Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR Beam Failure Detection & Recovery Engine.
#[derive(Debug)]
pub struct NrBfrEngine {
    pub c_rnti: u16,
    pub config: BeamFailureRecoveryConfig,
    pub state: BfrState,
    pub current_active_tci_beam: BeamIdentifier,

    // Counters & Timers
    pub bfi_counter: u16,
    pub bfd_timer_remaining: u16,
    pub bfr_timer_remaining: u16,

    // Telemetry
    pub total_bfi_events: u64,
    pub total_recovery_attempts: u64,
    pub successful_recoveries: u64,
    pub rlf_events: u64,
}

impl NrBfrEngine {
    /// Create a new BFR Engine instance.
    pub fn new(
        c_rnti: u16,
        initial_tci_beam: BeamIdentifier,
        config: BeamFailureRecoveryConfig,
    ) -> Self {
        Self {
            c_rnti,
            config,
            state: BfrState::Normal,
            current_active_tci_beam: initial_tci_beam,
            bfi_counter: 0,
            bfd_timer_remaining: 0,
            bfr_timer_remaining: 0,
            total_bfi_events: 0,
            total_recovery_attempts: 0,
            successful_recoveries: 0,
            rlf_events: 0,
        }
    }

    /// Process periodic L1 radio link measurements for serving and candidate beams.
    ///
    /// Evaluates BFI condition against Q_out and candidate selection against Q_in.
    pub fn process_l1_measurements(
        &mut self,
        serving_measurements: &[BeamMeasurement],
        candidate_measurements: &[BeamMeasurement],
    ) -> Option<BfrEvent> {
        if matches!(
            self.state,
            BfrState::AwaitingResponse { .. } | BfrState::RadioLinkFailure
        ) {
            // Already awaiting response or in RLF
            return None;
        }

        // 1. Evaluate Serving Beam Quality against Q_out
        // A BFI occurs when all configured serving beams in q_0 fall below Q_out (TS 38.321 §5.17)
        let mut all_serving_failed = !self.config.q0_serving_beams.is_empty();

        for serving_beam in &self.config.q0_serving_beams {
            let quality = serving_measurements
                .iter()
                .find(|m| m.beam == *serving_beam)
                .map(|m| m.rsrp_dbm)
                .unwrap_or(i16::MIN);

            if quality >= self.config.q_out_threshold_dbm {
                all_serving_failed = false;
                break;
            }
        }

        if all_serving_failed {
            self.total_bfi_events += 1;
            self.bfi_counter += 1;
            self.bfd_timer_remaining = self.config.bfd_timer_slots;

            if self.bfi_counter >= self.config.bfi_max_count {
                // Beam Failure Declared!
                self.state = BfrState::EvaluatingFailure;

                // 2. Candidate Beam Selection from set q_1 against Q_in
                let best_candidate = candidate_measurements
                    .iter()
                    .filter(|m| m.rsrp_dbm >= self.config.q_in_threshold_dbm)
                    .max_by_key(|m| m.rsrp_dbm);

                if let Some(candidate) = best_candidate {
                    let cand_cfg = self
                        .config
                        .q1_candidate_beams
                        .iter()
                        .find(|c| c.beam == candidate.beam);

                    let transmission_type = if let Some(cfg) = cand_cfg {
                        if let (Some(preamble), Some(slot)) =
                            (cfg.dedicated_preamble_index, cfg.prach_occasion_slot)
                        {
                            BfrTransmissionType::CfraPreamble {
                                preamble_index: preamble,
                                prach_slot: slot,
                            }
                        } else {
                            // Dedicated preamble not provisioned: format Single-Entry BFR MAC CE
                            let mac_ce = Self::format_single_entry_bfr_mac_ce(0, &candidate.beam);
                            BfrTransmissionType::BfrMacCe {
                                mac_ce_payload: mac_ce,
                            }
                        }
                    } else {
                        BfrTransmissionType::CbraFallback
                    };

                    // Arm Recovery Timer and transition to AwaitingResponse
                    self.bfr_timer_remaining = self.config.bfr_timer_slots;
                    self.total_recovery_attempts += 1;
                    self.state = BfrState::AwaitingResponse {
                        candidate_beam: candidate.beam,
                        transmission_type: transmission_type.clone(),
                    };

                    return Some(BfrEvent::RecoveryRequestDispatched {
                        candidate_beam: candidate.beam,
                        transmission: transmission_type,
                    });
                } else {
                    // No candidate beam met Q_in: CBRA fallback
                    let transmission_type = BfrTransmissionType::CbraFallback;
                    self.bfr_timer_remaining = self.config.bfr_timer_slots;
                    self.total_recovery_attempts += 1;
                    self.state = BfrState::AwaitingResponse {
                        candidate_beam: self.current_active_tci_beam,
                        transmission_type: transmission_type.clone(),
                    };

                    return Some(BfrEvent::RecoveryRequestDispatched {
                        candidate_beam: self.current_active_tci_beam,
                        transmission: transmission_type,
                    });
                }
            } else {
                self.state = BfrState::EvaluatingFailure;
                return Some(BfrEvent::BfiDetected {
                    counter: self.bfi_counter,
                });
            }
        }

        None
    }

    /// Advance time by one slot: decays BFD soak timer or expires BFR recovery timer.
    pub fn step_slot(&mut self) -> Option<BfrEvent> {
        // 1. Decay BFD Timer
        if self.bfd_timer_remaining > 0 {
            self.bfd_timer_remaining -= 1;
            if self.bfd_timer_remaining == 0 {
                // Soak timer expired without new failures: reset BFI counter!
                self.bfi_counter = 0;
                if self.state == BfrState::EvaluatingFailure {
                    self.state = BfrState::Normal;
                }
            }
        }

        // 2. Countdown Recovery Timer while awaiting response
        if let BfrState::AwaitingResponse { .. } = self.state {
            if self.bfr_timer_remaining > 0 {
                self.bfr_timer_remaining -= 1;
                if self.bfr_timer_remaining == 0 {
                    // Recovery Timer Expired -> Trigger Radio Link Failure!
                    self.state = BfrState::RadioLinkFailure;
                    self.rlf_events += 1;
                    return Some(BfrEvent::RadioLinkFailureDeclared);
                }
            }
        }

        None
    }

    /// Notify reception of a PDCCH grant/response addressed to C-RNTI in the recovery search space.
    ///
    /// Confirms beam recovery, stops timers, and switches active TCI beam.
    pub fn notify_pdcch_recovery_response(
        &mut self,
        matched_c_rnti: u16,
        new_tci: BeamIdentifier,
    ) -> Result<BfrEvent, &'static str> {
        if matched_c_rnti != self.c_rnti {
            return Err("PDCCH C-RNTI mismatch");
        }

        match self.state {
            BfrState::AwaitingResponse { .. } => {
                let old_beam = self.current_active_tci_beam;
                self.current_active_tci_beam = new_tci;
                self.bfi_counter = 0;
                self.bfd_timer_remaining = 0;
                self.bfr_timer_remaining = 0;
                self.successful_recoveries += 1;
                self.state = BfrState::Recovered {
                    active_tci_beam: new_tci,
                };

                Ok(BfrEvent::RecoverySuccess {
                    old_beam,
                    new_beam: new_tci,
                })
            }
            _ => Err("Not currently awaiting recovery response"),
        }
    }

    // -----------------------------------------------------------------------
    // BFR MAC CE Encoders and Parsers (3GPP TS 38.321 §6.1.3.23)
    // -----------------------------------------------------------------------

    /// Formats a 3GPP Rel-17 Single Entry BFR MAC CE (2 bytes).
    ///
    /// - Byte 0: [C: 1 bit][Serving Cell ID: 5 bits][AC: 1 bit][SP: 1 bit]
    /// - Byte 1: [Candidate RS ID: 6 bits][Reserved: 2 bits]
    pub fn format_single_entry_bfr_mac_ce(
        serving_cell_idx: u8,
        candidate_beam: &BeamIdentifier,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2);
        let c_bit = 1u8 << 7; // Candidate presence = 1
        let cell_id = (serving_cell_idx & 0x1F) << 2;
        let ac_bit = 0; // Asynchronous / SCell flag
        let sp_bit = 1; // Special Cell flag
        let b0 = c_bit | cell_id | ac_bit | sp_bit;

        // Byte 1: Candidate RS ID (6 bits) | Signal Type (1 bit) | Reserved (1 bit)
        let sig_bit = match candidate_beam.signal_type {
            ReferenceSignalType::Ssb => 0u8,
            ReferenceSignalType::CsiRs => 1u8,
        };
        let b1 = ((candidate_beam.id & 0x3F) << 2) | (sig_bit << 1);

        buf.push(b0);
        buf.push(b1);
        buf
    }

    /// Parses a 3GPP Rel-17 Single Entry BFR MAC CE.
    pub fn parse_single_entry_bfr_mac_ce(
        bytes: &[u8],
    ) -> Result<(u8, BeamIdentifier), &'static str> {
        if bytes.len() < 2 {
            return Err("BFR MAC CE too short");
        }

        let b0 = bytes[0];
        let has_candidate = (b0 & 0x80) != 0;
        let cell_idx = (b0 >> 2) & 0x1F;

        if !has_candidate {
            return Err("Candidate presence bit not set in BFR MAC CE");
        }

        let b1 = bytes[1];
        let candidate_id = (b1 >> 2) & 0x3F;
        let sig_type = if ((b1 >> 1) & 0x01) == 0 {
            ReferenceSignalType::Ssb
        } else {
            ReferenceSignalType::CsiRs
        };

        Ok((
            cell_idx,
            BeamIdentifier {
                signal_type: sig_type,
                id: candidate_id,
            },
        ))
    }

    /// Formats a 3GPP Rel-17 Multiple Entry BFR MAC CE for multi-carrier CA.
    pub fn format_multiple_entry_bfr_mac_ce(
        failed_cells: &[(u8, Option<BeamIdentifier>)],
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // 1-byte Bitmap of failed cells (up to 8 SCells)
        let mut bitmap = 0u8;
        for &(cell_idx, _) in failed_cells {
            if cell_idx < 8 {
                bitmap |= 1 << cell_idx;
            }
        }
        buf.push(bitmap);

        // Candidate entries for each failed cell
        for (_, candidate_opt) in failed_cells {
            if let Some(cand) = candidate_opt {
                let sig_bit = if cand.signal_type == ReferenceSignalType::CsiRs {
                    1u8
                } else {
                    0u8
                };
                let b = 0x80 | ((cand.id & 0x3F) << 1) | sig_bit;
                buf.push(b);
            } else {
                buf.push(0x00); // No candidate found
            }
        }

        buf
    }

    /// Parses a 3GPP Rel-17 Multiple Entry BFR MAC CE.
    pub fn parse_multiple_entry_bfr_mac_ce(
        bytes: &[u8],
    ) -> Result<Vec<(u8, Option<BeamIdentifier>)>, &'static str> {
        if bytes.is_empty() {
            return Err("Empty Multiple Entry BFR MAC CE");
        }

        let bitmap = bytes[0];
        let mut results = Vec::new();
        let mut offset = 1;

        for cell_idx in 0..8 {
            if (bitmap & (1 << cell_idx)) != 0 {
                if offset >= bytes.len() {
                    return Err("Truncated Multiple Entry BFR MAC CE");
                }
                let b = bytes[offset];
                offset += 1;

                let has_cand = (b & 0x80) != 0;
                let cand = if has_cand {
                    let id = (b >> 1) & 0x3F;
                    let sig_type = if (b & 0x01) == 0 {
                        ReferenceSignalType::Ssb
                    } else {
                        ReferenceSignalType::CsiRs
                    };
                    Some(BeamIdentifier {
                        signal_type: sig_type,
                        id,
                    })
                } else {
                    None
                };

                results.push((cell_idx, cand));
            }
        }

        Ok(results)
    }
}
