//! O-RAN WG4 Open Fronthaul Energy Savings Management (ESM) Engine.
//!
//! Conforms to:
//! - O-RAN.WG4.MP.0 Section 15: Energy Saving Management.
//! - `o-ran-energy-saving.yang`: YANG Data Model for Energy Saving Management.
//! - ETSI ES 203 228 / 3GPP TS 32.551: Energy Efficiency metrics in telecommunications.
//!
//! Pure standard Rust (`std` / `core` only) with zero external dependencies.

use std::collections::{HashMap, HashSet};

// ===========================================================================
// 1. Energy Saving States & Modes
// ===========================================================================

/// Energy saving operational state of the O-RU (o-ran-energy-saving.yang).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergySavingState {
    /// Fully active normal operation: all configured PAs and RF chains powered.
    Active,
    /// Energy saving mode actively engaged: power consumption minimized.
    Sleep,
    /// Extreme low-power hibernation state (C-Plane heartbeat only).
    DeepSleep,
    /// Transitioning from Active to Sleep: draining buffers, ramping down PA bias.
    TransitioningToSleep,
    /// Waking up from Sleep to Active: PLL lock, PA warmup, recalibration.
    WakingUp,
}

/// Category of Energy Saving technique deployed on the O-RU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnergySavingMode {
    /// Symbol/mini-slot level sleep: gating PA bias during unallocated DL symbols.
    MicroSleep,
    /// Disabling a subset of antenna branches (e.g. 64T64R -> 32T32R or 16T16R).
    TxArraySleep,
    /// Complete RF and digital chain deactivation of secondary/capacity carriers.
    CarrierSleep,
    /// Complete O-RU hibernation during extreme low-traffic hours (C-Plane heartbeat only).
    DeepSleep,
}

// ===========================================================================
// 2. Hardware Profile & Physical Power Model
// ===========================================================================

/// Component carrier operational state inside the O-RU.
#[derive(Debug, Clone, PartialEq)]
pub struct CarrierOperationalStatus {
    pub carrier_id: u8,
    pub is_primary: bool,
    pub is_active: bool,
    pub active_trx_count: u16,
    pub max_trx_count: u16,
    /// Current PRB utilization percentage [0.0, 100.0]
    pub prb_utilization_percent: f32,
    /// RF power output per active branch in Watts (e.g. 5.0W = 37 dBm)
    pub rf_power_per_branch_watts: f64,
}

/// Physical and electrical parameters of the O-RU hardware platform.
#[derive(Debug, Clone)]
pub struct OranRuHardwareProfile {
    /// Baseline digital baseband / FPGA / eCPRI processor power in Watts
    pub digital_baseline_watts: f64,
    /// Standby power draw per TRX RF branch (mixers, DAC/ADC, filters) in Watts
    pub trx_baseline_watts: f64,
    /// Total installed TRX branches (e.g. 64 for 64T64R Massive MIMO)
    pub total_installed_trx: u16,
    /// Power Amplifier (PA) power-added efficiency (PAE, e.g. 0.45 = 45%)
    pub pa_efficiency: f64,
    /// Micro-sleep PA bias quiescent reduction ratio (e.g. 0.50 = 50% savings during idle symbols)
    pub micro_sleep_quiescent_reduction: f64,
    /// Deep sleep residual maintenance power in Watts (e.g. 15.0W)
    pub deep_sleep_baseline_watts: f64,
    /// Cooling fan base power in Watts
    pub cooling_base_watts: f64,
}

impl Default for OranRuHardwareProfile {
    fn default() -> Self {
        Self {
            digital_baseline_watts: 80.0,
            trx_baseline_watts: 1.5,
            total_installed_trx: 64,
            pa_efficiency: 0.45,
            micro_sleep_quiescent_reduction: 0.50,
            deep_sleep_baseline_watts: 18.0,
            cooling_base_watts: 12.0,
        }
    }
}

// ===========================================================================
// 3. Micro-Sleep Gating Engine
// ===========================================================================

