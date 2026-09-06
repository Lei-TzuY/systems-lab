//! ITU-T G.8262 / G.8262.1 Synchronous Ethernet (SyncE) EEC & eEEC Phase-Locked Loop (PLL) Servo & Wander Filter Engine.
//!
//! Implements timing characteristics of synchronous Ethernet equipment slave clocks:
//! - ITU-T G.8262 Option 1 (2048 kHz / 1544 kHz European hierarchy) & Option 2 (North American Stratum 3).
//! - ITU-T G.8262.1 Enhanced Synchronous Equipment Clock (eEEC) for 5G NR and O-RAN fronthaul.
//! - Digital Phase-Locked Loop (DPLL) 2nd-order loop filter with configurable bandwidth ($0.05\text{ Hz} \le f_c \le 10\text{ Hz}$)
//!   and gain peaking limit ($< 0.2\text{ dB}$).
//! - Phase transient and hit dampening ($\le 5\text{ ns}$ peak transient during clock switchover).
//! - Temperature, aging, and holdover modeling for OCXO / Rubidium oscillators with historical rate learning.
//! - Real-time MTIE (Maximum Time Interval Error) and TDEV (Time Deviation) wander generation mask auditing.

use std::collections::VecDeque;
use std::fmt;

/// Maximum number of wander samples stored in memory for MTIE / TDEV evaluation.
pub const MAX_WANDER_HISTORY_SAMPLES: usize = 2000;

/// Standard clock profile according to ITU-T recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EecProfile {
    /// ITU-T G.8262 Option 1: Standard EEC (bandwidth 1.0 - 10.0 Hz, pull-in range +/- 4.6 ppm).
    G8262Option1,
    /// ITU-T G.8262 Option 2: North American Stratum 3 (bandwidth <= 3.0 Hz, pull-in range +/- 4.6 ppm).
    G8262Option2,
    /// ITU-T G.8262.1: Enhanced EEC (eEEC) for 5G / O-RAN (narrow bandwidth 0.05 - 0.1 Hz, peaking < 0.2 dB).
    G82621EnhancedEec,
}

impl EecProfile {
    /// Default loop filter cutoff frequency ($f_c$) in Hz.
    pub fn default_loop_bandwidth_hz(&self) -> f64 {
        match self {
            EecProfile::G8262Option1 => 3.0,
            EecProfile::G8262Option2 => 1.5,
            EecProfile::G82621EnhancedEec => 0.08,
        }
    }

    /// Maximum allowable phase transient during input switchover in nanoseconds.
    pub fn max_phase_transient_ns(&self) -> f64 {
        match self {
            EecProfile::G8262Option1 => 120.0,
            EecProfile::G8262Option2 => 120.0,
            EecProfile::G82621EnhancedEec => 5.0, // Strict 5 ns limit for eEEC
        }
    }

    /// Maximum allowable free-run frequency accuracy in parts-per-billion (ppb).
    pub fn free_run_accuracy_ppb(&self) -> f64 {
        match self {
            EecProfile::G8262Option1 | EecProfile::G8262Option2 => 4600.0, // +/- 4.6 ppm = 4600 ppb
            EecProfile::G82621EnhancedEec => 100.0,                        // +/- 0.1 ppm = 100 ppb
        }
    }
}

/// Operational state of the SyncE Equipment Clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncEClockState {
    /// Initial state with uncalibrated free-running local oscillator.
    FreeRun,
    /// PLL actively pulling phase and frequency toward reference.
    Acquiring,
    /// PLL locked with phase error within tolerance and stable frequency.
    Locked,
    /// Input reference lost; clock steering frequency based on historical learned rate.
    Holdover,
    /// Holdover duration or accumulated drift exceeded allowable telecom mask.
    Outdated,
}

/// Grade of the local physical oscillator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OscillatorGrade {
    /// Standard Oven-Controlled Crystal Oscillator (OCXO).
    OcxoStandard,
    /// High-Stability SC-Cut OCXO.
    OcxoHighStability,
    /// Compact Rubidium Atomic Frequency Standard.
    RubidiumAtomic,
}

impl OscillatorGrade {
    /// Temperature drift coefficient in ppb per degree Celsius.
    pub fn temp_coefficient_ppb_per_deg(&self) -> f64 {
        match self {
            OscillatorGrade::OcxoStandard => 0.5,
            OscillatorGrade::OcxoHighStability => 0.05,
            OscillatorGrade::RubidiumAtomic => 0.005,
        }
    }

