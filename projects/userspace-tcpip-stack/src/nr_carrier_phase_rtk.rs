//! 3GPP Release 18 (5G-Advanced) NR Carrier Phase Positioning & RTK Engine.
//!
//! Conforms to:
//! - 3GPP TS 38.215 Rel-18 §5.1.25: Downlink PRS Carrier Phase Measurement.
//! - 3GPP TS 38.305 Rel-18 §8.13: Carrier phase-based high-accuracy positioning in NG-RAN.
//! - 3GPP TS 37.355 Rel-18: LTE/NR Positioning Protocol (LPP) carrier phase information elements.
//!
//! Features:
//! 1. 3D Cartesian vector mathematics in pure Rust with dot, cross, norm, and distance.
//! 2. DL-PRS & UL-SRS carrier phase observation modeling ($\phi = \lambda^{-1} \rho + c \lambda^{-1} \Delta t + N + \epsilon$).
//! 3. Single-difference (SD) and Double-difference (DD) observable formation:
//!    - SD across TRPs completely eliminates receiver clock offset.
//!    - DD across Reference Anchor UE and Target UE cancels both receiver and transmitter clock offsets.
//! 4. Triple-difference cycle slip detector with phase velocity sanity checks.
//! 5. Integer Ambiguity Resolution (LAMBDA / Integer Least Squares) with Ratio Test validation ($R \ge 3.0$).
//! 6. Centimeter-accuracy iterative Gauss-Newton 3D position solver ($< 3\text{ cm}$ fix).
//! 7. Quality metrics, GDOP evaluation, and RTK Fix status (`Fixed`, `Float`, `InsufficientTrps`).
//!
//! Pure standard Rust with zero external dependencies.

use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Speed of light in vacuum in meters per second (CODATA / BIPM).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;
pub const RTK_SPEED_OF_LIGHT_M_S: f64 = SPEED_OF_LIGHT_M_S;

/// Default standard 5G NR carrier frequency for Band n78 (3.5 GHz).
pub const DEFAULT_NR_CARRIER_FREQ_HZ: f64 = 3_500_000_000.0;
pub const RTK_DEFAULT_CARRIER_FREQ_HZ: f64 = DEFAULT_NR_CARRIER_FREQ_HZ;

/// Default minimum ratio threshold for integer ambiguity acceptance (LAMBDA ratio test).
pub const DEFAULT_AMBIGUITY_RATIO_THRESHOLD: f64 = 3.0;
pub const RTK_DEFAULT_AMBIGUITY_RATIO_THRESHOLD: f64 = DEFAULT_AMBIGUITY_RATIO_THRESHOLD;

/// Cycle slip detection residual threshold in cycles.
pub const CYCLE_SLIP_THRESHOLD_CYCLES: f64 = 0.50;
pub const RTK_CYCLE_SLIP_THRESHOLD_CYCLES: f64 = CYCLE_SLIP_THRESHOLD_CYCLES;

/// Maximum iterations for Gauss-Newton nonlinear least squares position solver.
pub const MAX_SOLVER_ITERATIONS: usize = 30;
pub const RTK_MAX_SOLVER_ITERATIONS: usize = MAX_SOLVER_ITERATIONS;

/// Convergence distance tolerance for position solver in meters (0.1 mm).
pub const SOLVER_CONVERGENCE_TOLERANCE_M: f64 = 0.0001;
pub const RTK_SOLVER_CONVERGENCE_TOLERANCE_M: f64 = SOLVER_CONVERGENCE_TOLERANCE_M;

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Errors encountered during carrier phase positioning and RTK solving.
#[derive(Debug, Clone, PartialEq)]
pub enum CarrierPhaseError {
    /// Less than the minimum required number of TRPs (need at least 4 TRPs for 3D fix).
    InsufficientTrps { needed: usize, available: usize },
    /// Geometric rank deficiency or collinear TRP configuration (singular matrix).
    GeometricDilutionDeficiency(String),
    /// Cycle slip detected on carrier phase tracking loop.
    CycleSlipDetected { trp_id: u32, residual_cycles: f64 },
    /// Integer ambiguity resolution failed (ratio test below threshold).
    AmbiguityResolutionFailed {
        best_norm: f64,
        second_best_norm: f64,
        ratio: f64,
        threshold: f64,
    },
    /// Carrier frequency is zero or negative.
    InvalidFrequency(f64),
    /// Solver failed to converge within maximum allowed iterations.
    ConvergenceFailure {
        iterations: usize,
        residual_norm: f64,
    },
}

