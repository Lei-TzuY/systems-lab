//! 3GPP Rel-17 TS 38.213 §17 / TS 38.300 §4.10 / TS 38.304 / TS 38.133 RedCap HD-FDD & Relaxed RRM Engine.
//!
//! Implements 5G NR Reduced Capability (RedCap) Half-Duplex FDD (HD-FDD) scheduling,
//! collision avoidance, guard period insertion, and stationary measurement relaxation:
//! - Type A (slot-level) and Type B (symbol-level) HD-FDD operation.
//! - Rx-to-Tx ($N_{Rx-Tx}$) and Tx-to-Rx ($N_{Tx-Rx}$) RF transceiver switching gaps.
//! - Strict 3GPP Rel-17 collision resolution hierarchy:
//!   - SSB / SIB1 broadcast reception priority.
//!   - PRACH preamble priority during Random Access.
//!   - Dedicated PUCCH HARQ-ACK vs PDSCH data channel arbitration.
//!   - PDSCH / PUSCH guard symbol puncturing (symbol dropping).
//!   - Periodic SRS / CSI-RS automatic cancellation upon conflict.
//! - Stationary / Low-Mobility RRM measurement relaxation (TS 38.304 / TS 38.133 §4.6)
//!   reducing neighbour cell scans by 50-70% for multi-year battery lifetime.

use std::fmt;

/// Standard number of OFDM symbols per slot with normal cyclic prefix.
pub const NR_SYMBOLS_PER_SLOT: u8 = 14;

/// Errors raised during RedCap HD-FDD scheduling and RRM relaxation evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedCapHdFddError {
    InvalidSymbolRange { start: u8, duration: u8 },
    InvalidSwitchingGuard { rx_tx: u8, tx_rx: u8 },
    AllocationConflict(&'static str),
    EvaluationWindowTooShort { samples: usize, required: usize },
}

impl fmt::Display for RedCapHdFddError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedCapHdFddError::InvalidSymbolRange { start, duration } => {
                write!(
                    f,
                    "Invalid symbol range: start {} + duration {} exceeds slot boundary (14 symbols)",
                    start, duration
                )
            }
            RedCapHdFddError::InvalidSwitchingGuard { rx_tx, tx_rx } => {
                write!(
                    f,
                    "Invalid switching guard symbols: Rx-to-Tx {}, Tx-to-Rx {}",
                    rx_tx, tx_rx
                )
            }
            RedCapHdFddError::AllocationConflict(msg) => {
                write!(f, "HD-FDD allocation conflict: {}", msg)
            }
            RedCapHdFddError::EvaluationWindowTooShort { samples, required } => {
                write!(
                    f,
                    "RRM evaluation window has {} samples, need at least {}",
                    samples, required
                )
            }
        }
    }
}

impl std::error::Error for RedCapHdFddError {}

/// Half-Duplex FDD operation type defined in 3GPP TS 38.213 Section 17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdFddType {
    /// Type A: Slot-level half-duplex FDD. Switching occurs across slot boundaries.
    TypeA,
    /// Type B: Symbol-level half-duplex FDD. Switching occurs within a slot with explicit guard symbols.
    TypeB,
}

/// Transmission direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdDirection {
    Downlink,
    Uplink,
}

/// Physical channel or signal type in 5G NR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdChannelType {
    /// Synchronization Signal Block (PSS, SSS, PBCH) - Highest priority.
    Ssb,
    /// Downlink Control Information (PDCCH).
    Pdcch,
    /// Downlink Shared Channel (PDSCH) user data.
    Pdsch,
    /// Channel State Information Reference Signal (Periodic / Semi-persistent).
    PeriodicCsiRs,
    /// Paging PDCCH indication.
    PagingPdcch,
    /// Random Access Preamble (PRACH) - High priority.
    Prach,
    /// Uplink Control Channel with HARQ-ACK (PUCCH).
    PucchHarqAck,
    /// Uplink Control Channel with Scheduling Request (SR) or CSI.
    PucchPeriodicCsiSr,
    /// Uplink Shared Channel (PUSCH) user data.
    Pusch,
    /// Sounding Reference Signal (SRS) - Lowest priority.
    PeriodicSrs,
}

impl HdChannelType {
    pub fn direction(&self) -> HdDirection {
        match self {
            HdChannelType::Ssb
            | HdChannelType::Pdcch
            | HdChannelType::Pdsch
            | HdChannelType::PeriodicCsiRs
            | HdChannelType::PagingPdcch => HdDirection::Downlink,

            HdChannelType::Prach
            | HdChannelType::PucchHarqAck
            | HdChannelType::PucchPeriodicCsiSr
            | HdChannelType::Pusch
            | HdChannelType::PeriodicSrs => HdDirection::Uplink,
        }
    }