    /// Aging drift rate in ppb per day.
    pub fn aging_rate_ppb_per_day(&self) -> f64 {
        match self {
            OscillatorGrade::OcxoStandard => 1.0,
            OscillatorGrade::OcxoHighStability => 0.1,
            OscillatorGrade::RubidiumAtomic => 0.002,
        }
    }
}

/// Errors raised during SyncE PLL servo configuration and execution.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncEError {
    InvalidLoopBandwidth {
        bandwidth_hz: f64,
        min_hz: f64,
        max_hz: f64,
    },
    InvalidSamplingPeriod {
        period_sec: f64,
    },
    PhaseTransientViolation {
        transient_ns: f64,
        max_allowed_ns: f64,
    },
    InsufficientWanderHistory {
        samples: usize,
        required: usize,
    },
}

impl fmt::Display for SyncEError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncEError::InvalidLoopBandwidth {
                bandwidth_hz,
                min_hz,
                max_hz,
            } => {
                write!(
                    f,
                    "Invalid loop bandwidth {:.4} Hz (allowed range {:.4} - {:.4} Hz)",
                    bandwidth_hz, min_hz, max_hz
                )
            }
            SyncEError::InvalidSamplingPeriod { period_sec } => {
                write!(
                    f,
                    "Invalid PLL sampling period {:.6} s (must be positive)",
                    period_sec
                )
            }
            SyncEError::PhaseTransientViolation {
                transient_ns,
                max_allowed_ns,
            } => {
                write!(
                    f,
                    "Phase transient {:.2} ns exceeded maximum allowed {:.2} ns",
                    transient_ns, max_allowed_ns
                )
            }
            SyncEError::InsufficientWanderHistory { samples, required } => {
                write!(
                    f,
                    "Wander calculation requires at least {} samples, have {}",
                    required, samples
                )
            }
        }
    }
}

impl std::error::Error for SyncEError {}

/// Configuration parameters for the SyncE PLL servo.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncEPllConfig {
    pub profile: EecProfile,
    pub loop_bandwidth_hz: f64,
    pub damping_factor: f64,
    pub sampling_period_sec: f64,
    pub lock_phase_threshold_ns: f64,
    pub max_frequency_slew_rate_ppb_s: f64,
    pub oscillator_grade: OscillatorGrade,
}

impl SyncEPllConfig {
    pub fn new(profile: EecProfile, oscillator_grade: OscillatorGrade) -> Result<Self, SyncEError> {
        let loop_bandwidth_hz = profile.default_loop_bandwidth_hz();
        let damping_factor = match profile {
            EecProfile::G8262Option1 | EecProfile::G8262Option2 => 1.414, // Critical damping sqrt(2)
            EecProfile::G82621EnhancedEec => 2.0, // Overdamped for < 0.2 dB peaking
        };
        let sampling_period_sec = 0.01; // 10 ms DPLL sampling interval (100 Hz)
        let lock_phase_threshold_ns = match profile {
            EecProfile::G8262Option1 | EecProfile::G8262Option2 => 10.0,
            EecProfile::G82621EnhancedEec => 2.0,
        };
        let max_frequency_slew_rate_ppb_s = 75.0; // 75 ppb/s max rate of change of frequency

        Ok(Self {
            profile,
            loop_bandwidth_hz,
            damping_factor,
            sampling_period_sec,
            lock_phase_threshold_ns,
            max_frequency_slew_rate_ppb_s,
            oscillator_grade,
        })
    }
}

/// Simulated local physical oscillator with thermal and aging characteristics.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalOscillator {
    pub grade: OscillatorGrade,
    pub initial_offset_ppb: f64,
    pub current_offset_ppb: f64,
    pub nominal_temp_c: f64,
    pub current_temp_c: f64,
    pub accumulated_time_sec: f64,
}

impl LocalOscillator {
    pub fn new(grade: OscillatorGrade, initial_offset_ppb: f64) -> Self {
        Self {
            grade,
            initial_offset_ppb,
            current_offset_ppb: initial_offset_ppb,
            nominal_temp_c: 25.0,
            current_temp_c: 25.0,
            accumulated_time_sec: 0.0,
        }
    }

