//! 3GPP Rel-17 5G NR Connected-Mode DRX (Discontinuous Reception) & Power Saving Engine.
//!
//! Compliant with 3GPP TS 38.321 Rel-17 Section 5.7, TS 38.331 (`DRX-Config`),
//! and TS 38.213 (Rel-17 DCI format 2_6 Wake-Up Signal).
//!
//! Controls slot-by-slot PDCCH monitoring (Active Time) vs RF transceiver deactivation (Sleep)
//! to maximize battery savings across smartphones, RedCap IoT devices, and Industrial sensors.
//!
//! Features:
//! - Dual-tier DRX cycles: High-responsiveness Short DRX and deep-sleep Long DRX.
//! - Timers: drx-onDurationTimer, drx-InactivityTimer, drx-HARQ-RTT-TimerDL/UL, drx-RetransmissionTimerDL/UL.
//! - Active Time evaluation per TS 38.321 §5.7 including pending SR and RA contention.
//! - MAC Control Elements: DRX Command MAC CE (LCID 60) and Long DRX Command MAC CE (LCID 61).
//! - Rel-17 Power Saving Signal (WUS / DCI 2_6) evaluation allowing on-duration skip.
//! - Power consumption duty cycle telemetry and battery conservation analytics.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Configuration & Types per 3GPP TS 38.331 (`DRX-Config`)
// ---------------------------------------------------------------------------

/// Short DRX cycle configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortDrxConfig {
    /// Short DRX cycle length in slots (e.g. 20, 40, 80 slots).
    pub drx_short_cycle_slots: u32,
    /// Number of consecutive short cycles before transitioning to Long DRX (1..16).
    pub drx_short_cycle_timer_count: u16,
}

/// Connected-Mode DRX Configuration (TS 38.331 §6.3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrxConfig {
    /// Duration of on-duration timer in slots (e.g. 10 slots).
    pub drx_on_duration_slots: u16,
    /// Duration of inactivity timer in slots (restarted on new transmission).
    pub drx_inactivity_slots: u16,
    /// Downlink HARQ RTT timer in slots (e.g. 8 slots).
    pub drx_harq_rtt_timer_dl: u16,
    /// Uplink HARQ RTT timer in slots (e.g. 8 slots).
    pub drx_harq_rtt_timer_ul: u16,
    /// Downlink retransmission timer in slots (e.g. 16 slots).
    pub drx_retransmission_timer_dl: u16,
    /// Uplink retransmission timer in slots (e.g. 16 slots).
    pub drx_retransmission_timer_ul: u16,
    /// Long DRX cycle length in slots (e.g. 160, 320, 640, 1280 slots).
    pub drx_long_cycle_slots: u32,
    /// DRX start offset within the cycle in slots (0..long_cycle - 1).
    pub drx_start_offset_slots: u32,
    /// Optional Short DRX configuration.
    pub short_drx: Option<ShortDrxConfig>,
    /// Rel-17 Wake-Up Signal (WUS / DCI 2_6) monitoring enabled.
    pub dci_2_6_wus_enabled: bool,
}

/// Current active DRX Cycle mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrxCycleMode {
    LongDrx,
    ShortDrx,
}

/// 3GPP TS 38.321 MAC Control Elements for DRX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrxMacCe {
    /// DRX Command MAC CE (LCID 60 / 0x3C): starts Short DRX or Long DRX.
    DrxCommand,
    /// Long DRX Command MAC CE (LCID 61 / 0x3D): forces immediate transition to Long DRX.
    LongDrxCommand,
}

/// Primary reason for UE being in Active Time (PDCCH monitoring).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveReason {
    OnDurationTimer,
    InactivityTimer,
    HarqRetransmissionDL,
    HarqRetransmissionUL,
    RaContentionResolution,
    SchedulingRequestPending,
}

/// State of the UE radio transceiver during a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrxActivity {
    /// Active Time: UE transceiver is powered on and monitoring PDCCH.
    ActiveTime(ActiveReason),
    /// Sleep: UE transceiver is powered down for power saving.
    Sleep,
}

impl DrxActivity {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::ActiveTime(_))
    }
}

/// Per-process HARQ RTT and Retransmission tracking state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarqProcessState {
    pub rtt_timer_remaining: u16,
    pub retrans_timer_remaining: u16,
    pub nack_pending: bool,
}

// ---------------------------------------------------------------------------
// 5G NR Connected-Mode DRX Engine
// ---------------------------------------------------------------------------

/// 3GPP Rel-17 5G NR Connected-Mode DRX Engine.
#[derive(Debug)]
pub struct NrDrxEngine {
    pub config: DrxConfig,
    pub current_cycle_mode: DrxCycleMode,
    pub slots_per_frame: u16,
    pub current_sfn: u16,  // 0..1023
    pub current_slot: u16, // 0..slots_per_frame - 1