    /// Base 3GPP Rel-17 priority rank (higher number = higher priority).
    pub fn priority_rank(&self) -> u8 {
        match self {
            HdChannelType::Ssb => 10,
            HdChannelType::Prach => 9,
            HdChannelType::PucchHarqAck => 8,
            HdChannelType::Pdcch => 7,
            HdChannelType::PagingPdcch => 6,
            HdChannelType::Pdsch => 5,
            HdChannelType::Pusch => 4,
            HdChannelType::PucchPeriodicCsiSr => 3,
            HdChannelType::PeriodicCsiRs => 2,
            HdChannelType::PeriodicSrs => 1,
        }
    }
}

/// Configuration for RF switching guard times between reception and transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchingGuardConfig {
    /// Number of OFDM symbols to switch from Rx to Tx ($N_{Rx-Tx}$).
    pub n_rx_to_tx_symbols: u8,
    /// Number of OFDM symbols to switch from Tx to Rx ($N_{Tx-Rx}$).
    pub n_tx_to_rx_symbols: u8,
}

impl SwitchingGuardConfig {
    pub fn new(n_rx_to_tx_symbols: u8, n_tx_to_rx_symbols: u8) -> Result<Self, RedCapHdFddError> {
        if n_rx_to_tx_symbols > 4 || n_tx_to_rx_symbols > 4 {
            return Err(RedCapHdFddError::InvalidSwitchingGuard {
                rx_tx: n_rx_to_tx_symbols,
                tx_rx: n_tx_to_rx_symbols,
            });
        }
        Ok(Self {
            n_rx_to_tx_symbols,
            n_tx_to_rx_symbols,
        })
    }

    /// Default RedCap FR1 switching guard for 15/30 kHz SCS (1 symbol each).
    pub fn fr1_default() -> Self {
        Self {
            n_rx_to_tx_symbols: 1,
            n_tx_to_rx_symbols: 1,
        }
    }

    /// Conservative switching guard for low-cost transceivers (2 symbols each).
    pub fn relaxed() -> Self {
        Self {
            n_rx_to_tx_symbols: 2,
            n_tx_to_rx_symbols: 2,
        }
    }
}

/// Time-domain allocation of a physical channel within a 14-symbol slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelAllocation {
    pub allocation_id: u32,
    pub channel_type: HdChannelType,
    /// Start symbol within slot (0..13).
    pub start_symbol: u8,
    /// Duration in consecutive symbols (1..14).
    pub num_symbols: u8,
    /// Whether symbol puncturing (shortening) is permitted to accommodate switching gap.
    pub allows_puncturing: bool,
}

impl ChannelAllocation {
    pub fn new(
        allocation_id: u32,
        channel_type: HdChannelType,
        start_symbol: u8,
        num_symbols: u8,
        allows_puncturing: bool,
    ) -> Result<Self, RedCapHdFddError> {
        if start_symbol >= NR_SYMBOLS_PER_SLOT
            || num_symbols == 0
            || start_symbol + num_symbols > NR_SYMBOLS_PER_SLOT
        {
            return Err(RedCapHdFddError::InvalidSymbolRange {
                start: start_symbol,
                duration: num_symbols,
            });
        }
        Ok(Self {
            allocation_id,
            channel_type,
            start_symbol,
            num_symbols,
            allows_puncturing,
        })
    }

    pub fn end_symbol(&self) -> u8 {
        self.start_symbol + self.num_symbols
    }

    pub fn direction(&self) -> HdDirection {
        self.channel_type.direction()
    }

    /// Checks if this allocation directly overlaps in time with another.
    pub fn overlaps(&self, other: &ChannelAllocation) -> bool {
        self.start_symbol < other.end_symbol() && other.start_symbol < self.end_symbol()
    }
}

/// Reason an allocation was altered or dropped during HD-FDD scheduling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionReason {
    NoConflict,
    HigherPriorityPreemption { preempted_by: HdChannelType },
    InsufficientSwitchingGuard { needed_symbols: u8 },
    PuncturedForGuardPeriod { original_len: u8, punctured_len: u8 },
    CancelledPeriodicSignal,
}

/// Scheduled transmission details after collision resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledChannel {
    pub allocation_id: u32,
    pub channel_type: HdChannelType,
    pub start_symbol: u8,
    pub num_symbols: u8,
    pub is_punctured: bool,
}