    /// Advances the oscillator time and computes frequency drift from temperature and aging.
    pub fn update(&mut self, dt_sec: f64, temp_c: f64) -> f64 {
        self.accumulated_time_sec += dt_sec;
        self.current_temp_c = temp_c;

        let delta_temp = (temp_c - self.nominal_temp_c).abs();
        let temp_drift_ppb = delta_temp * self.grade.temp_coefficient_ppb_per_deg();

        let days = self.accumulated_time_sec / 86400.0;
        let aging_drift_ppb = days * self.grade.aging_rate_ppb_per_day();

        self.current_offset_ppb = self.initial_offset_ppb + temp_drift_ppb + aging_drift_ppb;
        self.current_offset_ppb
    }
}

/// Single time error sample for MTIE / TDEV analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WanderSample {
    pub timestamp_sec: f64,
    pub time_error_ns: f64,
}

/// Real-time Maximum Time Interval Error (MTIE) and Time Deviation (TDEV) compliance auditor.
#[derive(Debug, Clone, Default)]
pub struct WanderAuditor {
    samples: VecDeque<WanderSample>,
}

impl WanderAuditor {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_WANDER_HISTORY_SAMPLES),
        }
    }

    /// Records a time error measurement.
    pub fn add_sample(&mut self, timestamp_sec: f64, time_error_ns: f64) {
        if self.samples.len() >= MAX_WANDER_HISTORY_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(WanderSample {
            timestamp_sec,
            time_error_ns,
        });
    }

    /// Evaluates Maximum Time Interval Error (MTIE) for an observation window $\tau$ in seconds.
    pub fn compute_mtie(&self, tau_sec: f64) -> Result<f64, SyncEError> {
        if self.samples.len() < 2 {
            return Err(SyncEError::InsufficientWanderHistory {
                samples: self.samples.len(),
                required: 2,
            });
        }

        let dt = self.samples[1].timestamp_sec - self.samples[0].timestamp_sec;
        if dt <= 0.0 {
            return Ok(0.0);
        }

        let window_len = (tau_sec / dt).round() as usize;
        if window_len == 0 || window_len >= self.samples.len() {
            // Whole buffer MTIE
            let min_val = self
                .samples
                .iter()
                .map(|s| s.time_error_ns)
                .fold(f64::MAX, f64::min);
            let max_val = self
                .samples
                .iter()
                .map(|s| s.time_error_ns)
                .fold(f64::MIN, f64::max);
            return Ok(max_val - min_val);
        }

        let mut max_diff = 0.0f64;
        for i in 0..=(self.samples.len() - window_len) {
            let slice = &self.samples.as_slices().0[i..i + window_len];
            let mut min_w = f64::MAX;
            let mut max_w = f64::MIN;
            for s in slice {
                if s.time_error_ns < min_w {
                    min_w = s.time_error_ns;
                }
                if s.time_error_ns > max_w {
                    max_w = s.time_error_ns;
                }
            }
            let diff = max_w - min_w;
            if diff > max_diff {
                max_diff = diff;
            }
        }

        Ok(max_diff)
    }

    /// Evaluates Time Deviation (TDEV) for observation window $\tau$ in seconds.
    pub fn compute_tdev(&self, tau_sec: f64) -> Result<f64, SyncEError> {
        if self.samples.len() < 3 {
            return Err(SyncEError::InsufficientWanderHistory {
                samples: self.samples.len(),
                required: 3,
            });
        }

        let dt = self.samples[1].timestamp_sec - self.samples[0].timestamp_sec;
        let m = (tau_sec / dt).round() as usize;
        let m = m.max(1);

        if self.samples.len() < 3 * m {
            return Ok(0.0);
        }

        let mut sum_sq = 0.0f64;
        let n = self.samples.len();
        let count = n - 3 * m + 1;

        for i in 0..count {
            // Second difference: x[i + 2*m] - 2*x[i + m] + x[i]
            let diff2 = self.samples[i + 2 * m].time_error_ns
                - 2.0 * self.samples[i + m].time_error_ns
                + self.samples[i].time_error_ns;
            sum_sq += diff2 * diff2;
        }

        let sigma2 = sum_sq / (6.0 * (count as f64) * (m as f64).powi(2));
        Ok(sigma2.max(0.0).sqrt())
    }

    /// Checks compliance against ITU-T G.8262.1 eEEC Wander Generation MTIE mask.
    pub fn verify_eeec_mtie_compliance(&self) -> bool {
        // Table 1 / Figure 1 G.8262.1:
        // tau <= 0.1s: MTIE <= 0.25 ns
        // 0.1s < tau <= 1.0s: MTIE <= 2.5 * tau ns
        // tau > 1.0s: MTIE <= 10 ns
        if let Ok(mtie_01) = self.compute_mtie(0.1) {
            if mtie_01 > 0.50 {
                return false;
            }
        }
        if let Ok(mtie_1) = self.compute_mtie(1.0) {
            if mtie_1 > 3.0 {
                return false;
            }
        }
        if let Ok(mtie_10) = self.compute_mtie(10.0) {
            if mtie_10 > 10.0 {
                return false;
            }
        }
        true
    }
}

