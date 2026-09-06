//! 3GPP TS 38.304 Rel-17 5G NR Cell Selection & Reselection Engine.
//!
//! Implements 5G NR User Equipment (UE) procedures in `RRC_IDLE` and `RRC_INACTIVE` states:
//! - S-Criterion cell selection: S_rxlev and S_qual evaluation with power compensation (TS 38.304 §5.2.3.2).
//! - Suitable cell vs Acceptable cell classification (normal vs emergency camp, TS 38.304 §4.3).
//! - R-Criterion ranking for intra-frequency and equal-priority neighbor cells (TS 38.304 §5.2.4.6).
//! - Absolute priority-based inter-frequency and inter-RAT cell reselection (layers 0..7, TS 38.304 §5.2.4.5).
//! - Speed-dependent Mobility State Estimation (MSE) with Q_hyst and T_reselection scaling (TS 38.304 §5.2.4.3).
//! - Cell barring, T_barred timers, and blacklisted cell filtering (TS 38.304 §5.3.1).

use std::collections::HashMap;

/// Public Land Mobile Network (PLMN) Identity (MCC + MNC).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlmnIdentity {
    pub mcc: String,
    pub mnc: String,
}

impl PlmnIdentity {
    pub fn new(mcc: &str, mnc: &str) -> Self {
        Self {
            mcc: mcc.to_string(),
            mnc: mnc.to_string(),
        }
    }
}

/// 5G NR Cell Identity with Physical Cell ID (PCI) and carrier frequency (ARFCN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NrCellIdentity {
    /// 36-bit NR Cell Identity (NCI).
    pub nci: u64,
    /// Physical Cell ID (PCI: 0..1007).
    pub pci: u16,
    /// Absolute Radio Frequency Channel Number.
    pub arfcn: u32,
}

impl NrCellIdentity {
    pub fn new(nci: u64, pci: u16, arfcn: u32) -> Self {
        Self { nci, pci, arfcn }
    }
}

/// Broadcast Cell Access Information (from SIB1 / MIB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellAccessInfo {
    pub plmn_list: Vec<PlmnIdentity>,
    pub tac: u32,
    pub is_cell_barred: bool,
    pub intra_freq_reselection_allowed: bool,
    pub is_reserved_for_operator: bool,
}

/// S-Criterion Evaluation Parameters broadcast in SIB1 (TS 38.304 §5.2.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCriterionParams {
    /// Minimum required RX level in the cell (Q_rxlevmin in dBm, e.g. -120 dBm).
    pub q_rx_lev_min: i16,
    /// Offset to Q_rxlevmin (Q_rxlevminoffset in dB, default 0 dB).
    pub q_rx_lev_min_offset: i16,
    /// Minimum required quality level in the cell (Q_qualmin in dB, e.g. -20 dB).
    pub q_qual_min: i16,
    /// Offset to Q_qualmin (Q_qualminoffset in dB, default 0 dB).
    pub q_qual_min_offset: i16,
    /// Maximum allowed UE transmit power on carrier (P_EMAX in dBm, e.g. 23 dBm).
    pub p_emax: i16,
    /// UE maximum RF output power class (P_PowerClass in dBm, e.g. 23 dBm).
    pub ue_power_class: i16,
}

impl Default for SCriterionParams {
    fn default() -> Self {
        Self {
            q_rx_lev_min: -120,
            q_rx_lev_min_offset: 0,
            q_qual_min: -20,
            q_qual_min_offset: 0,
            p_emax: 23,
            ue_power_class: 23,
        }
    }
}

/// Result of S-Criterion Evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCriterionResult {
    /// S_rxlev = Q_rxlevmeas - (Q_rxlevmin + Q_rxlevminoffset) - P_compensation - Q_offsettemp
    pub s_rxlev: i16,
    /// S_qual = Q_qualmeas - (Q_qualmin + Q_qualminoffset) - Q_offsettemp
    pub s_qual: i16,
    pub is_rxlev_satisfied: bool,
    pub is_qual_satisfied: bool,
    pub is_satisfied: bool,
}