/// Evaluates sub-millisecond slot transmission patterns for PA bias gating.
#[derive(Debug)]
pub struct MicroSleepGater {
    pub is_enabled: bool,
    /// Quiescent reduction ratio when gated
    pub reduction_factor: f64,
}

impl MicroSleepGater {
    pub fn new(reduction_factor: f64) -> Self {
        Self {
            is_enabled: true,
            reduction_factor,
        }
    }

    /// Evaluates a 14-symbol 5G NR slot DL allocation mask (true = active DL transmission, false = idle).
    /// Returns the fraction of slot duration where PA bias was shut down.
    pub fn evaluate_slot_idle_ratio(&self, dl_symbol_mask: &[bool; 14]) -> f64 {
        if !self.is_enabled {
            return 0.0;
        }
        let idle_symbols = dl_symbol_mask.iter().filter(|&&active| !active).count();
        idle_symbols as f64 / 14.0
    }
}

// ===========================================================================
// 4. Carrier Sleep Calendar & Traffic Watchdog
// ===========================================================================

/// Scheduled Carrier Sleep window.
#[derive(Debug, Clone)]
pub struct CarrierSleepSchedule {
    pub schedule_id: u32,
    /// Second of day at which sleep begins (0..86399, e.g. 7200 for 02:00:00)
    pub start_second_of_day: u32,
    /// Duration of sleep window in seconds (e.g. 14400 for 4 hours)
    pub duration_seconds: u32,
    /// Target component carriers to sleep (e.g. [1] for SCell)
    pub target_carrier_ids: Vec<u8>,
    /// Traffic safety guardrail: if primary carrier PRB load exceeds this %, wake up carriers
    pub emergency_wake_prb_threshold: f32,
}

impl CarrierSleepSchedule {
    pub fn is_time_in_window(&self, second_of_day: u32) -> bool {
        let end = self.start_second_of_day + self.duration_seconds;
        if end <= 86400 {
            second_of_day >= self.start_second_of_day && second_of_day < end
        } else {
            // Wraps past midnight
            let end_wrap = end % 86400;
            second_of_day >= self.start_second_of_day || second_of_day < end_wrap
        }
    }
}

// ===========================================================================
// 5. O-RAN Energy Savings Manager
// ===========================================================================

/// Event notification emitted by the ESM engine.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergySavingEvent {
    pub timestamp_seconds: u64,
    pub previous_state: EnergySavingState,
    pub new_state: EnergySavingState,
    pub active_modes: Vec<EnergySavingMode>,
    pub current_power_watts: f64,
    pub message: String,
}

/// Comprehensive Energy Consumption and Savings Report.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyConsumptionReport {
    pub instantaneous_power_watts: f64,
    pub baseline_active_power_watts: f64,
    pub power_savings_percent: f64,
    pub cumulative_energy_consumed_kwh: f64,
    pub cumulative_energy_saved_kwh: f64,
    pub cumulative_cost_saved_usd: f64,
    pub estimated_co2_saved_kg: f64,
}

/// Main O-RAN Energy Savings Management Engine.
#[derive(Debug)]
pub struct OranEnergySavingsManager {
    pub hardware: OranRuHardwareProfile,
    pub state: EnergySavingState,
    pub active_modes: HashSet<EnergySavingMode>,
    pub carriers: HashMap<u8, CarrierOperationalStatus>,
    pub micro_sleep: MicroSleepGater,
    pub schedules: Vec<CarrierSleepSchedule>,
    /// Target wake-up latency budget in milliseconds
    pub max_acceptable_wakeup_latency_ms: u32,
    /// Measured current wakeup latency in milliseconds
    pub last_wakeup_latency_ms: u32,
    /// Energy metrics
    pub cumulative_energy_consumed_joules: f64,
    pub cumulative_baseline_energy_joules: f64,
    pub event_log: Vec<EnergySavingEvent>,
    pub last_tick_timestamp: u64,
}