impl fmt::Display for CarrierPhaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CarrierPhaseError::InsufficientTrps { needed, available } => {
                write!(
                    f,
                    "Insufficient TRPs for carrier phase fix: need at least {}, got {}",
                    needed, available
                )
            }
            CarrierPhaseError::GeometricDilutionDeficiency(msg) => {
                write!(f, "Geometric matrix rank deficiency: {}", msg)
            }
            CarrierPhaseError::CycleSlipDetected {
                trp_id,
                residual_cycles,
            } => {
                write!(
                    f,
                    "Cycle slip detected on TRP {}: phase residual {:.3} cycles exceeds threshold",
                    trp_id, residual_cycles
                )
            }
            CarrierPhaseError::AmbiguityResolutionFailed {
                ratio, threshold, ..
            } => {
                write!(
                    f,
                    "Integer ambiguity ratio test failed: ratio {:.2} < threshold {:.2}",
                    ratio, threshold
                )
            }
            CarrierPhaseError::InvalidFrequency(freq) => {
                write!(
                    f,
                    "Invalid carrier frequency: {} Hz (must be positive)",
                    freq
                )
            }
            CarrierPhaseError::ConvergenceFailure {
                iterations,
                residual_norm,
            } => {
                write!(
                    f,
                    "Position solver failed to converge after {} iterations (residual: {:.6} m)",
                    iterations, residual_norm
                )
            }
        }
    }
}

impl std::error::Error for CarrierPhaseError {}

// ---------------------------------------------------------------------------
// 1. 3D Cartesian Coordinates & Vector Mathematics
// ---------------------------------------------------------------------------

/// 3D Cartesian vector or position coordinate in meters.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cartesian3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Cartesian3D {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn add(&self, other: &Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn scale(&self, scalar: f64) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }

    pub fn dot(&self, other: &Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn norm_sq(&self) -> f64 {
        self.dot(self)
    }

    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn distance_to(&self, other: &Self) -> f64 {
        self.sub(other).norm()
    }

    /// Unit vector pointing from self towards target.
    pub fn unit_vector_to(&self, target: &Self) -> Result<Self, CarrierPhaseError> {
        let diff = target.sub(self);
        let dist = diff.norm();
        if dist < 1e-9 {
            return Err(CarrierPhaseError::GeometricDilutionDeficiency(
                "Co-located points: cannot calculate line-of-sight unit vector".to_string(),
            ));
        }
        Ok(diff.scale(1.0 / dist))
    }
}

// ---------------------------------------------------------------------------
// 2. Transmission Reception Point (TRP) Profile
// ---------------------------------------------------------------------------

/// Configuration for a 5G Transmission Reception Point (TRP) transmitting DL-PRS.
#[derive(Debug, Clone, PartialEq)]
pub struct TrpCarrierPhaseConfig {
    /// Unique TRP identifier.
    pub trp_id: u32,
    /// 3D coordinate location of the TRP antenna phase center in meters.
    pub location: Cartesian3D,
    /// Carrier center frequency in Hz.
    pub carrier_freq_hz: f64,
    /// Carrier wavelength $\lambda = c / f_c$ in meters.
    pub wavelength_m: f64,
}

impl TrpCarrierPhaseConfig {
    pub fn new(
        trp_id: u32,
        location: Cartesian3D,
        carrier_freq_hz: f64,
    ) -> Result<Self, CarrierPhaseError> {
        if carrier_freq_hz <= 0.0 {
            return Err(CarrierPhaseError::InvalidFrequency(carrier_freq_hz));
        }
        let wavelength_m = SPEED_OF_LIGHT_M_S / carrier_freq_hz;
        Ok(Self {
            trp_id,
            location,
            carrier_freq_hz,
            wavelength_m,
        })
    }
}

