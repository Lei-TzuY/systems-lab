//! ITU-T G.8273.2 Class D Enhanced Telecom Boundary Clock (T-BC) Engine.
//!
//! Compliant with:
//! - ITU-T G.8273.2 (2020/2022) Table 1 ("Time error performance for Telecom Boundary Clock")
//! - ITU-T G.8275.1 Precision Time Protocol telecom profile for phase/time synchronization
//! - IEEE 1588-2019 sub-nanosecond High-Accuracy (White Rabbit / Class D) timing
//!
//! Specifications for Class D:
//! - Maximum absolute time error: max|TE| <= 5.0 ns (5000 ps)
//! - Constant time error: |cTE| <= 3.0 ns (3000 ps)
//! - Dynamic time error: dTE_L <= 2.0 ns (2000 ps)
//!
//! Features:
//! 1. Picosecond-accurate sub-nanosecond timestamping and phase error computation.
//! 2. Dynamic fiber chromatic dispersion and thermal asymmetry compensation (~40 ps/km/°C).
//! 3. Closed-loop PI/PID Class D Phase Servo with parts-per-trillion (ppt) frequency steering.
//! 4. Real-time decomposition of TE(t) into constant time error (cTE) and dynamic time error (dTE).
//! 5. Conformance auditing against Class A, B, C, and D specifications.
//! 6. Holdover time error drift prediction during upstream reference loss.

use std::collections::VecDeque;

pub const PICOSECONDS_PER_NANOSECOND: i64 = 1_000;
pub const PICOSECONDS_PER_SECOND: i64 = 1_000_000_000_000;

// ITU-T G.8273.2 Table 1 Time Error Limits (in Picoseconds)
pub const CLASS_D_MAX_TE_PS: i64 = 5_000; // 5.0 ns
pub const CLASS_D_MAX_CTE_PS: i64 = 3_000; // 3.0 ns
pub const CLASS_D_MAX_DTE_PS: i64 = 2_000; // 2.0 ns

pub const CLASS_C_MAX_TE_PS: i64 = 30_000; // 30.0 ns
pub const CLASS_C_MAX_CTE_PS: i64 = 10_000; // 10.0 ns
pub const CLASS_C_MAX_DTE_PS: i64 = 10_000; // 10.0 ns

pub const CLASS_B_MAX_TE_PS: i64 = 70_000; // 70.0 ns
pub const CLASS_A_MAX_TE_PS: i64 = 100_000; // 100.0 ns

// ---------------------------------------------------------------------------
// Enums & Structs
// ---------------------------------------------------------------------------

/// Clock Quality Classification per ITU-T G.8273.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PtpClockClassTier {
    /// Class D: Ultra-precise Fronthaul / CoMP (max|TE| <= 5 ns).
    ClassD,
    /// Class C: High-grade Fronthaul (max|TE| <= 30 ns).
    ClassC,
    /// Class B: Standard Cellular (max|TE| <= 70 ns).
    ClassB,
    /// Class A: Basic Telecom (max|TE| <= 100 ns).
    ClassA,
    /// Out of Spec (> 100 ns).
    OutOfSpec,
}

/// Four-Timestamp PTP Measurement Sample in Picoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubNanoPtpSample {
    pub seq_id: u16,
    pub t1_master_tx_ps: i64,
    pub t2_slave_rx_ps: i64,
    pub t3_slave_tx_ps: i64,
    pub t4_master_rx_ps: i64,
    pub correction_field_ps: i64,
}

impl SubNanoPtpSample {
    pub fn new(
        seq_id: u16,
        t1_ps: i64,
        t2_ps: i64,
        t3_ps: i64,
        t4_ps: i64,
        correction_field_ps: i64,
    ) -> Self {
        Self {
            seq_id,
            t1_master_tx_ps: t1_ps,
            t2_slave_rx_ps: t2_ps,
            t3_slave_tx_ps: t3_ps,
            t4_master_rx_ps: t4_ps,
            correction_field_ps,
        }
    }