impl OranEnergySavingsManager {
    pub fn new(hardware: OranRuHardwareProfile) -> Self {
        let micro_sleep_factor = hardware.micro_sleep_quiescent_reduction;
        Self {
            hardware,
            state: EnergySavingState::Active,
            active_modes: HashSet::new(),
            carriers: HashMap::new(),
            micro_sleep: MicroSleepGater::new(micro_sleep_factor),
            schedules: Vec::new(),
            max_acceptable_wakeup_latency_ms: 100, // 100ms default
            last_wakeup_latency_ms: 25,
            cumulative_energy_consumed_joules: 0.0,
            cumulative_baseline_energy_joules: 0.0,
            event_log: Vec::new(),
            last_tick_timestamp: 0,
        }
    }

    /// Register a component carrier in the O-RU.
    pub fn add_carrier(&mut self, carrier: CarrierOperationalStatus) {
        self.carriers.insert(carrier.carrier_id, carrier);
    }

    /// Add a scheduled carrier sleep window.
    pub fn add_schedule(&mut self, schedule: CarrierSleepSchedule) {
        self.schedules.push(schedule);
    }

    /// Calculate instantaneous baseline power (if all carriers/TRX were 100% active).
    pub fn calculate_baseline_power_watts(&self) -> f64 {
        let mut total = self.hardware.digital_baseline_watts + self.hardware.cooling_base_watts;

        for carrier in self.carriers.values() {
            let trx_count = carrier.max_trx_count as f64;
            // Transceiver baseline
            total += trx_count * self.hardware.trx_baseline_watts;
            // Full PA power at 100% load
            let pa_rf_power = trx_count * carrier.rf_power_per_branch_watts;
            total += pa_rf_power / self.hardware.pa_efficiency;
        }

        total
    }

    /// Calculate instantaneous real-time power consumption under active energy-saving states.
    pub fn calculate_instantaneous_power_watts(&self, current_slot_idle_ratio: f64) -> f64 {
        match self.state {
            EnergySavingState::DeepSleep => {
                // In Deep Sleep, only base maintenance controller is powered
                self.hardware.deep_sleep_baseline_watts
            }
            EnergySavingState::WakingUp | EnergySavingState::TransitioningToSleep => {
                // Transitional state power is slightly below baseline
                self.calculate_baseline_power_watts() * 0.85
            }
            EnergySavingState::Active | EnergySavingState::Sleep => {
                let mut power =
                    self.hardware.digital_baseline_watts + self.hardware.cooling_base_watts;

                for carrier in self.carriers.values() {
                    if !carrier.is_active {
                        // Carrier sleep: 0W for RF chains of this carrier
                        continue;
                    }

                    let active_trx = carrier.active_trx_count as f64;
                    // TRX baseline
                    power += active_trx * self.hardware.trx_baseline_watts;

                    // PA RF power
                    let utilization = (carrier.prb_utilization_percent / 100.0) as f64;
                    let full_rf_power = active_trx * carrier.rf_power_per_branch_watts;
                    let active_rf_power = full_rf_power * utilization;
                    let mut pa_power = active_rf_power / self.hardware.pa_efficiency;

                    // Apply Micro-Sleep gating on the unallocated symbol fraction
                    if self.active_modes.contains(&EnergySavingMode::MicroSleep) {
                        let idle_fraction = current_slot_idle_ratio.clamp(0.0, 1.0);
                        let micro_saving =
                            idle_fraction * self.hardware.micro_sleep_quiescent_reduction;
                        pa_power *= 1.0 - micro_saving;
                    }

                    power += pa_power;
                }

                power
            }
        }
    }

    /// Advances time by `elapsed_seconds` and updates cumulative energy consumption.
    pub fn tick_seconds(
        &mut self,
        elapsed_seconds: u32,
        second_of_day: u32,
        current_slot_idle_ratio: f64,
    ) {
        let current_power = self.calculate_instantaneous_power_watts(current_slot_idle_ratio);
        let baseline_power = self.calculate_baseline_power_watts();

        let dt = elapsed_seconds as f64;
        self.cumulative_energy_consumed_joules += current_power * dt;
        self.cumulative_baseline_energy_joules += baseline_power * dt;

        self.last_tick_timestamp += elapsed_seconds as u64;

        // Check scheduled carrier sleep rules
        self.evaluate_scheduled_rules(second_of_day);
    }