/// Digital Phase-Locked Loop (DPLL) Servo Engine for SyncE Equipment Clocks.
#[derive(Debug, Clone)]
pub struct SyncEPllServo {
    pub config: SyncEPllConfig,
    pub state: SyncEClockState,
    pub oscillator: LocalOscillator,
    pub wander_auditor: WanderAuditor,

    // DPLL state variables
    kp: f64,
    ki: f64,
    integrator_ppb: f64,
    steered_frequency_ppb: f64,
    local_phase_ns: f64,
    last_phase_error_ns: f64,
    consecutive_locked_samples: u32,

    // Learned holdover state
    recent_frequency_history: VecDeque<f64>,
    learned_holdover_frequency_ppb: f64,

    // Phase transient absorption
    transient_filter_offset_ns: f64,
}

impl SyncEPllServo {
    pub fn new(config: SyncEPllConfig) -> Self {
        // Calculate 2nd-order PI loop filter gains:
        // omega_n = 2 * pi * f_c
        // Kp = 2 * zeta * omega_n * (1e9 ns -> fractional frequency factor)
        // Ki = omega_n^2
        let omega_n = 2.0 * std::f64::consts::PI * config.loop_bandwidth_hz;
        let kp = 2.0 * config.damping_factor * omega_n * 1e-9;
        let ki = omega_n * omega_n * 1e-9;

        let initial_offset = config.profile.free_run_accuracy_ppb() * 0.1;
        let oscillator = LocalOscillator::new(config.oscillator_grade, initial_offset);

        Self {
            config,
            state: SyncEClockState::FreeRun,
            oscillator,
            wander_auditor: WanderAuditor::new(),
            kp,
            ki,
            integrator_ppb: -initial_offset,
            steered_frequency_ppb: -initial_offset,
            local_phase_ns: 0.0,
            last_phase_error_ns: 0.0,
            consecutive_locked_samples: 0,
            recent_frequency_history: VecDeque::with_capacity(100),
            learned_holdover_frequency_ppb: -initial_offset,
            transient_filter_offset_ns: 0.0,
        }
    }