    // Protocol Timers
    pub on_duration_timer: u16,
    pub inactivity_timer: u16,
    pub short_cycle_cycles_left: u16,
    pub ra_contention_timer: u16,
    pub sr_pending: bool,

    // HARQ process timers
    pub harq_dl: HashMap<u8, HarqProcessState>,
    pub harq_ul: HashMap<u8, HarqProcessState>,

    // Rel-17 WUS state
    pub wus_wake_up_flag: bool,

    // Telemetry
    pub total_slots_elapsed: u64,
    pub active_slots_count: u64,
    pub sleep_slots_count: u64,
}

impl NrDrxEngine {
    /// Create a new 5G NR DRX engine instance.
    ///
    /// - `slots_per_frame`: 10 for 15 kHz subcarrier spacing (SCS), 20 for 30 kHz SCS, 40 for 60 kHz SCS.
    pub fn new(config: DrxConfig, slots_per_frame: u16) -> Self {
        Self {
            config,
            current_cycle_mode: DrxCycleMode::LongDrx,
            slots_per_frame,
            current_sfn: 0,
            current_slot: 0,
            on_duration_timer: 0,
            inactivity_timer: 0,
            short_cycle_cycles_left: 0,
            ra_contention_timer: 0,
            sr_pending: false,
            harq_dl: HashMap::new(),
            harq_ul: HashMap::new(),
            wus_wake_up_flag: true,
            total_slots_elapsed: 0,
            active_slots_count: 0,
            sleep_slots_count: 0,
        }
    }

    /// Advance simulation clock by one slot and determine PDCCH Active Time vs Sleep.
    pub fn step_slot(&mut self) -> DrxActivity {
        self.total_slots_elapsed += 1;

        // 1. Calculate absolute slot number across hyperframes
        let absolute_slot = (self.current_sfn as u64)
            .wrapping_mul(self.slots_per_frame as u64)
            .wrapping_add(self.current_slot as u64);

        // 2. Evaluate DRX Cycle start conditions (TS 38.321 §5.7)
        let cycle_len = match self.current_cycle_mode {
            DrxCycleMode::ShortDrx => {
                if let Some(ref short) = self.config.short_drx {
                    short.drx_short_cycle_slots as u64
                } else {
                    self.config.drx_long_cycle_slots as u64
                }
            }
            DrxCycleMode::LongDrx => self.config.drx_long_cycle_slots as u64,
        };

        let expected_offset = (self.config.drx_start_offset_slots as u64) % cycle_len;
        let is_cycle_start = (absolute_slot % cycle_len) == expected_offset;

        if is_cycle_start {
            // Check Short DRX cycle countdown
            if self.current_cycle_mode == DrxCycleMode::ShortDrx {
                if self.short_cycle_cycles_left > 0 {
                    self.short_cycle_cycles_left -= 1;
                    if self.short_cycle_cycles_left == 0 {
                        // Short DRX expired: transition to Long DRX!
                        self.current_cycle_mode = DrxCycleMode::LongDrx;
                    }
                }
            }

            // If Wake-Up Signal allows, start onDurationTimer
            if self.wus_wake_up_flag {
                self.on_duration_timer = self.config.drx_on_duration_slots;
            } else {
                // Rel-17 Power Saving: WUS instructed to skip this On-Duration!
                self.wus_wake_up_flag = true; // reset for next cycle
            }
        }

        // 3. Check HARQ Retransmission status
        let mut retrans_dl_active = false;
        for proc in self.harq_dl.values_mut() {
            if proc.rtt_timer_remaining == 1 && proc.nack_pending {
                // RTT timer expires at end of this slot; arm retransmission timer
                proc.retrans_timer_remaining = self.config.drx_retransmission_timer_dl;
                proc.nack_pending = false;
            }
            if proc.retrans_timer_remaining > 0 {
                retrans_dl_active = true;
            }
        }

        let mut retrans_ul_active = false;
        for proc in self.harq_ul.values_mut() {
            if proc.rtt_timer_remaining == 1 && proc.nack_pending {
                proc.retrans_timer_remaining = self.config.drx_retransmission_timer_ul;
                proc.nack_pending = false;
            }
            if proc.retrans_timer_remaining > 0 {
                retrans_ul_active = true;
            }
        }

        // 4. Evaluate Active Time (TS 38.321 §5.7)
        let activity = if self.on_duration_timer > 0 {
            DrxActivity::ActiveTime(ActiveReason::OnDurationTimer)
        } else if self.inactivity_timer > 0 {
            DrxActivity::ActiveTime(ActiveReason::InactivityTimer)
        } else if retrans_dl_active {
            DrxActivity::ActiveTime(ActiveReason::HarqRetransmissionDL)
        } else if retrans_ul_active {
            DrxActivity::ActiveTime(ActiveReason::HarqRetransmissionUL)
        } else if self.ra_contention_timer > 0 {
            DrxActivity::ActiveTime(ActiveReason::RaContentionResolution)
        } else if self.sr_pending {
            DrxActivity::ActiveTime(ActiveReason::SchedulingRequestPending)
        } else {
            DrxActivity::Sleep
        };

        // Telemetry update
        if activity.is_active() {
            self.active_slots_count += 1;
        } else {
            self.sleep_slots_count += 1;
        }

        // 5. Decrement running timers at end of slot
        if self.on_duration_timer > 0 {
            self.on_duration_timer -= 1;
        }
        if self.inactivity_timer > 0 {
            self.inactivity_timer -= 1;
        }
        if self.ra_contention_timer > 0 {
            self.ra_contention_timer -= 1;
        }

        for proc in self.harq_dl.values_mut() {
            if proc.rtt_timer_remaining > 0 {
                proc.rtt_timer_remaining -= 1;
            } else if proc.retrans_timer_remaining > 0 {
                proc.retrans_timer_remaining -= 1;
            }
        }

        for proc in self.harq_ul.values_mut() {
            if proc.rtt_timer_remaining > 0 {
                proc.rtt_timer_remaining -= 1;
            } else if proc.retrans_timer_remaining > 0 {
                proc.retrans_timer_remaining -= 1;
            }
        }

        // 6. Advance slot and SFN counters
        self.current_slot += 1;
        if self.current_slot >= self.slots_per_frame {
            self.current_slot = 0;
            self.current_sfn = (self.current_sfn + 1) % 1024;
        }

        activity
    }