    fn evaluate_scheduled_rules(&mut self, second_of_day: u32) {
        let mut should_sleep_carriers = HashSet::new();
        let mut emergency_wake_needed = false;

        // Check primary carrier load
        let primary_load = self
            .carriers
            .values()
            .find(|c| c.is_primary)
            .map(|c| c.prb_utilization_percent)
            .unwrap_or(0.0);

        for sched in &self.schedules {
            if sched.is_time_in_window(second_of_day) {
                if primary_load > sched.emergency_wake_prb_threshold {
                    // Traffic overload: trigger emergency guardrail wakeup
                    emergency_wake_needed = true;
                } else {
                    for &cid in &sched.target_carrier_ids {
                        should_sleep_carriers.insert(cid);
                    }
                }
            }
        }

        if emergency_wake_needed {
            // Wake all carriers
            for carrier in self.carriers.values_mut() {
                if !carrier.is_primary && !carrier.is_active {
                    carrier.is_active = true;
                    carrier.active_trx_count = carrier.max_trx_count;
                }
            }
            self.active_modes.remove(&EnergySavingMode::CarrierSleep);
            if self.active_modes.is_empty() {
                self.set_state(
                    EnergySavingState::Active,
                    "Emergency traffic guardrail wakeup".to_string(),
                );
            }
        } else if !should_sleep_carriers.is_empty() {
            // Apply Carrier Sleep to designated carriers
            for &cid in &should_sleep_carriers {
                if let Some(carrier) = self.carriers.get_mut(&cid) {
                    if !carrier.is_primary && carrier.is_active {
                        carrier.is_active = false;
                        carrier.active_trx_count = 0;
                    }
                }
            }
            self.active_modes.insert(EnergySavingMode::CarrierSleep);
            self.set_state(
                EnergySavingState::Sleep,
                "Scheduled Carrier Sleep engaged".to_string(),
            );
        }
    }

    fn set_state(&mut self, new_state: EnergySavingState, message: String) {
        if self.state != new_state {
            let prev = self.state;
            self.state = new_state;
            let current_pwr = self.calculate_instantaneous_power_watts(0.0);
            self.event_log.push(EnergySavingEvent {
                timestamp_seconds: self.last_tick_timestamp,
                previous_state: prev,
                new_state,
                active_modes: self.active_modes.iter().copied().collect(),
                current_power_watts: current_pwr,
                message,
            });
        }
    }

    // =======================================================================
    // 6. O-RAN M-Plane RPC Handlers
    // =======================================================================

    /// RPC: `activate-energy-saving` (o-ran-energy-saving.yang §7.1).
    pub fn rpc_activate_energy_saving(
        &mut self,
        mode: EnergySavingMode,
        target_carrier_id: Option<u8>,
        target_trx_branches: Option<u16>,
    ) -> Result<String, String> {
        match mode {
            EnergySavingMode::MicroSleep => {
                self.active_modes.insert(EnergySavingMode::MicroSleep);
                self.micro_sleep.is_enabled = true;
                self.set_state(
                    EnergySavingState::Sleep,
                    "Micro-Sleep activated".to_string(),
                );
                Ok("Micro-Sleep activated successfully".to_string())
            }
            EnergySavingMode::TxArraySleep => {
                let target_trx = target_trx_branches.unwrap_or(32);
                if target_trx == 0 || target_trx > self.hardware.total_installed_trx {
                    return Err(format!("Invalid target TRX count: {}", target_trx));
                }
                for carrier in self.carriers.values_mut() {
                    carrier.active_trx_count = target_trx;
                }
                self.active_modes.insert(EnergySavingMode::TxArraySleep);
                self.set_state(
                    EnergySavingState::Sleep,
                    format!("Tx Array Sleep engaged: reduced to {} TRX", target_trx),
                );
                Ok(format!(
                    "Tx Array Sleep active with {} TRX branches",
                    target_trx
                ))
            }
            EnergySavingMode::CarrierSleep => {
                let cid = target_carrier_id
                    .ok_or_else(|| "Carrier ID required for CarrierSleep".to_string())?;
                let carrier = self
                    .carriers
                    .get_mut(&cid)
                    .ok_or_else(|| format!("Carrier {} not found", cid))?;
                if carrier.is_primary {
                    return Err("Cannot sleep primary coverage carrier".to_string());
                }
                carrier.is_active = false;
                carrier.active_trx_count = 0;
                self.active_modes.insert(EnergySavingMode::CarrierSleep);
                self.set_state(
                    EnergySavingState::Sleep,
                    format!("Carrier {} deactivated into Carrier Sleep", cid),
                );
                Ok(format!("Carrier {} entered Carrier Sleep", cid))
            }
            EnergySavingMode::DeepSleep => {
                for carrier in self.carriers.values_mut() {
                    carrier.is_active = false;
                    carrier.active_trx_count = 0;
                }
                self.active_modes.insert(EnergySavingMode::DeepSleep);
                self.set_state(
                    EnergySavingState::DeepSleep,
                    "O-RU entering Deep Sleep hibernation".to_string(),
                );
                Ok("O-RU entered Deep Sleep".to_string())
            }
        }
    }