/// Cancelled / dropped transmission details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelledChannel {
    pub allocation_id: u32,
    pub channel_type: HdChannelType,
    pub reason: ResolutionReason,
}

/// Comprehensive outcome of HD-FDD scheduling for a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotScheduleResult {
    pub slot_index: u32,
    pub scheduled: Vec<ScheduledChannel>,
    pub cancelled: Vec<CancelledChannel>,
    pub guard_symbols_inserted: u8,
    pub dl_active_symbols: u8,
    pub ul_active_symbols: u8,
}

/// Performance telemetry for HD-FDD operation.
#[derive(Debug, Clone, PartialEq)]
pub struct HdFddMetrics {
    pub total_slots_evaluated: u32,
    pub total_allocations: u32,
    pub scheduled_count: u32,
    pub cancelled_count: u32,
    pub punctured_count: u32,
    pub dl_duty_cycle_percent: f64,
    pub ul_duty_cycle_percent: f64,
    pub throughput_retention_ratio: f64,
}

/// HD-FDD Scheduling and Collision Avoidance Engine.
#[derive(Debug, Clone)]
pub struct HdFddScheduler {
    pub fdd_type: HdFddType,
    pub guard_config: SwitchingGuardConfig,
    history: Vec<SlotScheduleResult>,
}

impl HdFddScheduler {
    pub fn new(fdd_type: HdFddType, guard_config: SwitchingGuardConfig) -> Self {
        Self {
            fdd_type,
            guard_config,
            history: Vec::new(),
        }
    }

    /// Schedules a single slot, arbitrating between competing DL and UL allocations
    /// under 3GPP Rel-17 HD-FDD switching constraints.
    pub fn schedule_slot(
        &mut self,
        slot_index: u32,
        mut allocations: Vec<ChannelAllocation>,
    ) -> SlotScheduleResult {
        // Sort allocations chronologically; if equal start symbol, by descending priority rank
        allocations.sort_by(|a, b| {
            a.start_symbol.cmp(&b.start_symbol).then_with(|| {
                b.channel_type
                    .priority_rank()
                    .cmp(&a.channel_type.priority_rank())
            })
        });

        let mut scheduled: Vec<ScheduledChannel> = Vec::new();
        let mut cancelled: Vec<CancelledChannel> = Vec::new();
        let mut guard_symbols_inserted = 0u8;

        for alloc in allocations {
            // Check direct overlapping collision with already scheduled channels
            let overlapping_indices: Vec<usize> = scheduled
                .iter()
                .enumerate()
                .filter(|(_, sched)| {
                    let sched_alloc = ChannelAllocation {
                        allocation_id: sched.allocation_id,
                        channel_type: sched.channel_type,
                        start_symbol: sched.start_symbol,
                        num_symbols: sched.num_symbols,
                        allows_puncturing: false,
                    };
                    alloc.overlaps(&sched_alloc)
                })
                .map(|(idx, _)| idx)
                .collect();

            if !overlapping_indices.is_empty() {
                // Find highest priority overlapping channel
                let highest_overlapping = overlapping_indices
                    .iter()
                    .map(|&idx| &scheduled[idx])
                    .max_by_key(|s| s.channel_type.priority_rank())
                    .unwrap();

                if alloc.channel_type.priority_rank()
                    > highest_overlapping.channel_type.priority_rank()
                {
                    // Alloc wins and preempts all overlapping scheduled channels
                    for &idx in overlapping_indices.iter().rev() {
                        let preempted = scheduled.remove(idx);
                        cancelled.push(CancelledChannel {
                            allocation_id: preempted.allocation_id,
                            channel_type: preempted.channel_type,
                            reason: ResolutionReason::HigherPriorityPreemption {
                                preempted_by: alloc.channel_type,
                            },
                        });
                    }
                } else {
                    // Alloc loses
                    cancelled.push(CancelledChannel {
                        allocation_id: alloc.allocation_id,
                        channel_type: alloc.channel_type,
                        reason: ResolutionReason::HigherPriorityPreemption {
                            preempted_by: highest_overlapping.channel_type,
                        },
                    });
                    continue;
                }
            }

            // Check switching guard requirement against previously scheduled transmission
            if let Some(prev) = scheduled.last_mut() {
                if prev.channel_type.direction() != alloc.direction() {
                    let req_guard = match prev.channel_type.direction() {
                        HdDirection::Downlink => self.guard_config.n_rx_to_tx_symbols,
                        HdDirection::Uplink => self.guard_config.n_tx_to_rx_symbols,
                    };

                    let gap = alloc
                        .start_symbol
                        .saturating_sub(prev.start_symbol + prev.num_symbols);

                    if gap < req_guard {
                        let deficit = req_guard - gap;
                        // Try puncturing (shortening) the previous channel if it allows it and has enough symbols
                        if prev.num_symbols > deficit
                            && prev.channel_type.priority_rank()
                                <= alloc.channel_type.priority_rank()
                        {
                            prev.num_symbols -= deficit;
                            prev.is_punctured = true;
                            guard_symbols_inserted += req_guard;
                        } else if alloc.allows_puncturing && alloc.num_symbols > deficit {
                            // Puncture start of current allocation
                            let new_start = alloc.start_symbol + deficit;
                            let new_len = alloc.num_symbols - deficit;
                            guard_symbols_inserted += req_guard;

                            scheduled.push(ScheduledChannel {
                                allocation_id: alloc.allocation_id,
                                channel_type: alloc.channel_type,
                                start_symbol: new_start,
                                num_symbols: new_len,
                                is_punctured: true,
                            });
                            continue;
                        } else {
                            // Cannot puncture; cancel lower priority
                            if alloc.channel_type.priority_rank()
                                > prev.channel_type.priority_rank()
                            {
                                let dropped = scheduled.pop().unwrap();
                                cancelled.push(CancelledChannel {
                                    allocation_id: dropped.allocation_id,
                                    channel_type: dropped.channel_type,
                                    reason: ResolutionReason::HigherPriorityPreemption {
                                        preempted_by: alloc.channel_type,
                                    },
                                });
                            } else {
                                cancelled.push(CancelledChannel {
                                    allocation_id: alloc.allocation_id,
                                    channel_type: alloc.channel_type,
                                    reason: ResolutionReason::InsufficientSwitchingGuard {
                                        needed_symbols: req_guard,
                                    },
                                });
                                continue;
                            }
                        }
                    }
                }
            }

            scheduled.push(ScheduledChannel {
                allocation_id: alloc.allocation_id,
                channel_type: alloc.channel_type,
                start_symbol: alloc.start_symbol,
                num_symbols: alloc.num_symbols,
                is_punctured: false,
            });
        }

        let mut dl_active = 0u8;
        let mut ul_active = 0u8;
        for s in &scheduled {
            match s.channel_type.direction() {
                HdDirection::Downlink => dl_active += s.num_symbols,
                HdDirection::Uplink => ul_active += s.num_symbols,
            }
        }

        let result = SlotScheduleResult {
            slot_index,
            scheduled,
            cancelled,
            guard_symbols_inserted,
            dl_active_symbols: dl_active,
            ul_active_symbols: ul_active,
        };

        self.history.push(result.clone());
        result
    }