    /// Computes raw mean path delay in picoseconds:
    /// MeanPathDelay = [ (t2 - t1) + (t4 - t3) - correctionField ] / 2
    #[inline]
    pub fn raw_mean_path_delay_ps(&self) -> i64 {
        let fwd = self.t2_slave_rx_ps - self.t1_master_tx_ps;
        let rev = self.t4_master_rx_ps - self.t3_slave_tx_ps;
        (fwd + rev - self.correction_field_ps) / 2
    }

    /// Computes raw phase offset in picoseconds (Master to Slave):
    /// PhaseOffset = [ (t2 - t1) - (t4 - t3) - correctionField ] / 2
    #[inline]
    pub fn raw_phase_offset_ps(&self) -> i64 {
        let fwd = self.t2_slave_rx_ps - self.t1_master_tx_ps;
        let rev = self.t4_master_rx_ps - self.t3_slave_tx_ps;
        (fwd - rev - self.correction_field_ps) / 2
    }
}

/// Dynamic Fiber Delay and Temperature Asymmetry Model.
#[derive(Debug, Clone, PartialEq)]
pub struct FiberAsymmetryModel {
    /// Fiber cable physical length in kilometers.
    pub fiber_length_km: f64,
    /// Forward wavelength in nanometers (e.g. 1310.0 nm).
    pub lambda_fwd_nm: f64,
    /// Reverse wavelength in nanometers (e.g. 1490.0 nm).
    pub lambda_rev_nm: f64,
    /// Thermal delay sensitivity in ps / (km * °C). Typically 35.0 to 45.0 ps/km/°C.
    pub thermal_coeff_ps_per_km_deg: f64,
    /// Reference temperature in degrees Celsius.
    pub reference_temp_deg: f64,
    /// Current measured temperature in degrees Celsius.
    pub current_temp_deg: f64,
    /// Chromatic dispersion parameter in ps / (nm * km) (typically ~17 ps/nm*km around 1550 nm, ~3 around 1310 nm).
    pub dispersion_coeff_ps_per_nm_km: f64,
}

impl FiberAsymmetryModel {
    pub fn new(fiber_length_km: f64, lambda_fwd_nm: f64, lambda_rev_nm: f64) -> Self {
        Self {
            fiber_length_km,
            lambda_fwd_nm,
            lambda_rev_nm,
            thermal_coeff_ps_per_km_deg: 40.0,
            reference_temp_deg: 25.0,
            current_temp_deg: 25.0,
            dispersion_coeff_ps_per_nm_km: 3.5, // Standard G.652 fiber at 1310/1490 nm
        }
    }

    /// Calculates total fiber asymmetry correction in picoseconds:
    /// Asymmetry = Asymmetry_chromatic + Asymmetry_thermal
    pub fn compute_asymmetry_ps(&self) -> i64 {
        // 1. Chromatic dispersion asymmetry due to wavelength difference:
        // Delta_tau_chrom = D * (lambda_rev - lambda_fwd) * L
        let delta_lambda = self.lambda_rev_nm - self.lambda_fwd_nm;
        let chromatic_asym_ps =
            self.dispersion_coeff_ps_per_nm_km * delta_lambda * self.fiber_length_km;

        // 2. Thermal drift delay variation:
        // Delta_tau_thermal = k_thermal * (T_current - T_ref) * L
        let delta_temp = self.current_temp_deg - self.reference_temp_deg;
        let thermal_drift_ps = self.thermal_coeff_ps_per_km_deg * delta_temp * self.fiber_length_km;

        (chromatic_asym_ps + thermal_drift_ps).round() as i64
    }
}

// ---------------------------------------------------------------------------
// Class D Phase Servo & Loop Filter
// ---------------------------------------------------------------------------

