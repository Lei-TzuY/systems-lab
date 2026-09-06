//! 3GPP TS 38.213 §11 / TS 38.300 §6.4 Rel-17 5G NR-U (Unlicensed Spectrum) & Listen-Before-Talk (LBT) Engine.
//!
//! Provides the complete physical/MAC layer shared spectrum channel access engine for
//! 5G Standalone and Dual Connectivity operation in 5 GHz (Band n46) and 6 GHz
//! (Bands n96, n102, UNII-5..8) unlicensed bands.
//!
//! Standard features implemented:
//! - Regulatory Maximum Energy Detection (ED) threshold adaptation based on bandwidth and TX EIRP.
//! - Channel Access Priority Classes (CAPC 1..4) per TS 37.213 / TS 38.213 Table 4.1.1-1.
//! - Type 1 Channel Access (Category 4 LBT: Defer period sensing, random backoff, freeze & resume).
//! - Type 2A/2B/2C Channel Access (Category 2 & Category 1 LBT: 25us / 16us / immediate access).
//! - Contention Window Adaptation (CWA): dynamic doubling when HARQ NACK/DTX ratio >= 80%, reset on success.
//! - Maximum Channel Occupancy Time (MCOT) enforcement and gNodeB-to-UE COT Sharing.
//! - Fractional Slot Channel Reservation Signal (CRS) / Extension Preamble generation.
//! - Wideband Carrier Sensing (40/80/160 MHz) and Dynamic Bandwidth Puncturing.

use std::collections::HashMap;

/// 3GPP Channel Access Priority Classes (CAPC) per TS 37.213 / TS 38.213 Table 4.1.1-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelAccessPriorityClass {
    /// CAPC 1: Conversational voice, mission critical control signaling.
    VoiceSignaling = 1,
    /// CAPC 2: Interactive gaming, low-latency video.
    InteractiveVideo = 2,
    /// CAPC 3: Buffered video, standard web / best-effort traffic.
    BestEffort = 3,
    /// CAPC 4: Background file download / bulk transfer.
    Background = 4,
}

impl ChannelAccessPriorityClass {
    /// Number of consecutive idle CCA slots required in the defer period (m_p).
    pub fn defer_slots_mp(&self) -> u32 {
        match self {
            Self::VoiceSignaling => 1,
            Self::InteractiveVideo => 1,
            Self::BestEffort => 3,
            Self::Background => 7,
        }
    }

    /// Total defer duration T_d = 16 us + m_p * 9 us.
    pub fn defer_duration_us(&self) -> u32 {
        16 + self.defer_slots_mp() * 9
    }

    /// Minimum Contention Window (CW_min).
    pub fn cw_min(&self) -> u32 {
        match self {
            Self::VoiceSignaling => 3,
            Self::InteractiveVideo => 7,
            Self::BestEffort => 15,
            Self::Background => 15,
        }
    }

    /// Maximum Contention Window (CW_max).
    pub fn cw_max(&self) -> u32 {
        match self {
            Self::VoiceSignaling => 7,
            Self::InteractiveVideo => 15,
            Self::BestEffort => 63,
            Self::Background => 1023,
        }
    }

    /// Maximum Channel Occupancy Time (MCOT) in milliseconds.
    pub fn mcot_ms(&self) -> u32 {
        match self {
            Self::VoiceSignaling => 2,
            Self::InteractiveVideo => 3,
            Self::BestEffort => 8,
            Self::Background => 8,
        }
    }

    /// Maximum Channel Occupancy Time (MCOT) in microseconds.
    pub fn mcot_us(&self) -> u32 {
        self.mcot_ms() * 1000
    }
}

/// 3GPP NR-U Channel Access Types (LBT Categories) per TS 38.213 §11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LbtType {
    /// Type 1: Category 4 LBT with random backoff and variable contention window.
    Type1Cat4,
    /// Type 2A: Category 2 LBT with fixed 25 us sensing without random backoff.
    Type2ACat2,
    /// Type 2B: Category 2 LBT with fixed 16 us sensing without random backoff.
    Type2BCat2,
    /// Type 2C: Category 1 LBT (immediate transmission, switching gap <= 16 us).
    Type2CCat1,
}