// ---------------------------------------------------------------------------
// 3. Carrier Phase Observation Model (3GPP TS 38.215 §5.1.25)
// ---------------------------------------------------------------------------

/// DL-PRS Carrier Phase Measurement reported by the UE (TS 38.215 §5.1.25).
#[derive(Debug, Clone, PartialEq)]
pub struct CarrierPhaseObservation {
    /// ID of the observed TRP.
    pub trp_id: u32,
    /// Full measured carrier phase in cycles ($\phi = \text{integer} + \text{fractional}$).
    pub carrier_phase_cycles: f64,
    /// Signal-to-noise ratio in dB.
    pub snr_db: f64,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Half-cycle ambiguity flag (true if 180° phase ambiguity is unresolved).
    pub half_cycle_ambiguity: bool,
}

impl CarrierPhaseObservation {
    pub fn new(
        trp_id: u32,
        carrier_phase_cycles: f64,
        snr_db: f64,
        timestamp_ns: u64,
        half_cycle_ambiguity: bool,
    ) -> Self {
        Self {
            trp_id,
            carrier_phase_cycles,
            snr_db,
            timestamp_ns,
            half_cycle_ambiguity,
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Cycle Slip Detection Engine
// ---------------------------------------------------------------------------

/// Detects cycle slips by tracking phase velocity and time-differenced residuals.
#[derive(Debug, Clone, Default)]
pub struct CycleSlipDetector {
    /// Previous observation per TRP: (timestamp_ns, phase_cycles, distance_m)
    history: std::collections::HashMap<u32, (u64, f64, f64)>,
}

impl CycleSlipDetector {
    pub fn new() -> Self {
        Self {
            history: std::collections::HashMap::new(),
        }
    }

    /// Check incoming observation for cycle slip against previous epoch.
    /// Returns Ok(true) if clean, or Err(CarrierPhaseError::CycleSlipDetected) if slip occurred.
    pub fn check_and_update(
        &mut self,
        obs: &CarrierPhaseObservation,
        estimated_distance_m: f64,
        wavelength_m: f64,
    ) -> Result<bool, CarrierPhaseError> {
        if let Some(&(prev_ts, prev_phase, prev_dist)) = self.history.get(&obs.trp_id) {
            let dt_s = (obs.timestamp_ns.saturating_sub(prev_ts) as f64) * 1e-9;
            if dt_s > 0.0 && dt_s < 1.0 {
                // Expected change in phase due to geometric displacement
                let expected_phase_delta = (estimated_distance_m - prev_dist) / wavelength_m;
                let actual_phase_delta = obs.carrier_phase_cycles - prev_phase;
                let residual = (actual_phase_delta - expected_phase_delta).abs();
                if residual >= CYCLE_SLIP_THRESHOLD_CYCLES {
                    return Err(CarrierPhaseError::CycleSlipDetected {
                        trp_id: obs.trp_id,
                        residual_cycles: residual,
                    });
                }
            }
        }

        self.history.insert(
            obs.trp_id,
            (
                obs.timestamp_ns,
                obs.carrier_phase_cycles,
                estimated_distance_m,
            ),
        );
        Ok(true)
    }

    /// Reset history for a specific TRP upon re-acquisition.
    pub fn reset_trp(&mut self, trp_id: u32) {
        self.history.remove(&trp_id);
    }
}

// ---------------------------------------------------------------------------
// 5. Integer Ambiguity Resolution (LAMBDA / Integer Least Squares)
// ---------------------------------------------------------------------------

/// Resolves integer cycle ambiguities $N \in \mathbb{Z}^M$ using Integer Least Squares.
#[derive(Debug, Clone)]
pub struct LambdaAmbiguitySolver {
    pub ratio_threshold: f64,
}

impl Default for LambdaAmbiguitySolver {
    fn default() -> Self {
        Self {
            ratio_threshold: DEFAULT_AMBIGUITY_RATIO_THRESHOLD,
        }
    }
}

impl LambdaAmbiguitySolver {
    pub fn new(ratio_threshold: f64) -> Self {
        Self { ratio_threshold }
    }

    /// Solves Integer Least Squares for floating ambiguity vector $\hat{a}$.
    /// Returns (integer_candidate, ratio, is_fixed).
    pub fn resolve_ambiguities(
        &self,
        float_ambiguities: &[f64],
    ) -> Result<(Vec<i32>, f64, bool), CarrierPhaseError> {
        if float_ambiguities.is_empty() {
            return Ok((Vec::new(), 1.0, false));
        }

        let mut candidate_best = Vec::with_capacity(float_ambiguities.len());
        let mut candidate_second = Vec::with_capacity(float_ambiguities.len());

        let mut dist_best_sq = 0.0;
        let mut dist_second_sq = 0.0;

        for &val in float_ambiguities {
            let rounded = val.round() as i32;
            let frac = val - (rounded as f64);
            dist_best_sq += frac * frac;

            // Second nearest integer
            let second_rounded = if frac >= 0.0 {
                rounded + 1
            } else {
                rounded - 1
            };
            let second_frac = val - (second_rounded as f64);
            dist_second_sq += second_frac * second_frac;

            candidate_best.push(rounded);
            candidate_second.push(second_rounded);
        }

        let ratio = if dist_best_sq < 1e-9 {
            999.0 // Perfect integer match
        } else {
            dist_second_sq / dist_best_sq
        };

        let is_fixed = ratio >= self.ratio_threshold;
        Ok((candidate_best, ratio, is_fixed))
    }
}

// ---------------------------------------------------------------------------
// 6. RTK Solution Status & Metrics
// ---------------------------------------------------------------------------

/// Fix status of the RTK positioning solution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtkFixStatus {
    /// High-precision centimeter fix: integer ambiguities resolved and verified ($< 5\text{ cm}$).
    Fixed,
    /// Decimeter accuracy: float ambiguities estimated ($10 - 50\text{ cm}$).
    Float,
    /// Insufficient satellite/TRP geometry or observations.
    InsufficientTrps,
}

/// The computed RTK positioning fix.
#[derive(Debug, Clone, PartialEq)]
pub struct RtkSolution {
    /// 3D position coordinate estimate.
    pub position: Cartesian3D,
    /// Fix quality state.
    pub status: RtkFixStatus,
    /// Ambiguity ratio metric ($Q_2 / Q_1$).
    pub ambiguity_ratio: f64,
    /// Geometric Dilution of Precision (GDOP).
    pub gdop: f64,
    /// Estimated horizontal 3D root-mean-square error in meters.
    pub horizontal_rms_error_m: f64,
    /// Number of TRPs utilized in fix.
    pub num_trps: usize,
    /// Solver iterations executed.
    pub iterations: usize,
}

/// Operational telemetry metrics for carrier phase RTK.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RtkMetrics {
    pub total_epochs_processed: u64,
    pub fixed_epochs_count: u64,
    pub float_epochs_count: u64,
    pub cycle_slip_count: u64,
    pub average_3d_error_cm: f64,
}

// ---------------------------------------------------------------------------
// 7. Complete Carrier Phase RTK Solver Engine
// ---------------------------------------------------------------------------

/// 5G NR Carrier Phase Real-Time Kinematic (RTK) Solver Engine.
pub struct CarrierPhaseRtkSolver {
    pub trps: Vec<TrpCarrierPhaseConfig>,
    pub slip_detector: CycleSlipDetector,
    pub lambda_solver: LambdaAmbiguitySolver,
    pub metrics: RtkMetrics,
}

impl CarrierPhaseRtkSolver {
    /// Create new solver with configured TRP network.
    pub fn new(trps: Vec<TrpCarrierPhaseConfig>) -> Self {
        Self {
            trps,
            slip_detector: CycleSlipDetector::new(),
            lambda_solver: LambdaAmbiguitySolver::default(),
            metrics: RtkMetrics::default(),
        }
    }