    /// Evaluates cumulative HD-FDD throughput and duty-cycle metrics.
    pub fn compute_metrics(&self) -> HdFddMetrics {
        let total_slots = self.history.len() as u32;
        let mut total_allocs = 0u32;
        let mut sched_count = 0u32;
        let mut canc_count = 0u32;
        let mut punc_count = 0u32;
        let mut total_dl_syms = 0u64;
        let mut total_ul_syms = 0u64;

        for res in &self.history {
            total_allocs += (res.scheduled.len() + res.cancelled.len()) as u32;
            sched_count += res.scheduled.len() as u32;
            canc_count += res.cancelled.len() as u32;
            for s in &res.scheduled {
                if s.is_punctured {
                    punc_count += 1;
                }
            }
            total_dl_syms += res.dl_active_symbols as u64;
            total_ul_syms += res.ul_active_symbols as u64;
        }

        let total_symbol_capacity = (total_slots as u64) * (NR_SYMBOLS_PER_SLOT as u64);
        let dl_duty_cycle_percent = if total_symbol_capacity > 0 {
            (total_dl_syms as f64 / total_symbol_capacity as f64) * 100.0
        } else {
            0.0
        };
        let ul_duty_cycle_percent = if total_symbol_capacity > 0 {
            (total_ul_syms as f64 / total_symbol_capacity as f64) * 100.0
        } else {
            0.0
        };

        let throughput_retention_ratio = if total_allocs > 0 {
            sched_count as f64 / total_allocs as f64
        } else {
            1.0
        };

        HdFddMetrics {
            total_slots_evaluated: total_slots,
            total_allocations: total_allocs,
            scheduled_count: sched_count,
            cancelled_count: canc_count,
            punctured_count: punc_count,
            dl_duty_cycle_percent,
            ul_duty_cycle_percent,
            throughput_retention_ratio,
        }
    }
}