impl SCriterionParams {
    /// Evaluates S-Criterion:
    /// S_rxlev > 0 and S_qual > 0
    /// where P_compensation = max(p_emax - ue_power_class, 0).
    pub fn evaluate(
        &self,
        q_rx_lev_meas: i16,
        q_qual_meas: i16,
        q_offset_temp: i16,
    ) -> SCriterionResult {
        let p_compensation = (self.p_emax.saturating_sub(self.ue_power_class)).max(0);
        let s_rxlev = q_rx_lev_meas
            .saturating_sub(self.q_rx_lev_min.saturating_add(self.q_rx_lev_min_offset))
            .saturating_sub(p_compensation)
            .saturating_sub(q_offset_temp);

        let s_qual = q_qual_meas
            .saturating_sub(self.q_qual_min.saturating_add(self.q_qual_min_offset))
            .saturating_sub(q_offset_temp);

        let is_rxlev_satisfied = s_rxlev > 0;
        let is_qual_satisfied = s_qual > 0;
        let is_satisfied = is_rxlev_satisfied && is_qual_satisfied;

        SCriterionResult {
            s_rxlev,
            s_qual,
            is_rxlev_satisfied,
            is_qual_satisfied,
            is_satisfied,
        }
    }
}

/// Cell suitability outcome for camping (TS 38.304 §4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellSuitability {
    /// Suitable cell: UE can camp for normal service (monitor paging, initiate calls).
    Suitable,
    /// Acceptable cell: S-criterion met, but PLMN or TAC fails. Restricted to Emergency Calls only.
    Acceptable(AcceptableReason),
    /// Unsuitable cell: Cannot camp (S-criterion failed, cell barred, or operator reserved).
    Unsuitable(UnsuitableReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptableReason {
    PlmnNotAllowed,
    ForbiddenTrackingArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsuitableReason {
    SCriterionFailed,
    CellBarred,
    ReservedForOperator,
}

/// Carrier frequency priority layer configuration broadcast in SIB4 / SIB5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrequencyLayerConfig {
    pub arfcn: u32,
    /// Absolute priority: 0 (lowest) to 7 (highest).
    pub priority: u8,
    /// Thresh_X,HighP in dB (for higher priority reselection).
    pub thresh_x_high_p: i16,
    /// Thresh_X,LowP in dB (for lower priority reselection).
    pub thresh_x_low_p: i16,
    /// Cell reselection timer T_reselection in seconds.
    pub t_reselection_s: u32,
    /// Frequency-specific offset Q_offset_frequency in dB.
    pub q_offset_freq: i16,
}

/// Serving cell reselection thresholds broadcast in SIB2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServingCellConfig {
    /// S_IntraSearchP in dB: threshold for triggering intra-frequency neighbor measurements.
    pub s_intra_search_p: i16,
    /// S_nonIntraSearchP in dB: threshold for triggering non-intra-frequency neighbor measurements.
    pub s_non_intra_search_p: i16,
    /// Thresh_Serving,LowP in dB: threshold below which lower priority layers are considered.
    pub thresh_serving_low_p: i16,
    /// Reselection hysteresis Q_hyst in dB.
    pub q_hyst: i16,
}

impl Default for ServingCellConfig {
    fn default() -> Self {
        Self {
            s_intra_search_p: 12,
            s_non_intra_search_p: 10,
            thresh_serving_low_p: 6,
            q_hyst: 4,
        }
    }
}

/// Speed-dependent Mobility State Estimation (MSE) per TS 38.304 §5.2.4.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilityState {
    Normal,
    Medium,
    High,
}

/// Configuration for Mobility State Estimation (MSE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MseConfig {
    /// Evaluation time window T_CRmax in seconds.
    pub t_crmax_s: u32,
    /// Number of cell reselections to enter Medium mobility (N_CR_M).
    pub n_cr_m: u32,
    /// Number of cell reselections to enter High mobility (N_CR_H).
    pub n_cr_h: u32,
    /// Q_hyst scaling in dB (e.g. -2 dB for Medium, -4 dB for High).
    pub q_hyst_scaling_medium_db: i16,
    pub q_hyst_scaling_high_db: i16,
    /// T_reselection scaling factor (e.g. 75 for 0.75x in Medium, 50 for 0.50x in High).
    pub t_reselection_scaling_medium_percent: u32,
    pub t_reselection_scaling_high_percent: u32,
}