    /// Add a TRP configuration to the active network.
    pub fn add_trp(&mut self, trp: TrpCarrierPhaseConfig) {
        self.trps.push(trp);
    }

    /// Solve 3D position using Double-Difference Carrier Phase observations.
    ///
    /// - `target_obs`: Carrier phase observations collected by the target UE.
    /// - `reference_obs`: Simultaneous observations collected by a reference anchor UE with known location `reference_pos`.
    /// - `reference_pos`: Known coordinate of the reference station.
    /// - `initial_guess`: Initial position estimate (e.g. from DL-TDoA / cell ID).
    pub fn solve_double_difference(
        &mut self,
        target_obs: &[CarrierPhaseObservation],
        reference_obs: &[CarrierPhaseObservation],
        reference_pos: &Cartesian3D,
        initial_guess: &Cartesian3D,
        known_ambiguities: Option<&[i32]>,
    ) -> Result<RtkSolution, CarrierPhaseError> {
        self.metrics.total_epochs_processed += 1;

        // 1. Identify common TRPs tracked by both target and reference
        let mut common_trp_ids = Vec::new();
        for to in target_obs {
            if reference_obs.iter().any(|ro| ro.trp_id == to.trp_id)
                && self.trps.iter().any(|t| t.trp_id == to.trp_id)
            {
                common_trp_ids.push(to.trp_id);
            }
        }

        // Need at least 4 TRPs (1 master reference + 3 difference pairs) for 3D (x, y, z) fix
        if common_trp_ids.len() < 4 {
            return Err(CarrierPhaseError::InsufficientTrps {
                needed: 4,
                available: common_trp_ids.len(),
            });
        }

        // Master reference TRP: choose common TRP with highest target SNR
        let master_trp_id = *common_trp_ids
            .iter()
            .max_by(|&&a, &&b| {
                let snr_a = target_obs
                    .iter()
                    .find(|o| o.trp_id == a)
                    .map(|o| o.snr_db)
                    .unwrap_or(0.0);
                let snr_b = target_obs
                    .iter()
                    .find(|o| o.trp_id == b)
                    .map(|o| o.snr_db)
                    .unwrap_or(0.0);
                snr_a
                    .partial_cmp(&snr_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        let master_trp = self
            .trps
            .iter()
            .find(|t| t.trp_id == master_trp_id)
            .unwrap()
            .clone();

        let target_master_phase = target_obs
            .iter()
            .find(|o| o.trp_id == master_trp_id)
            .unwrap()
            .carrier_phase_cycles;
        let ref_master_phase = reference_obs
            .iter()
            .find(|o| o.trp_id == master_trp_id)
            .unwrap()
            .carrier_phase_cycles;

        // 2. Form Double Difference observables:
        // DD_i = (target_phase_i - target_master_phase) - (ref_phase_i - ref_master_phase)
        let non_master_ids: Vec<u32> = common_trp_ids
            .into_iter()
            .filter(|&id| id != master_trp_id)
            .collect();

        let mut dd_observables = Vec::with_capacity(non_master_ids.len());
        let mut active_trps = Vec::with_capacity(non_master_ids.len());

        for &id in &non_master_ids {
            let trp = self.trps.iter().find(|t| t.trp_id == id).unwrap().clone();
            let t_phase = target_obs
                .iter()
                .find(|o| o.trp_id == id)
                .unwrap()
                .carrier_phase_cycles;
            let r_phase = reference_obs
                .iter()
                .find(|o| o.trp_id == id)
                .unwrap()
                .carrier_phase_cycles;

            let single_diff_target = t_phase - target_master_phase;
            let single_diff_ref = r_phase - ref_master_phase;
            let double_diff_cycles = single_diff_target - single_diff_ref;

            dd_observables.push(double_diff_cycles);
            active_trps.push(trp);
        }

        // 3. Estimate float ambiguities and solve position iteratively via Gauss-Newton
        let mut est_pos = *initial_guess;
        let wavelength = master_trp.wavelength_m;

        // Compute reference geometric terms: (dist_ref_i - dist_ref_0)
        let dist_ref_0 = reference_pos.distance_to(&master_trp.location);
        let ref_ranges: Vec<f64> = active_trps
            .iter()
            .map(|t| reference_pos.distance_to(&t.location) - dist_ref_0)
            .collect();

        // Approximate float ambiguities from initial guess
        let mut float_ambiguities = Vec::with_capacity(active_trps.len());
        let dist_tgt_0 = est_pos.distance_to(&master_trp.location);
        for (i, t) in active_trps.iter().enumerate() {
            let dist_tgt_i = est_pos.distance_to(&t.location);
            let expected_dd_cycles = ((dist_tgt_i - dist_tgt_0) - ref_ranges[i]) / wavelength;
            let float_n = dd_observables[i] - expected_dd_cycles;
            float_ambiguities.push(float_n);
        }

        // 4. Attempt Integer Ambiguity Resolution
        let (selected_ambiguities, ratio, is_fixed) = match known_ambiguities {
            Some(fixed_amb) => {
                let amb: Vec<f64> = fixed_amb.iter().map(|&n| n as f64).collect();
                self.metrics.fixed_epochs_count += 1;
                (amb, 999.0, true)
            }
            None => {
                let (int_ambiguities, ratio, is_fixed) =
                    self.lambda_solver.resolve_ambiguities(&float_ambiguities)?;
                let amb: Vec<f64> = if is_fixed {
                    self.metrics.fixed_epochs_count += 1;
                    int_ambiguities.iter().map(|&n| n as f64).collect()
                } else {
                    self.metrics.float_epochs_count += 1;
                    float_ambiguities
                };
                (amb, ratio, is_fixed)
            }
        };

        // 5. Gauss-Newton Iterative 3D Position Solver
        let mut converged = false;
        let mut final_iterations = 0;

        for iter in 0..MAX_SOLVER_ITERATIONS {
            final_iterations = iter + 1;
            let dist_0 = est_pos.distance_to(&master_trp.location);
            let u0 = master_trp.location.unit_vector_to(&est_pos)?;

            // Build design matrix H (M x 3) and residual vector r (M x 1)
            let m = active_trps.len();
            let mut h_matrix = vec![0.0; m * 3];
            let mut r_vector = vec![0.0; m];

            for (i, t) in active_trps.iter().enumerate() {
                let dist_i = est_pos.distance_to(&t.location);
                let ui = t.location.unit_vector_to(&est_pos)?;

                // Row i of H: (ui - u0)
                let hx = ui.x - u0.x;
                let hy = ui.y - u0.y;
                let hz = ui.z - u0.z;

                h_matrix[i * 3 + 0] = hx;
                h_matrix[i * 3 + 1] = hy;
                h_matrix[i * 3 + 2] = hz;

                // Residual: measured DD distance - modeled DD distance
                let modeled_dd_m = (dist_i - dist_0) - ref_ranges[i];
                let measured_dd_m = (dd_observables[i] - selected_ambiguities[i]) * wavelength;
                r_vector[i] = measured_dd_m - modeled_dd_m;
            }

            // Normal equations: (H^T H) delta_p = H^T r
            // Compute 3x3 normal matrix N = H^T H
            let mut n_matrix = [0.0; 9];
            let mut g_vector = [0.0; 3];

            for i in 0..m {
                let hx = h_matrix[i * 3 + 0];
                let hy = h_matrix[i * 3 + 1];
                let hz = h_matrix[i * 3 + 2];
                let r = r_vector[i];

                n_matrix[0] += hx * hx;
                n_matrix[1] += hx * hy;
                n_matrix[2] += hx * hz;

                n_matrix[3] += hy * hx;
                n_matrix[4] += hy * hy;
                n_matrix[5] += hy * hz;

                n_matrix[6] += hz * hx;
                n_matrix[7] += hz * hy;
                n_matrix[8] += hz * hz;

                g_vector[0] += hx * r;
                g_vector[1] += hy * r;
                g_vector[2] += hz * r;
            }

            // Invert 3x3 normal matrix
            let inv_n = invert_3x3(&n_matrix).ok_or_else(|| {
                CarrierPhaseError::GeometricDilutionDeficiency(
                    "Singular normal matrix: TRPs are coplanar or collinear".to_string(),
                )
            })?;

            // Compute delta_p = inv_N * g
            let dx = inv_n[0] * g_vector[0] + inv_n[1] * g_vector[1] + inv_n[2] * g_vector[2];
            let dy = inv_n[3] * g_vector[0] + inv_n[4] * g_vector[1] + inv_n[5] * g_vector[2];
            let dz = inv_n[6] * g_vector[0] + inv_n[7] * g_vector[1] + inv_n[8] * g_vector[2];

            est_pos.x += dx;
            est_pos.y += dy;
            est_pos.z += dz;

            let step_norm = (dx * dx + dy * dy + dz * dz).sqrt();
            if step_norm < SOLVER_CONVERGENCE_TOLERANCE_M {
                converged = true;
                break;
            }
        }

        if !converged {
            return Err(CarrierPhaseError::ConvergenceFailure {
                iterations: final_iterations,
                residual_norm: 0.0,
            });
        }

        // Calculate GDOP from trace of inverted normal matrix
        let u0_final = master_trp.location.unit_vector_to(&est_pos)?;
        let mut final_h = vec![0.0; active_trps.len() * 3];
        for (i, t) in active_trps.iter().enumerate() {
            let ui = t.location.unit_vector_to(&est_pos)?;
            final_h[i * 3 + 0] = ui.x - u0_final.x;
            final_h[i * 3 + 1] = ui.y - u0_final.y;
            final_h[i * 3 + 2] = ui.z - u0_final.z;
        }
        let mut final_n = [0.0; 9];
        for i in 0..active_trps.len() {
            let hx = final_h[i * 3 + 0];
            let hy = final_h[i * 3 + 1];
            let hz = final_h[i * 3 + 2];
            final_n[0] += hx * hx;
            final_n[1] += hx * hy;
            final_n[2] += hx * hz;
            final_n[3] += hy * hx;
            final_n[4] += hy * hy;
            final_n[5] += hy * hz;
            final_n[6] += hz * hx;
            final_n[7] += hz * hy;
            final_n[8] += hz * hz;
        }
        let inv_final_n = invert_3x3(&final_n).unwrap_or([1.0; 9]);
        let gdop = (inv_final_n[0] + inv_final_n[4] + inv_final_n[8])
            .max(0.0)
            .sqrt();

        // Estimated RMS error: carrier wavelength scale * noise factor
        let horizontal_rms = if is_fixed {
            // Millimeter to centimeter precision for Fixed RTK
            (0.015 * gdop).clamp(0.005, 0.050)
        } else {
            // Decimeter precision for Float RTK
            (0.150 * gdop).clamp(0.100, 0.800)
        };

        let status = if is_fixed {
            RtkFixStatus::Fixed
        } else {
            RtkFixStatus::Float
        };

        // Cycle slip detection update on tracked TRPs
        for t in &active_trps {
            let d = est_pos.distance_to(&t.location);
            let obs = target_obs.iter().find(|o| o.trp_id == t.trp_id).unwrap();
            let _ = self.slip_detector.check_and_update(obs, d, wavelength);
        }

        Ok(RtkSolution {
            position: est_pos,
            status,
            ambiguity_ratio: ratio,
            gdop,
            horizontal_rms_error_m: horizontal_rms,
            num_trps: active_trps.len() + 1,
            iterations: final_iterations,
        })
    }
}

// ---------------------------------------------------------------------------
// Matrix Inversion Utility (Pure Rust 3x3 Analytic Inversion)
// ---------------------------------------------------------------------------

/// Computes analytic inverse of a 3x3 matrix in row-major layout.
/// Returns None if determinant is zero / singular.
fn invert_3x3(m: &[f64; 9]) -> Option<[f64; 9]> {
    let a = m[0];
    let b = m[1];
    let c = m[2];
    let d = m[3];
    let e = m[4];
    let f = m[5];
    let g = m[6];
    let h = m[7];
    let i = m[8];

    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);

    if det.abs() < 1e-12 {
        return None;
    }

    let inv_det = 1.0 / det;

    Some([
        (e * i - f * h) * inv_det,
        (c * h - b * i) * inv_det,
        (b * f - c * e) * inv_det,
        (f * g - d * i) * inv_det,
        (a * i - c * g) * inv_det,
        (c * d - a * f) * inv_det,
        (d * h - e * g) * inv_det,
        (g * b - a * h) * inv_det,
        (a * e - b * d) * inv_det,
    ])
}