/// Sub-Nanosecond Proportional-Integral (PI) Phase Servo for Class D T-BC.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDPhaseServo {
    /// Proportional gain Kp.
    pub kp: f64,
    /// Integral gain Ki.
    pub ki: f64,
    /// Integrator state in parts-per-trillion (ppt).
    pub integrator_ppt: f64,
    /// Current frequency steering adjustment in parts-per-trillion (ppt).
    pub current_freq_adjust_ppt: f64,
    /// Internal Numerically Controlled Oscillator (NCO) phase offset in picoseconds.
    pub nco_phase_ps: f64,
    /// Damping low-pass filter state for jitter rejection.
    pub filtered_error_ps: f64,
}

impl ClassDPhaseServo {
    pub fn new(kp: f64, ki: f64) -> Self {
        Self {
            kp,
            ki,
            integrator_ppt: 0.0,
            current_freq_adjust_ppt: 0.0,
            nco_phase_ps: 0.0,
            filtered_error_ps: 0.0,
        }
    }

    /// Default tuning parameters optimized for ITU-T G.8273.2 Class D (< 0.1 Hz loop bandwidth).
    pub fn default_class_d() -> Self {
        Self::new(0.25, 0.02)
    }

    /// Updates the servo with a new phase error measurement over interval `dt_s`.
    /// Returns the corrected residual phase error in picoseconds.
    pub fn update(&mut self, measured_phase_error_ps: i64, dt_s: f64) -> i64 {
        // Low-pass filter the input measurement (alpha = 0.35)
        let alpha = 0.35;
        self.filtered_error_ps =
            alpha * (measured_phase_error_ps as f64) + (1.0 - alpha) * self.filtered_error_ps;

        let err = self.filtered_error_ps;

        // PI loop steering
        let p_term = self.kp * err;
        // Convert phase error to frequency adjustment: 1 ps error over 1s = 1 ppt
        self.integrator_ppt += self.ki * err * dt_s;
        // Clamp integrator to +/- 100,000 ppt (+/- 100 ppb) for stability
        self.integrator_ppt = self.integrator_ppt.clamp(-100_000.0, 100_000.0);

        self.current_freq_adjust_ppt = p_term + self.integrator_ppt;

        // Apply frequency adjustment to NCO phase:
        // delta_phase = freq_adjust (ppt) * dt_s * 1e12 ps/s * 1e-12 = freq_adjust * dt_s (ps)
        let phase_correction = self.current_freq_adjust_ppt * dt_s;
        self.nco_phase_ps = err - phase_correction;

        self.nco_phase_ps.round() as i64
    }
}

// ---------------------------------------------------------------------------
// cTE / dTE Real-Time Decomposition & Compliance
// ---------------------------------------------------------------------------

/// Decomposed Time Error metrics per ITU-T G.8273.2.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeErrorComponents {
    /// Instantaneous time error TE(t) in picoseconds.
    pub instantaneous_te_ps: i64,
    /// Constant time error cTE in picoseconds (low-frequency average).
    pub constant_te_ps: i64,
    /// Dynamic time error dTE(t) in picoseconds (high-frequency zero-mean residual).
    pub dynamic_te_ps: i64,
    /// Peak-to-peak dynamic time error in sliding window.
    pub dte_peak_to_peak_ps: i64,
    /// Determined G.8273.2 Clock Class tier.
    pub class_tier: PtpClockClassTier,
    /// Whether instantaneous and statistical parameters meet Class D limits.
    pub is_class_d_compliant: bool,
}

/// Sliding-window Filter for cTE / dTE decomposition.
#[derive(Debug, Clone)]
pub struct ClassDTimeErrorFilter {
    window_capacity: usize,
    history_te_ps: VecDeque<i64>,
    running_sum_te_ps: i64,
}

impl ClassDTimeErrorFilter {
    pub fn new(window_capacity: usize) -> Self {
        Self {
            window_capacity: window_capacity.max(10),
            history_te_ps: VecDeque::with_capacity(window_capacity),
            running_sum_te_ps: 0,
        }
    }