impl Default for MseConfig {
    fn default() -> Self {
        Self {
            t_crmax_s: 60,
            n_cr_m: 4,
            n_cr_h: 8,
            q_hyst_scaling_medium_db: 2,
            q_hyst_scaling_high_db: 4,
            t_reselection_scaling_medium_percent: 75,
            t_reselection_scaling_high_percent: 50,
        }
    }
}

/// Radio measurement of a candidate serving or neighbor cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMeasurement {
    pub cell: NrCellIdentity,
    /// Measured RSRP in dBm (Q_rxlevmeas, e.g. -85 dBm).
    pub q_rx_lev_meas: i16,
    /// Measured RSRQ in dB (Q_qualmeas, e.g. -11 dB).
    pub q_qual_meas: i16,
    /// Cell-specific offset Q_offset_cell in dB (default 0 dB).
    pub q_offset_cell: i16,
}

/// Cause for deciding to reselect to a target cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReselectionCause {
    HighPriorityInterFreq,
    IntraFreqRanked,
    EqualPriorityRanked,
    LowPriorityInterFreq,
}

/// Cell Reselection Decision produced by the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellReselectionDecision {
    pub target_cell: NrCellIdentity,
    pub cause: ReselectionCause,
    pub target_r_rank: Option<i16>,
    pub target_s_rxlev: i16,
}

/// 3GPP Rel-17 5G NR Cell Selection & Reselection Engine.
#[derive(Debug)]
pub struct NrCellReselectionEngine {
    pub ue_id: String,
    pub registered_plmns: Vec<PlmnIdentity>,
    pub forbidden_tacs: Vec<u32>,
    pub current_serving_cell: Option<NrCellIdentity>,
    pub serving_config: ServingCellConfig,
    pub freq_layers: HashMap<u32, FrequencyLayerConfig>,
    pub mobility_state: MobilityState,
    pub mse_config: MseConfig,
    /// Reselection timestamps in epoch seconds for MSE sliding window.
    pub reselection_history_s: Vec<u64>,
    /// Barred cells table: NCI -> barred_until_epoch_s.
    pub barred_cells: HashMap<u64, u64>,
    /// Permanent or temporary blacklisted cell NCIs.
    pub blacklisted_cells: Vec<u64>,
    /// Candidate cell that is currently timing out T_reselection: (Cell, start_time_s).
    pub reselection_timer_start: Option<(NrCellIdentity, ReselectionCause, u64)>,
}

impl NrCellReselectionEngine {
    /// Create a new 5G NR Cell Selection & Reselection Engine.
    pub fn new(
        ue_id: &str,
        registered_plmns: Vec<PlmnIdentity>,
        forbidden_tacs: Vec<u32>,
        serving_config: ServingCellConfig,
    ) -> Self {
        Self {
            ue_id: ue_id.to_string(),
            registered_plmns,
            forbidden_tacs,
            current_serving_cell: None,
            serving_config,
            freq_layers: HashMap::new(),
            mobility_state: MobilityState::Normal,
            mse_config: MseConfig::default(),
            reselection_history_s: Vec::new(),
            barred_cells: HashMap::new(),
            blacklisted_cells: Vec::new(),
            reselection_timer_start: None,
        }
    }

    /// Add or update a frequency priority layer.
    pub fn configure_frequency_layer(&mut self, layer: FrequencyLayerConfig) {
        self.freq_layers.insert(layer.arfcn, layer);
    }

    /// Mark a cell as barred for a duration (TS 38.304 §5.3.1, up to 300 s).
    pub fn bar_cell(&mut self, nci: u64, duration_s: u64, current_epoch_s: u64) {
        self.barred_cells
            .insert(nci, current_epoch_s.saturating_add(duration_s));
    }