/// Operating bandwidth configuration for NR-U carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelBandwidthMhz {
    Bw20 = 20,
    Bw40 = 40,
    Bw80 = 80,
    Bw160 = 160,
}

impl ChannelBandwidthMhz {
    pub fn as_mhz(&self) -> u32 {
        *self as u32
    }
}

/// Energy Detection (ED) threshold configuration per regulatory standards (e.g. ETSI EN 301 893 / FCC Part 15).
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyDetectionConfig {
    pub bandwidth: ChannelBandwidthMhz,
    /// Maximum radiated transmit power in dBm (EIRP).
    pub tx_power_eirp_dbm: i32,
    /// Regulatory baseline threshold in dBm for 20 MHz (-72 dBm).
    pub baseline_threshold_dbm: f32,
}

impl Default for EnergyDetectionConfig {
    fn default() -> Self {
        Self {
            bandwidth: ChannelBandwidthMhz::Bw20,
            tx_power_eirp_dbm: 23,
            baseline_threshold_dbm: -72.0,
        }
    }
}

impl EnergyDetectionConfig {
    pub fn new(bandwidth: ChannelBandwidthMhz, tx_power_eirp_dbm: i32) -> Self {
        Self {
            bandwidth,
            tx_power_eirp_dbm,
            baseline_threshold_dbm: -72.0,
        }
    }

    /// Calculate regulatory maximum Energy Detection threshold:
    /// X_thresh = min(-72 dBm, -72 dBm + 10 * log10(B / 20 MHz) + (23 - P_tx))
    pub fn calculate_threshold_dbm(&self) -> f32 {
        let b_ratio = self.bandwidth.as_mhz() as f32 / 20.0;
        let bw_adjustment = 10.0 * (b_ratio.ln() / 10.0_f32.ln());
        let power_adjustment = (23 - self.tx_power_eirp_dbm) as f32;

        let formula_result = self.baseline_threshold_dbm + bw_adjustment + power_adjustment;
        if formula_result < self.baseline_threshold_dbm {
            formula_result
        } else {
            self.baseline_threshold_dbm
        }
    }
}

/// State of the LBT Channel Access Engine.
#[derive(Debug, Clone, PartialEq)]
pub enum LbtState {
    /// Medium idle, no transmission pending.
    Idle,
    /// Performing initial or deferral sensing (T_d duration).
    Deferring {
        capc: ChannelAccessPriorityClass,
        remaining_defer_us: u32,
        total_defer_us: u32,
    },
    /// Executing random backoff counter decrementation.
    Backoff {
        capc: ChannelAccessPriorityClass,
        counter_n: u32,
        current_cw: u32,
    },
    /// Channel sensed busy during sensing or backoff; counter frozen.
    Frozen {
        capc: ChannelAccessPriorityClass,
        frozen_counter_n: u32,
        current_cw: u32,
    },
    /// Medium successfully acquired! Transmitter is inside MCOT.
    ChannelAcquired {
        capc: ChannelAccessPriorityClass,
        lbt_type: LbtType,
        mcot_remaining_us: u32,
        elapsed_tx_us: u32,
    },
    /// Maximum Channel Occupancy Time (MCOT) exhausted; must release channel.
    CotExpired {
        capc: ChannelAccessPriorityClass,
        total_tx_us: u32,
    },
}

/// HARQ-ACK feedback received from the Reference Subframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarqFeedback {
    Ack,
    Nack,
    Dtx,
}

/// COT Sharing information element signaled from gNodeB to UE (TS 38.213 §11.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CotSharingInfo {
    pub target_ue_id: String,
    pub shared_duration_us: u32,
    pub allowed_ul_lbt: LbtType,
}