    /// RPC: `deactivate-energy-saving` (o-ran-energy-saving.yang §7.2).
    pub fn rpc_deactivate_energy_saving(
        &mut self,
        mode: Option<EnergySavingMode>,
    ) -> Result<String, String> {
        // Fast warmup check
        if self.last_wakeup_latency_ms > self.max_acceptable_wakeup_latency_ms {
            return Err(format!(
                "Wakeup latency {}ms exceeded limit of {}ms",
                self.last_wakeup_latency_ms, self.max_acceptable_wakeup_latency_ms
            ));
        }

        match mode {
            Some(EnergySavingMode::MicroSleep) => {
                self.active_modes.remove(&EnergySavingMode::MicroSleep);
                self.micro_sleep.is_enabled = false;
            }
            Some(EnergySavingMode::TxArraySleep) => {
                self.active_modes.remove(&EnergySavingMode::TxArraySleep);
                for carrier in self.carriers.values_mut() {
                    carrier.active_trx_count = carrier.max_trx_count;
                }
            }
            Some(EnergySavingMode::CarrierSleep) => {
                self.active_modes.remove(&EnergySavingMode::CarrierSleep);
                for carrier in self.carriers.values_mut() {
                    carrier.is_active = true;
                    carrier.active_trx_count = carrier.max_trx_count;
                }
            }
            Some(EnergySavingMode::DeepSleep) | None => {
                // Restore everything to full active
                self.active_modes.clear();
                self.micro_sleep.is_enabled = true;
                for carrier in self.carriers.values_mut() {
                    carrier.is_active = true;
                    carrier.active_trx_count = carrier.max_trx_count;
                }
            }
        }

        if self.active_modes.is_empty() {
            self.set_state(
                EnergySavingState::Active,
                "All energy saving modes deactivated".to_string(),
            );
        }

        Ok("O-RU restored to Active operation".to_string())
    }

    /// Generates cumulative energy consumption and savings metrics report.
    pub fn generate_report(&self) -> EnergyConsumptionReport {
        let p_inst = self.calculate_instantaneous_power_watts(0.0);
        let p_base = self.calculate_baseline_power_watts();
        let savings_pct = if p_base > 0.0 {
            ((p_base - p_inst) / p_base) * 100.0
        } else {
            0.0
        };

        let kwh_consumed = self.cumulative_energy_consumed_joules / 3_600_000.0;
        let kwh_baseline = self.cumulative_baseline_energy_joules / 3_600_000.0;
        let kwh_saved = (kwh_baseline - kwh_consumed).max(0.0);

        let cost_per_kwh = 0.15; // $0.15 / kWh typical commercial power
        let co2_kg_per_kwh = 0.42; // 0.42 kg CO2 / kWh average grid emission

        EnergyConsumptionReport {
            instantaneous_power_watts: p_inst,
            baseline_active_power_watts: p_base,
            power_savings_percent: savings_pct,
            cumulative_energy_consumed_kwh: kwh_consumed,
            cumulative_energy_saved_kwh: kwh_saved,
            cumulative_cost_saved_usd: kwh_saved * cost_per_kwh,
            estimated_co2_saved_kg: kwh_saved * co2_kg_per_kwh,
        }
    }