    /// Feeds a new time error measurement into the filter and computes cTE / dTE.
    pub fn feed(&mut self, te_ps: i64) -> TimeErrorComponents {
        if self.history_te_ps.len() == self.window_capacity {
            if let Some(old) = self.history_te_ps.pop_front() {
                self.running_sum_te_ps -= old;
            }
        }

        self.history_te_ps.push_back(te_ps);
        self.running_sum_te_ps += te_ps;

        let cte = self.running_sum_te_ps / (self.history_te_ps.len() as i64);
        let dte = te_ps - cte;

        let mut min_dte = i64::MAX;
        let mut max_dte = i64::MIN;
        for &val in &self.history_te_ps {
            let res = val - cte;
            if res < min_dte {
                min_dte = res;
            }
            if res > max_dte {
                max_dte = res;
            }
        }
        let dte_p2p = if min_dte <= max_dte {
            max_dte - min_dte
        } else {
            0
        };

        // Class Tier Determination based on max|TE|, |cTE|, and |dTE|
        let abs_te = te_ps.abs();
        let abs_cte = cte.abs();
        let abs_dte = dte.abs();

        let is_class_d = abs_te <= CLASS_D_MAX_TE_PS
            && abs_cte <= CLASS_D_MAX_CTE_PS
            && abs_dte <= CLASS_D_MAX_DTE_PS;

        let class_tier = if is_class_d {
            PtpClockClassTier::ClassD
        } else if abs_te <= CLASS_C_MAX_TE_PS
            && abs_cte <= CLASS_C_MAX_CTE_PS
            && abs_dte <= CLASS_C_MAX_DTE_PS
        {
            PtpClockClassTier::ClassC
        } else if abs_te <= CLASS_B_MAX_TE_PS {
            PtpClockClassTier::ClassB
        } else if abs_te <= CLASS_A_MAX_TE_PS {
            PtpClockClassTier::ClassA
        } else {
            PtpClockClassTier::OutOfSpec
        };

        TimeErrorComponents {
            instantaneous_te_ps: te_ps,
            constant_te_ps: cte,
            dynamic_te_ps: dte,
            dte_peak_to_peak_ps: dte_p2p,
            class_tier,
            is_class_d_compliant: is_class_d,
        }
    }
}

// ---------------------------------------------------------------------------
// Holdover Drift Predictor
// ---------------------------------------------------------------------------

/// Oscillator Holdover Aging & Thermal Drift Predictor.
#[derive(Debug, Clone, PartialEq)]
pub struct HoldoverPredictor {
    /// Oscillator aging drift in parts-per-trillion (ppt) per day.
    pub aging_ppt_per_day: f64,
    /// Temperature drift coefficient in ppt per degree Celsius.
    pub temp_coeff_ppt_per_deg: f64,
}

impl HoldoverPredictor {
    pub fn new_ocxo() -> Self {
        // High-stability Oven-Controlled Crystal Oscillator (OCXO)
        Self {
            aging_ppt_per_day: 50.0, // 0.05 ppb/day
            temp_coeff_ppt_per_deg: 20.0,
        }
    }

    /// Predicts accumulated time error in picoseconds over holdover duration in seconds.
    pub fn predict_drift_ps(&self, holdover_seconds: f64, temp_delta_deg: f64) -> i64 {
        let days = holdover_seconds / 86400.0;
        // Total fractional frequency offset (ppt):
        let freq_offset_ppt =
            self.aging_ppt_per_day * days + self.temp_coeff_ppt_per_deg * temp_delta_deg;
        // Integrated time drift: Delta_T = freq_offset * holdover_seconds (in picoseconds)
        // 1 ppt = 1e-12 -> 1 ppt * 1 sec = 1 picosecond!
        let drift_ps = freq_offset_ppt * holdover_seconds;
        drift_ps.round() as i64
    }
}