    /// Check if a cell is currently barred.
    pub fn is_cell_barred(&self, nci: u64, current_epoch_s: u64) -> bool {
        if let Some(&barred_until) = self.barred_cells.get(&nci) {
            current_epoch_s < barred_until
        } else {
            false
        }
    }

    /// Add a cell to the reselection blacklist.
    pub fn blacklist_cell(&mut self, nci: u64) {
        if !self.blacklisted_cells.contains(&nci) {
            self.blacklisted_cells.push(nci);
        }
    }

    /// Remove a cell from the blacklist.
    pub fn remove_blacklisted_cell(&mut self, nci: u64) {
        self.blacklisted_cells.retain(|&c| c != nci);
    }

    /// Classify cell suitability for camping (TS 38.304 §4.3).
    pub fn check_cell_suitability(
        &self,
        cell: &NrCellIdentity,
        access: &CellAccessInfo,
        s_params: &SCriterionParams,
        meas: &CellMeasurement,
        current_epoch_s: u64,
    ) -> CellSuitability {
        // 1. Check barring
        if access.is_cell_barred || self.is_cell_barred(cell.nci, current_epoch_s) {
            return CellSuitability::Unsuitable(UnsuitableReason::CellBarred);
        }

        // 2. Check operator reservation
        if access.is_reserved_for_operator {
            return CellSuitability::Unsuitable(UnsuitableReason::ReservedForOperator);
        }

        // 3. Evaluate S-Criterion
        let s_result = s_params.evaluate(meas.q_rx_lev_meas, meas.q_qual_meas, 0);
        if !s_result.is_satisfied {
            return CellSuitability::Unsuitable(UnsuitableReason::SCriterionFailed);
        }

        // 4. Check PLMN matching
        let plmn_match = access
            .plmn_list
            .iter()
            .any(|p| self.registered_plmns.contains(p));
        if !plmn_match {
            return CellSuitability::Acceptable(AcceptableReason::PlmnNotAllowed);
        }

        // 5. Check forbidden tracking area
        if self.forbidden_tacs.contains(&access.tac) {
            return CellSuitability::Acceptable(AcceptableReason::ForbiddenTrackingArea);
        }

        CellSuitability::Suitable
    }

    /// Record a successful cell reselection and update Mobility State Estimation (MSE).
    pub fn record_reselection(&mut self, new_cell: NrCellIdentity, current_epoch_s: u64) {
        self.current_serving_cell = Some(new_cell);
        self.reselection_timer_start = None;
        self.reselection_history_s.push(current_epoch_s);

        // Prune entries outside sliding window T_CRmax
        let cutoff = current_epoch_s.saturating_sub(self.mse_config.t_crmax_s as u64);
        self.reselection_history_s.retain(|&t| t >= cutoff);

        let count = self.reselection_history_s.len() as u32;
        if count >= self.mse_config.n_cr_h {
            self.mobility_state = MobilityState::High;
        } else if count >= self.mse_config.n_cr_m {
            self.mobility_state = MobilityState::Medium;
        } else {
            self.mobility_state = MobilityState::Normal;
        }
    }

    /// Effective Q_hyst with speed scaling applied.
    pub fn effective_q_hyst(&self) -> i16 {
        let base = self.serving_config.q_hyst;
        match self.mobility_state {
            MobilityState::Normal => base,
            MobilityState::Medium => base.saturating_sub(self.mse_config.q_hyst_scaling_medium_db),
            MobilityState::High => base.saturating_sub(self.mse_config.q_hyst_scaling_high_db),
        }
    }

    /// Effective T_reselection with speed scaling applied.
    pub fn effective_t_reselection_s(&self, base_t_reselection_s: u32) -> u32 {
        let scaled = match self.mobility_state {
            MobilityState::Normal => base_t_reselection_s,
            MobilityState::Medium => {
                (base_t_reselection_s * self.mse_config.t_reselection_scaling_medium_percent) / 100
            }
            MobilityState::High => {
                (base_t_reselection_s * self.mse_config.t_reselection_scaling_high_percent) / 100
            }
        };
        scaled.max(1) // at least 1 second
    }