    /// Pure Rust RFC 7950 XML serialization of `<energy-saving-status>`.
    pub fn serialize_status_xml(&self) -> String {
        let report = self.generate_report();
        let state_str = match self.state {
            EnergySavingState::Active => "ACTIVE",
            EnergySavingState::Sleep => "SLEEP",
            EnergySavingState::DeepSleep => "DEEP_SLEEP",
            EnergySavingState::TransitioningToSleep => "TRANSITIONING_TO_SLEEP",
            EnergySavingState::WakingUp => "WAKING_UP",
        };

        let mut modes_xml = String::new();
        for mode in &self.active_modes {
            let m_str = match mode {
                EnergySavingMode::MicroSleep => "MICRO_SLEEP",
                EnergySavingMode::TxArraySleep => "TX_ARRAY_SLEEP",
                EnergySavingMode::CarrierSleep => "CARRIER_SLEEP",
                EnergySavingMode::DeepSleep => "DEEP_SLEEP",
            };
            modes_xml.push_str(&format!("<active-mode>{}</active-mode>", m_str));
        }

        format!(
            "<energy-saving-status xmlns=\"urn:o-ran:energy-saving:1.0\">\
               <state>{}</state>\
               <active-modes>{}</active-modes>\
               <instantaneous-power-watts>{:.1}</instantaneous-power-watts>\
               <baseline-power-watts>{:.1}</baseline-power-watts>\
               <power-savings-percent>{:.1}</power-savings-percent>\
               <cumulative-energy-saved-kwh>{:.3}</cumulative-energy-saved-kwh>\
               <estimated-co2-saved-kg>{:.2}</estimated-co2-saved-kg>\
             </energy-saving-status>",
            state_str,
            modes_xml,
            report.instantaneous_power_watts,
            report.baseline_active_power_watts,
            report.power_savings_percent,
            report.cumulative_energy_saved_kwh,
            report.estimated_co2_saved_kg
        )
    }

    /// Pure Rust RFC 7951 JSON serialization of `<energy-saving-status>`.
    pub fn serialize_status_json(&self) -> String {
        let report = self.generate_report();
        let state_str = match self.state {
            EnergySavingState::Active => "ACTIVE",
            EnergySavingState::Sleep => "SLEEP",
            EnergySavingState::DeepSleep => "DEEP_SLEEP",
            EnergySavingState::TransitioningToSleep => "TRANSITIONING_TO_SLEEP",
            EnergySavingState::WakingUp => "WAKING_UP",
        };

        let modes_json: Vec<String> = self
            .active_modes
            .iter()
            .map(|m| match m {
                EnergySavingMode::MicroSleep => "\"MICRO_SLEEP\"".to_string(),
                EnergySavingMode::TxArraySleep => "\"TX_ARRAY_SLEEP\"".to_string(),
                EnergySavingMode::CarrierSleep => "\"CARRIER_SLEEP\"".to_string(),
                EnergySavingMode::DeepSleep => "\"DEEP_SLEEP\"".to_string(),
            })
            .collect();

        format!(
            "{{\
               \"o-ran-energy-saving:energy-saving-status\":{{\
                 \"state\":\"{}\",\
                 \"active-modes\":[{}],\
                 \"instantaneous-power-watts\":{:.1},\
                 \"baseline-power-watts\":{:.1},\
                 \"power-savings-percent\":{:.1},\
                 \"cumulative-energy-saved-kwh\":{:.3},\
                 \"estimated-co2-saved-kg\":{:.2}\
               }}\
             }}",
            state_str,
            modes_json.join(","),
            report.instantaneous_power_watts,
            report.baseline_active_power_watts,
            report.power_savings_percent,
            report.cumulative_energy_saved_kwh,
            report.estimated_co2_saved_kg
        )
    }
}