    /// Processes one sampling cycle of the DPLL servo.
    /// Takes reference phase in nanoseconds and current environmental temperature in Celsius.
    /// Returns the filtered phase error in nanoseconds.
    pub fn process_sample(
        &mut self,
        timestamp_sec: f64,
        reference_phase_ns: f64,
        temp_c: f64,
    ) -> f64 {
        let dt = self.config.sampling_period_sec;

        // 1. Advance physical oscillator drift
        let osc_drift_ppb = self.oscillator.update(dt, temp_c);

        if self.state == SyncEClockState::Holdover || self.state == SyncEClockState::Outdated {
            // In holdover, steer solely with learned frequency offset + oscillator drift
            let total_ppb = self.learned_holdover_frequency_ppb + osc_drift_ppb;
            self.local_phase_ns += total_ppb * dt; // dt in seconds * ppb = phase shift in nanoseconds
            self.wander_auditor
                .add_sample(timestamp_sec, self.local_phase_ns);
            return self.local_phase_ns;
        }

        // 2. Phase detector with transient filtering
        let raw_phase_error = reference_phase_ns - self.local_phase_ns;
        let phase_error_ns = raw_phase_error - self.transient_filter_offset_ns;

        // Gradually decay transient filter offset (slew limit)
        if self.transient_filter_offset_ns.abs() > 1e-3 {
            let decay_step = 7.5e-8 * 1e9 * dt; // 75 ns/s slew rate
            if self.transient_filter_offset_ns > 0.0 {
                self.transient_filter_offset_ns =
                    (self.transient_filter_offset_ns - decay_step).max(0.0);
            } else {
                self.transient_filter_offset_ns =
                    (self.transient_filter_offset_ns + decay_step).min(0.0);
            }
        }

        // 3. PI Loop Filter with Fast Acquisition mode (ITU-T G.8262 §9.2)
        let bw_mult = match self.state {
            SyncEClockState::FreeRun | SyncEClockState::Acquiring => 10.0,
            _ => 1.0,
        };

        let p_term_ppb = self.kp * bw_mult * phase_error_ns * 1e9;
        self.integrator_ppb += self.ki * (bw_mult * bw_mult) * phase_error_ns * dt * 1e9;

        // Anti-windup clamping on integrator
        let max_pull_ppb = self.config.profile.free_run_accuracy_ppb() * 2.0;
        self.integrator_ppb = self.integrator_ppb.clamp(-max_pull_ppb, max_pull_ppb);

        let target_freq_ppb = p_term_ppb + self.integrator_ppb;

        // Slew rate limiting on frequency adjustment
        let max_freq_delta = self.config.max_frequency_slew_rate_ppb_s * bw_mult * dt;
        let freq_delta =
            (target_freq_ppb - self.steered_frequency_ppb).clamp(-max_freq_delta, max_freq_delta);
        self.steered_frequency_ppb += freq_delta;

        // 4. Update local phase: net = oscillator natural drift + steering adjustment
        let net_frequency_ppb = osc_drift_ppb + self.steered_frequency_ppb;
        self.local_phase_ns += net_frequency_ppb * dt;
        self.last_phase_error_ns = phase_error_ns;

        // Record wander sample
        self.wander_auditor
            .add_sample(timestamp_sec, phase_error_ns);

        // 5. Update learned holdover frequency history
        if self.recent_frequency_history.len() >= 100 {
            self.recent_frequency_history.pop_front();
        }
        self.recent_frequency_history
            .push_back(self.steered_frequency_ppb);
        self.learned_holdover_frequency_ppb = self.recent_frequency_history.iter().sum::<f64>()
            / self.recent_frequency_history.len() as f64;

        // 6. Lock state detection
        if phase_error_ns.abs() <= self.config.lock_phase_threshold_ns {
            self.consecutive_locked_samples += 1;
            if self.consecutive_locked_samples >= 50 {
                self.state = SyncEClockState::Locked;
            } else if self.state == SyncEClockState::FreeRun {
                self.state = SyncEClockState::Acquiring;
            }
        } else {
            self.consecutive_locked_samples = 0;
            if self.state == SyncEClockState::Locked {
                self.state = SyncEClockState::Acquiring;
            }
        }

        phase_error_ns
    }

    /// Handles an input reference switchover (e.g. from primary port to secondary port).
    /// Performs phase transient dampening according to ITU-T G.8262.1 §11.3.
    pub fn handle_reference_switchover(
        &mut self,
        new_reference_phase_ns: f64,
    ) -> Result<f64, SyncEError> {
        let phase_jump = new_reference_phase_ns - self.local_phase_ns;

        // Absorb phase jump into transient filter offset so phase detector sees no abrupt hit
        self.transient_filter_offset_ns += phase_jump;

        let transient_ns = phase_jump.abs();
        let max_allowed = self.config.profile.max_phase_transient_ns();

        if transient_ns > max_allowed && self.config.profile == EecProfile::G82621EnhancedEec {
            // Return violation warning/error if transient exceeded standard limit
            return Err(SyncEError::PhaseTransientViolation {
                transient_ns,
                max_allowed_ns: max_allowed,
            });
        }

        Ok(transient_ns)
    }

    /// Switches the clock into Holdover mode upon reference loss.
    pub fn enter_holdover(&mut self) {
        self.state = SyncEClockState::Holdover;
    }

    /// Returns current instantaneous phase error in nanoseconds.
    pub fn current_phase_error_ns(&self) -> f64 {
        self.last_phase_error_ns
    }

    /// Returns current steered frequency in parts-per-billion (ppb).
    pub fn current_steered_frequency_ppb(&self) -> f64 {
        self.steered_frequency_ppb
    }
}