    /// Evaluate cell reselection across serving and neighbor measurements.
    ///
    /// Evaluates in 3GPP specification order (TS 38.304 §5.2.4.5):
    /// 1. High-priority inter-frequency candidate meeting Thresh_X,HighP.
    /// 2. Intra-frequency / equal-priority candidate ranked higher via R-criterion.
    /// 3. Low-priority inter-frequency candidate when serving drops below Thresh_Serving,LowP.
    pub fn evaluate_reselection(
        &mut self,
        serving_meas: &CellMeasurement,
        serving_s_params: &SCriterionParams,
        neighbors: &[CellMeasurement],
        neighbor_s_params: &HashMap<u64, SCriterionParams>,
        current_epoch_s: u64,
    ) -> Option<CellReselectionDecision> {
        let serving_s =
            serving_s_params.evaluate(serving_meas.q_rx_lev_meas, serving_meas.q_qual_meas, 0);

        let serving_layer = match self.freq_layers.get(&serving_meas.cell.arfcn) {
            Some(l) => *l,
            None => FrequencyLayerConfig {
                arfcn: serving_meas.cell.arfcn,
                priority: 4,
                thresh_x_high_p: 10,
                thresh_x_low_p: 8,
                t_reselection_s: 2,
                q_offset_freq: 0,
            },
        };

        // Filter valid candidates (not blacklisted, not barred)
        let valid_neighbors: Vec<&CellMeasurement> = neighbors
            .iter()
            .filter(|n| {
                !self.blacklisted_cells.contains(&n.cell.nci)
                    && !self.is_cell_barred(n.cell.nci, current_epoch_s)
            })
            .collect();

        // -------------------------------------------------------------------
        // Rule 1: High Priority Layers (priority > serving_layer.priority)
        // -------------------------------------------------------------------
        for cand in &valid_neighbors {
            if let Some(cand_layer) = self.freq_layers.get(&cand.cell.arfcn) {
                if cand_layer.priority > serving_layer.priority {
                    let default_sparams = SCriterionParams::default();
                    let sparams = neighbor_s_params
                        .get(&cand.cell.nci)
                        .unwrap_or(&default_sparams);
                    let cand_s = sparams.evaluate(cand.q_rx_lev_meas, cand.q_qual_meas, 0);

                    // High priority reselection condition: S_rxlev > Thresh_X,HighP
                    if cand_s.s_rxlev > cand_layer.thresh_x_high_p {
                        let req_time = self.effective_t_reselection_s(cand_layer.t_reselection_s);
                        if self.check_reselection_timer(
                            cand.cell,
                            ReselectionCause::HighPriorityInterFreq,
                            req_time,
                            current_epoch_s,
                        ) {
                            return Some(CellReselectionDecision {
                                target_cell: cand.cell,
                                cause: ReselectionCause::HighPriorityInterFreq,
                                target_r_rank: None,
                                target_s_rxlev: cand_s.s_rxlev,
                            });
                        }
                        return None;
                    }
                }
            }
        }

        // -------------------------------------------------------------------
        // Rule 2: Intra-Frequency / Equal Priority Layers (R-Criterion Ranking)
        // -------------------------------------------------------------------
        // Check if intra-frequency search is triggered: S_rxlev <= S_IntraSearchP
        let should_search_intra = serving_s.s_rxlev <= self.serving_config.s_intra_search_p;

        if should_search_intra {
            // Serving cell R-rank: R_s = Q_meas,s + Q_hyst - Q_offsettemp
            let r_s = serving_meas
                .q_rx_lev_meas
                .saturating_add(self.effective_q_hyst());

            let mut best_equal_cand = None;
            let mut best_r_n = r_s; // Must be strictly better than serving cell

            for cand in &valid_neighbors {
                let is_intra = cand.cell.arfcn == serving_meas.cell.arfcn;
                let is_equal_priority = match self.freq_layers.get(&cand.cell.arfcn) {
                    Some(l) => l.priority == serving_layer.priority,
                    None => is_intra,
                };

                if is_intra || is_equal_priority {
                    let q_offset_freq = self
                        .freq_layers
                        .get(&cand.cell.arfcn)
                        .map(|l| l.q_offset_freq)
                        .unwrap_or(0);

                    // Neighbor R-rank: R_n = Q_meas,n - Q_offset_frequency - Q_offset_cell
                    let r_n = cand
                        .q_rx_lev_meas
                        .saturating_sub(q_offset_freq)
                        .saturating_sub(cand.q_offset_cell);

                    if r_n > best_r_n {
                        best_r_n = r_n;
                        best_equal_cand = Some(*cand);
                    }
                }
            }

            if let Some(cand) = best_equal_cand {
                let t_res = serving_layer.t_reselection_s;
                let req_time = self.effective_t_reselection_s(t_res);
                let cause = if cand.cell.arfcn == serving_meas.cell.arfcn {
                    ReselectionCause::IntraFreqRanked
                } else {
                    ReselectionCause::EqualPriorityRanked
                };

                if self.check_reselection_timer(cand.cell, cause, req_time, current_epoch_s) {
                    let default_sparams = SCriterionParams::default();
                    let sparams = neighbor_s_params
                        .get(&cand.cell.nci)
                        .unwrap_or(&default_sparams);
                    let cand_s = sparams.evaluate(cand.q_rx_lev_meas, cand.q_qual_meas, 0);

                    return Some(CellReselectionDecision {
                        target_cell: cand.cell,
                        cause,
                        target_r_rank: Some(best_r_n),
                        target_s_rxlev: cand_s.s_rxlev,
                    });
                }
                return None;
            }
        }

        // -------------------------------------------------------------------
        // Rule 3: Low Priority Layers (priority < serving_layer.priority)
        // -------------------------------------------------------------------
        // Only considered if serving cell drops below Thresh_Serving,LowP!
        if serving_s.s_rxlev < self.serving_config.thresh_serving_low_p {
            for cand in &valid_neighbors {
                if let Some(cand_layer) = self.freq_layers.get(&cand.cell.arfcn) {
                    if cand_layer.priority < serving_layer.priority {
                        let default_sparams = SCriterionParams::default();
                        let sparams = neighbor_s_params
                            .get(&cand.cell.nci)
                            .unwrap_or(&default_sparams);
                        let cand_s = sparams.evaluate(cand.q_rx_lev_meas, cand.q_qual_meas, 0);

                        // Low priority reselection condition: Candidate S_rxlev > Thresh_X,LowP
                        if cand_s.s_rxlev > cand_layer.thresh_x_low_p {
                            let req_time =
                                self.effective_t_reselection_s(cand_layer.t_reselection_s);
                            if self.check_reselection_timer(
                                cand.cell,
                                ReselectionCause::LowPriorityInterFreq,
                                req_time,
                                current_epoch_s,
                            ) {
                                return Some(CellReselectionDecision {
                                    target_cell: cand.cell,
                                    cause: ReselectionCause::LowPriorityInterFreq,
                                    target_r_rank: None,
                                    target_s_rxlev: cand_s.s_rxlev,
                                });
                            }
                            return None;
                        }
                    }
                }
            }
        }

        // No candidate fulfilled criteria -> reset timer
        self.reselection_timer_start = None;
        None
    }

    /// Check if target cell has continuously satisfied criteria for effective T_reselection duration.
    fn check_reselection_timer(
        &mut self,
        target_cell: NrCellIdentity,
        cause: ReselectionCause,
        required_duration_s: u32,
        current_epoch_s: u64,
    ) -> bool {
        match self.reselection_timer_start {
            Some((current_cand, current_cause, start_s)) => {
                if current_cand == target_cell && current_cause == cause {
                    if current_epoch_s.saturating_sub(start_s) >= required_duration_s as u64 {
                        true
                    } else {
                        false
                    }
                } else {
                    // Different candidate became better: reset timer for new candidate
                    self.reselection_timer_start = Some((target_cell, cause, current_epoch_s));
                    false
                }
            }
            None => {
                // Timer started
                self.reselection_timer_start = Some((target_cell, cause, current_epoch_s));
                false
            }
        }
    }
}