/// Channel Reservation Signal (CRS) / Extension Preamble for fractional slot alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelReservationSignal {
    pub duration_us: u32,
    pub target_slot_index: u32,
    pub scs_khz: u32,
    pub payload_bytes: Vec<u8>,
}

/// Telemetry metrics for NR-U channel access.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NrLbtMetrics {
    pub total_access_attempts: u64,
    pub successful_acquisitions: u64,
    pub busy_channel_detections: u64,
    pub backoff_freeze_events: u64,
    pub cwa_doubling_events: u64,
    pub cwa_reset_events: u64,
    pub total_tx_microseconds: u64,
}

/// 3GPP Rel-17 NR-U Listen-Before-Talk (LBT) & Unlicensed Channel Access Engine.
#[derive(Debug)]
pub struct NrLbtEngine {
    pub node_id: String,
    pub ed_config: EnergyDetectionConfig,
    pub state: LbtState,
    pub current_cws: HashMap<ChannelAccessPriorityClass, u32>,
    pub metrics: NrLbtMetrics,
    prng_state: u64,
}

impl NrLbtEngine {
    /// Creates a new NR-U LBT Channel Access Engine with node identifier and PRNG seed.
    pub fn new(node_id: &str, ed_config: EnergyDetectionConfig, seed: u64) -> Self {
        let mut current_cws = HashMap::new();
        current_cws.insert(
            ChannelAccessPriorityClass::VoiceSignaling,
            ChannelAccessPriorityClass::VoiceSignaling.cw_min(),
        );
        current_cws.insert(
            ChannelAccessPriorityClass::InteractiveVideo,
            ChannelAccessPriorityClass::InteractiveVideo.cw_min(),
        );
        current_cws.insert(
            ChannelAccessPriorityClass::BestEffort,
            ChannelAccessPriorityClass::BestEffort.cw_min(),
        );
        current_cws.insert(
            ChannelAccessPriorityClass::Background,
            ChannelAccessPriorityClass::Background.cw_min(),
        );

        let initial_seed = if seed == 0 {
            0xCAFE_BABE_DEAD_BEEF
        } else {
            seed
        };

        Self {
            node_id: node_id.to_string(),
            ed_config,
            state: LbtState::Idle,
            current_cws,
            metrics: NrLbtMetrics::default(),
            prng_state: initial_seed,
        }
    }

    /// Fast 64-bit Xorshift pseudo-random number generator for backoff counter draws.
    fn next_random_u32(&mut self) -> u32 {
        let mut x = self.prng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.prng_state = x;
        (x & 0xFFFF_FFFF) as u32
    }

    /// Draw uniform random counter N in [0, CW_p] for Category 4 LBT backoff.
    pub fn draw_backoff_counter(&mut self, capc: ChannelAccessPriorityClass) -> u32 {
        let cw = *self.current_cws.get(&capc).unwrap_or(&capc.cw_min());
        self.next_random_u32() % (cw + 1)
    }

    /// Request channel access with given CAPC and LBT Type.
    pub fn request_channel_access(
        &mut self,
        capc: ChannelAccessPriorityClass,
        lbt_type: LbtType,
    ) -> Result<LbtState, &'static str> {
        self.metrics.total_access_attempts += 1;