    /// Notify that PDCCH indicates a new transmission (UL or DL).
    ///
    /// Restarts drx-InactivityTimer and starts HARQ RTT timer (TS 38.321 §5.7).
    pub fn notify_new_transmission(&mut self, is_downlink: bool, harq_id: u8) {
        // Restart Inactivity Timer
        self.inactivity_timer = self.config.drx_inactivity_slots;

        // Start HARQ RTT timer
        let rtt_val = if is_downlink {
            self.config.drx_harq_rtt_timer_dl
        } else {
            self.config.drx_harq_rtt_timer_ul
        };

        let proc = HarqProcessState {
            rtt_timer_remaining: rtt_val,
            retrans_timer_remaining: 0,
            nack_pending: false,
        };

        if is_downlink {
            self.harq_dl.insert(harq_id, proc);
        } else {
            self.harq_ul.insert(harq_id, proc);
        }
    }

    /// Notify that HARQ transmission was not successfully decoded (NACK).
    pub fn notify_harq_nack(&mut self, is_downlink: bool, harq_id: u8) {
        let map = if is_downlink {
            &mut self.harq_dl
        } else {
            &mut self.harq_ul
        };

        if let Some(proc) = map.get_mut(&harq_id) {
            proc.nack_pending = true;
        }
    }

    /// Process received MAC Control Element for DRX (TS 38.321 §5.7).
    pub fn process_mac_ce(&mut self, ce: DrxMacCe) {
        match ce {
            DrxMacCe::DrxCommand => {
                // Stop on_duration and inactivity timers
                self.on_duration_timer = 0;
                self.inactivity_timer = 0;

                if let Some(ref short) = self.config.short_drx {
                    // Start or restart Short DRX cycle
                    self.current_cycle_mode = DrxCycleMode::ShortDrx;
                    self.short_cycle_cycles_left = short.drx_short_cycle_timer_count;
                } else {
                    self.current_cycle_mode = DrxCycleMode::LongDrx;
                }
            }
            DrxMacCe::LongDrxCommand => {
                // Stop on_duration, inactivity, and short_cycle timers
                self.on_duration_timer = 0;
                self.inactivity_timer = 0;
                self.short_cycle_cycles_left = 0;
                self.current_cycle_mode = DrxCycleMode::LongDrx;
            }
        }
    }

    /// Update Rel-17 DCI format 2_6 Wake-Up Signal indication for next cycle.
    pub fn set_wus_indication(&mut self, wake_up: bool) {
        if self.config.dci_2_6_wus_enabled {
            self.wus_wake_up_flag = wake_up;
        }
    }

    /// Update Scheduling Request (SR) pending status on PUCCH.
    pub fn set_sr_pending(&mut self, pending: bool) {
        self.sr_pending = pending;
    }

    /// Set RA Contention Resolution timer duration (e.g. during RACH Msg3 PUSCH).
    pub fn set_ra_contention_timer(&mut self, duration_slots: u16) {
        self.ra_contention_timer = duration_slots;
    }

    /// Active Duty Cycle ratio: fraction of time transceiver is powered on.
    pub fn active_duty_cycle(&self) -> f64 {
        if self.total_slots_elapsed == 0 {
            1.0
        } else {
            self.active_slots_count as f64 / self.total_slots_elapsed as f64
        }
    }

    /// RF transceiver battery energy savings percentage compared to continuous reception.
    pub fn energy_savings_percentage(&self) -> f64 {
        (1.0 - self.active_duty_cycle()) * 100.0
    }
}