// ---------------------------------------------------------------------------
// Top-Level Class D Telecom Boundary Clock Manager
// ---------------------------------------------------------------------------

/// Telemetry metrics for Class D T-BC performance.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDTelemetry {
    pub total_samples_processed: u64,
    pub class_d_conformance_count: u64,
    pub class_d_conformance_ratio: f64,
    pub peak_te_ps: i64,
    pub current_cte_ps: i64,
    pub peak_dte_ps: i64,
    pub current_fiber_asymmetry_ps: i64,
}

impl Default for ClassDTelemetry {
    fn default() -> Self {
        Self {
            total_samples_processed: 0,
            class_d_conformance_count: 0,
            class_d_conformance_ratio: 0.0,
            peak_te_ps: 0,
            current_cte_ps: 0,
            peak_dte_ps: 0,
            current_fiber_asymmetry_ps: 0,
        }
    }
}

/// ITU-T G.8273.2 Class D Telecom Boundary Clock (T-BC) Protocol Engine.
pub struct PtpTelecomClassDManager {
    pub asymmetry_model: FiberAsymmetryModel,
    pub servo: ClassDPhaseServo,
    pub te_filter: ClassDTimeErrorFilter,
    pub holdover: HoldoverPredictor,
    pub telemetry: ClassDTelemetry,
}

impl PtpTelecomClassDManager {
    /// Creates a new Class D Telecom Boundary Clock Manager.
    pub fn new(asymmetry_model: FiberAsymmetryModel) -> Self {
        Self {
            asymmetry_model,
            servo: ClassDPhaseServo::default_class_d(),
            te_filter: ClassDTimeErrorFilter::new(100),
            holdover: HoldoverPredictor::new_ocxo(),
            telemetry: ClassDTelemetry::default(),
        }
    }

    /// Processes an incoming sub-nanosecond PTP sample, applies asymmetry correction,
    /// steers the servo, and computes ITU-T G.8273.2 time error components.
    pub fn process_ptp_sample(
        &mut self,
        sample: &SubNanoPtpSample,
        interval_s: f64,
    ) -> TimeErrorComponents {
        // 1. Calculate raw phase offset
        let raw_offset_ps = sample.raw_phase_offset_ps();

        // 2. Calculate and apply fiber asymmetry compensation (IEEE 1588: offset_corr = asym / 2)
        let asym_ps = self.asymmetry_model.compute_asymmetry_ps();
        self.telemetry.current_fiber_asymmetry_ps = asym_ps;
        let compensated_offset_ps = raw_offset_ps - (asym_ps / 2);

        // 3. Drive closed-loop Class D phase servo
        let residual_te_ps = self.servo.update(compensated_offset_ps, interval_s);

        // 4. Decompose into cTE and dTE
        let components = self.te_filter.feed(residual_te_ps);

        // 5. Update telemetry
        self.telemetry.total_samples_processed += 1;
        if components.is_class_d_compliant {
            self.telemetry.class_d_conformance_count += 1;
        }
        self.telemetry.class_d_conformance_ratio = (self.telemetry.class_d_conformance_count
            as f64)
            / (self.telemetry.total_samples_processed as f64);

        if components.instantaneous_te_ps.abs() > self.telemetry.peak_te_ps {
            self.telemetry.peak_te_ps = components.instantaneous_te_ps.abs();
        }
        if components.dynamic_te_ps.abs() > self.telemetry.peak_dte_ps {
            self.telemetry.peak_dte_ps = components.dynamic_te_ps.abs();
        }
        self.telemetry.current_cte_ps = components.constant_te_ps;

        components
    }

    /// Updates the environmental temperature of the optical link.
    pub fn set_optical_temperature(&mut self, temp_deg: f64) {
        self.asymmetry_model.current_temp_deg = temp_deg;
    }
}