        match lbt_type {
            LbtType::Type2CCat1 => {
                // Immediate transmission (gap <= 16 us within COT)
                self.metrics.successful_acquisitions += 1;
                let state = LbtState::ChannelAcquired {
                    capc,
                    lbt_type,
                    mcot_remaining_us: capc.mcot_us(),
                    elapsed_tx_us: 0,
                };
                self.state = state.clone();
                Ok(state)
            }
            LbtType::Type2ACat2 => {
                let defer_us = 25;
                let state = LbtState::Deferring {
                    capc,
                    remaining_defer_us: defer_us,
                    total_defer_us: defer_us,
                };
                self.state = state.clone();
                Ok(state)
            }
            LbtType::Type2BCat2 => {
                let defer_us = 16;
                let state = LbtState::Deferring {
                    capc,
                    remaining_defer_us: defer_us,
                    total_defer_us: defer_us,
                };
                self.state = state.clone();
                Ok(state)
            }
            LbtType::Type1Cat4 => {
                let defer_us = capc.defer_duration_us();
                let state = LbtState::Deferring {
                    capc,
                    remaining_defer_us: defer_us,
                    total_defer_us: defer_us,
                };
                self.state = state.clone();
                Ok(state)
            }
        }
    }

    /// Execute a single 9 us Clear Channel Assessment (CCA) slot with measured medium energy (dBm).
    pub fn step_cca_slot_9us(&mut self, measured_energy_dbm: f32) -> LbtState {
        let threshold = self.ed_config.calculate_threshold_dbm();
        let is_busy = measured_energy_dbm >= threshold;

        if is_busy {
            self.metrics.busy_channel_detections += 1;
        }

        match self.state.clone() {
            LbtState::Idle => LbtState::Idle,

            LbtState::Deferring {
                capc,
                remaining_defer_us,
                total_defer_us,
            } => {
                if is_busy {
                    // Medium busy: reset defer timer back to full duration
                    self.state = LbtState::Deferring {
                        capc,
                        remaining_defer_us: total_defer_us,
                        total_defer_us,
                    };
                } else {
                    let step = 9.min(remaining_defer_us);
                    let new_remaining = remaining_defer_us.saturating_sub(step);
                    if new_remaining == 0 {
                        // Defer period completed!
                        // Check if this was a Type 2A/2B access or Type 1
                        if total_defer_us == 25 || total_defer_us == 16 {
                            // Type 2 completed without backoff!
                            self.metrics.successful_acquisitions += 1;
                            self.state = LbtState::ChannelAcquired {
                                capc,
                                lbt_type: if total_defer_us == 25 {
                                    LbtType::Type2ACat2
                                } else {
                                    LbtType::Type2BCat2
                                },
                                mcot_remaining_us: capc.mcot_us(),
                                elapsed_tx_us: 0,
                            };
                        } else {
                            // Type 1: proceed to random backoff stage
                            let cw = *self.current_cws.get(&capc).unwrap_or(&capc.cw_min());
                            let counter_n = self.draw_backoff_counter(capc);
                            if counter_n == 0 {
                                // Counter drew 0: immediate acquisition!
                                self.metrics.successful_acquisitions += 1;
                                self.state = LbtState::ChannelAcquired {
                                    capc,
                                    lbt_type: LbtType::Type1Cat4,
                                    mcot_remaining_us: capc.mcot_us(),
                                    elapsed_tx_us: 0,
                                };
                            } else {
                                self.state = LbtState::Backoff {
                                    capc,
                                    counter_n,
                                    current_cw: cw,
                                };
                            }
                        }
                    } else {
                        self.state = LbtState::Deferring {
                            capc,
                            remaining_defer_us: new_remaining,
                            total_defer_us,
                        };
                    }
                }
                self.state.clone()
            }

            LbtState::Backoff {
                capc,
                counter_n,
                current_cw,
            } => {
                if is_busy {
                    // Medium busy: freeze counter and enter Frozen state
                    self.metrics.backoff_freeze_events += 1;
                    self.state = LbtState::Frozen {
                        capc,
                        frozen_counter_n: counter_n,
                        current_cw,
                    };
                } else {
                    // Decrement backoff counter
                    let next_n = counter_n.saturating_sub(1);
                    if next_n == 0 {
                        // Channel successfully acquired!
                        self.metrics.successful_acquisitions += 1;
                        self.state = LbtState::ChannelAcquired {
                            capc,
                            lbt_type: LbtType::Type1Cat4,
                            mcot_remaining_us: capc.mcot_us(),
                            elapsed_tx_us: 0,
                        };
                    } else {
                        self.state = LbtState::Backoff {
                            capc,
                            counter_n: next_n,
                            current_cw,
                        };
                    }
                }
                self.state.clone()
            }

            LbtState::Frozen {
                capc,
                frozen_counter_n,
                current_cw,
            } => {
                if !is_busy {
                    // Channel became idle: resume backoff with preserved counter
                    self.state = LbtState::Backoff {
                        capc,
                        counter_n: frozen_counter_n,
                        current_cw,
                    };
                }
                self.state.clone()
            }

            LbtState::ChannelAcquired { .. } | LbtState::CotExpired { .. } => self.state.clone(),
        }
    }

    /// Fast-forward Type 2 sensing (16 us or 25 us) in a single atomic check.
    pub fn step_type2_sensing(
        &mut self,
        sensing_duration_us: u32,
        measured_energy_dbm: f32,
    ) -> bool {
        let threshold = self.ed_config.calculate_threshold_dbm();
        if measured_energy_dbm >= threshold {
            self.metrics.busy_channel_detections += 1;
            return false;
        }

        if let LbtState::Deferring {
            capc,
            total_defer_us,
            ..
        } = self.state
        {
            if total_defer_us == sensing_duration_us {
                self.metrics.successful_acquisitions += 1;
                self.state = LbtState::ChannelAcquired {
                    capc,
                    lbt_type: if sensing_duration_us == 25 {
                        LbtType::Type2ACat2
                    } else {
                        LbtType::Type2BCat2
                    },
                    mcot_remaining_us: capc.mcot_us(),
                    elapsed_tx_us: 0,
                };
                return true;
            }
        }
        false
    }

    /// Dynamic Contention Window Adaptation (CWA) per TS 38.213 §11.1.4.
    ///
    /// Evaluates HARQ-ACK feedback from the Reference Subframe:
    /// - If (NACK + DTX) / Total >= 80%: CW is doubled: CW = min(2 * (CW + 1) - 1, CW_max).
    /// - If (NACK + DTX) / Total < 80%: CW is reset to CW_min.
    /// Returns the newly updated CW value.
    pub fn process_harq_reference_subframe(
        &mut self,
        capc: ChannelAccessPriorityClass,
        feedbacks: &[HarqFeedback],
    ) -> u32 {
        if feedbacks.is_empty() {
            return *self.current_cws.get(&capc).unwrap_or(&capc.cw_min());
        }

        let mut nack_or_dtx_count = 0;
        for fb in feedbacks {
            match fb {
                HarqFeedback::Nack | HarqFeedback::Dtx => nack_or_dtx_count += 1,
                HarqFeedback::Ack => {}
            }
        }

        let nack_ratio = nack_or_dtx_count as f32 / feedbacks.len() as f32;
        let old_cw = *self.current_cws.get(&capc).unwrap_or(&capc.cw_min());

        let new_cw = if nack_ratio >= 0.80 {
            // Collision inferred: double CW
            self.metrics.cwa_doubling_events += 1;
            let doubled = 2 * (old_cw + 1) - 1;
            doubled.min(capc.cw_max())
        } else {
            // Success: reset to CW_min
            self.metrics.cwa_reset_events += 1;
            capc.cw_min()
        };

        self.current_cws.insert(capc, new_cw);
        new_cw
    }

    /// Consume transmission time within the acquired MCOT.
    ///
    /// Returns Ok(remaining_mcot_us) or Err("MCOT expired").
    pub fn consume_transmission_time(&mut self, duration_us: u32) -> Result<u32, &'static str> {
        match self.state {
            LbtState::ChannelAcquired {
                capc,
                lbt_type,
                mcot_remaining_us,
                elapsed_tx_us,
            } => {
                self.metrics.total_tx_microseconds += duration_us as u64;

                if duration_us >= mcot_remaining_us {
                    let total_tx = elapsed_tx_us + mcot_remaining_us;
                    self.state = LbtState::CotExpired {
                        capc,
                        total_tx_us: total_tx,
                    };
                    Err("MCOT expired")
                } else {
                    let next_remaining = mcot_remaining_us - duration_us;
                    let next_elapsed = elapsed_tx_us + duration_us;
                    self.state = LbtState::ChannelAcquired {
                        capc,
                        lbt_type,
                        mcot_remaining_us: next_remaining,
                        elapsed_tx_us: next_elapsed,
                    };
                    Ok(next_remaining)
                }
            }
            _ => Err("Channel is not acquired"),
        }
    }

    /// Release the channel and transition back to Idle state.
    pub fn release_channel(&mut self) {
        self.state = LbtState::Idle;
    }

    /// Construct a Channel Reservation Signal (CRS) / Extension Preamble to hold the medium
    /// between LBT completion and the scheduled 5G NR slot boundary.
    pub fn generate_channel_reservation_signal(
        &self,
        remaining_gap_us: u32,
        target_slot: u32,
        scs_khz: u32,
    ) -> ChannelReservationSignal {
        // Pattern: Alternating pilot tones / Zadoff-Chu sequence representation
        let pattern_len = (remaining_gap_us.min(1000) as usize).max(16);
        let mut payload = Vec::with_capacity(pattern_len);
        for i in 0..pattern_len {
            payload.push(((i * 37 + (scs_khz as usize)) & 0xFF) as u8);
        }

        ChannelReservationSignal {
            duration_us: remaining_gap_us,
            target_slot_index: target_slot,
            scs_khz,
            payload_bytes: payload,
        }
    }

    /// Share remaining acquired Channel Occupancy Time (COT) with a scheduled UE (TS 38.213 §11.2).
    pub fn create_cot_sharing(
        &self,
        target_ue_id: &str,
        duration_to_share_us: u32,
    ) -> Result<CotSharingInfo, &'static str> {
        match self.state {
            LbtState::ChannelAcquired {
                mcot_remaining_us, ..
            } => {
                if duration_to_share_us > mcot_remaining_us {
                    return Err("Cannot share more duration than remaining MCOT");
                }

                // If gap between DL and UL is <= 16 us -> Type 2C (no LBT).
                // If gap is <= 25 us -> Type 2A.
                let allowed_lbt = if duration_to_share_us <= 16 {
                    LbtType::Type2CCat1
                } else {
                    LbtType::Type2ACat2
                };

                Ok(CotSharingInfo {
                    target_ue_id: target_ue_id.to_string(),
                    shared_duration_us: duration_to_share_us,
                    allowed_ul_lbt: allowed_lbt,
                })
            }
            _ => Err("Cannot share COT when channel is not acquired"),
        }
    }

    /// Wideband Carrier Sensing & Dynamic Bandwidth Puncturing (TS 37.213 §4.1.3).
    ///
    /// Evaluates primary 20 MHz carrier and secondary 20 MHz sub-bands.
    /// If primary carrier is busy, returns 0 MHz (cannot transmit).
    /// If primary carrier is idle, checks contiguous secondary sub-bands to determine
    /// actual transmitted bandwidth (20, 40, 80, or 160 MHz) without violating etiquette.
    pub fn sense_and_puncture_wideband(
        &self,
        primary_energy_dbm: f32,
        secondary_energies_dbm: &[f32],
    ) -> u32 {
        let threshold = self.ed_config.calculate_threshold_dbm();

        // Primary 20 MHz carrier must be idle
        if primary_energy_dbm >= threshold {
            return 0;
        }

        let mut available_bw = 20;

        // Secondary 20 MHz (to reach 40 MHz)
        if !secondary_energies_dbm.is_empty() && secondary_energies_dbm[0] < threshold {
            available_bw = 40;

            // Secondary 40 MHz (to reach 80 MHz, requires 2 more 20 MHz sub-bands idle)
            if secondary_energies_dbm.len() >= 3
                && secondary_energies_dbm[1] < threshold
                && secondary_energies_dbm[2] < threshold
            {
                available_bw = 80;

                // Secondary 80 MHz (to reach 160 MHz, requires 4 more 20 MHz sub-bands idle)
                if secondary_energies_dbm.len() >= 7
                    && secondary_energies_dbm[3..7].iter().all(|&e| e < threshold)
                {
                    available_bw = 160;
                }
            }
        }

        available_bw
    }
}