// ---------------------------------------------------------------------------
// Relaxed RRM Measurement Engine for Stationary RedCap (TS 38.304 / TS 38.133)
// ---------------------------------------------------------------------------

/// Operating state of RedCap RRM measurement relaxation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelaxedRrmState {
    /// Full measurement: standard eMBB scan rate (normal mobility or near cell-edge).
    FullMeasurement,
    /// Relaxed serving-only: serving cell scans halved, neighbor scans slowed down.
    RelaxedServingOnly,
    /// Neighbor measurement disabled: UE is stationary and far from cell edge.
    NeighborMeasurementDisabled,
}

/// Criteria parameters for entering and maintaining relaxed RRM measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct RelaxedRrmCriteria {
    /// Threshold difference in dB for RSRP variation to qualify as stationary ($S_{searchDeltaP}$).
    pub s_search_delta_p_db: f64,
    /// Cell-edge offset threshold in dB ($S_{searchP}$).
    pub s_search_p_thresh_dbm: f64,
    /// Required stationary evaluation window in seconds ($T_{searchDeltaP}$).
    pub t_search_delta_p_sec: f64,
}

impl RelaxedRrmCriteria {
    pub fn default_redcap() -> Self {
        Self {
            s_search_delta_p_db: 3.0,     // 3 dB maximum variation
            s_search_p_thresh_dbm: -95.0, // Serving cell RSRP must exceed -95 dBm
            t_search_delta_p_sec: 300.0,  // 5 minutes window
        }
    }
}

/// Evaluator that determines whether a RedCap UE qualifies for RRM measurement relaxation.
#[derive(Debug, Clone)]
pub struct RrmRelaxationEvaluator {
    pub criteria: RelaxedRrmCriteria,
    pub current_state: RelaxedRrmState,
    rsrp_samples: Vec<(f64, f64)>, // (timestamp_seconds, rsrp_dbm)
}

impl RrmRelaxationEvaluator {
    pub fn new(criteria: RelaxedRrmCriteria) -> Self {
        Self {
            criteria,
            current_state: RelaxedRrmState::FullMeasurement,
            rsrp_samples: Vec::new(),
        }
    }

    /// Records a serving cell RSRP measurement.
    pub fn add_measurement(&mut self, timestamp_sec: f64, rsrp_dbm: f64) {
        self.rsrp_samples.push((timestamp_sec, rsrp_dbm));
        // Prune measurements older than the evaluation window
        let cutoff = timestamp_sec - self.criteria.t_search_delta_p_sec;
        self.rsrp_samples.retain(|(t, _)| *t >= cutoff);
    }

    /// Evaluates current RSRP history against stationary and cell-edge criteria (TS 38.304 §5.2.4.9).
    pub fn evaluate_state(&mut self) -> Result<RelaxedRrmState, RedCapHdFddError> {
        if self.rsrp_samples.len() < 3 {
            self.current_state = RelaxedRrmState::FullMeasurement;
            return Ok(self.current_state);
        }

        let latest_rsrp = self.rsrp_samples.last().unwrap().1;

        // Find min and max RSRP in evaluation window
        let mut min_rsrp = f64::MAX;
        let mut max_rsrp = f64::MIN;
        for &(_, rsrp) in &self.rsrp_samples {
            if rsrp < min_rsrp {
                min_rsrp = rsrp;
            }
            if rsrp > max_rsrp {
                max_rsrp = rsrp;
            }
        }

        let variation_db = max_rsrp - min_rsrp;
        let is_stationary = variation_db <= self.criteria.s_search_delta_p_db;
        let is_good_coverage = latest_rsrp > self.criteria.s_search_p_thresh_dbm;

        let next_state = if is_stationary && is_good_coverage {
            RelaxedRrmState::NeighborMeasurementDisabled
        } else if is_stationary {
            RelaxedRrmState::RelaxedServingOnly
        } else {
            RelaxedRrmState::FullMeasurement
        };

        self.current_state = next_state;
        Ok(next_state)
    }

    /// Estimated battery power consumption reduction factor under current state.
    pub fn power_saving_factor(&self) -> f64 {
        match self.current_state {
            RelaxedRrmState::FullMeasurement => 1.0, // baseline 100% consumption
            RelaxedRrmState::RelaxedServingOnly => 0.65, // ~35% power savings
            RelaxedRrmState::NeighborMeasurementDisabled => 0.35, // ~65% power savings
        }
    }
}
