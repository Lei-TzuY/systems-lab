//! 3GPP Rel-17 / Rel-18 5G NR Sidelink Positioning & Direct Ranging Engine.
//!
//! Implements 3GPP TR 38.845, TS 38.305, TS 38.215, and TS 38.211 §5.2.1 / §8.4 specifications:
//! - Sidelink Positioning Reference Signal (SL-PRS) 31-bit length Gold sequence generation.
//! - Configurable comb patterns (Comb-2, Comb-4, Comb-6, Comb-12) and resource pool slot mapping.
//! - High-accuracy Sidelink Round-Trip Time (SL-RTT) two-way time-of-flight (ToF) ranging with
//!   internal hardware transceiver group delay calibration.
//! - Sidelink Angle of Arrival (SL-AoA) and Angle of Departure (SL-AoD) multi-antenna phase interferometry.
//! - Cooperative multi-anchor 2D/3D non-linear least-squares Gauss-Newton multilateration solver.
//! - Kinematic tracking filter estimating relative position, velocity, and dilution of precision (GDOP).
//! - Direct PC5 Sidelink Ranging Session state machine and telemetry.

use std::fmt;

/// Speed of light in vacuum in meters per second (CODATA / BIPM).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// Standard number of subcarriers per Physical Resource Block in 5G NR.
pub const NR_SUBCARRIERS_PER_PRB: usize = 12;

/// Standard number of symbols in a normal cyclic prefix slot.
pub const NR_SYMBOLS_PER_SLOT: usize = 14;

/// Sidelink PRS Comb Size (Subcarrier Decimation Factor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlCombSize {
    Comb2 = 2,
    Comb4 = 4,
    Comb6 = 6,
    Comb12 = 12,
}

impl SlCombSize {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Operational state of a Sidelink Direct Ranging Session (PC5 interface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlSessionState {
    Idle,
    Negotiating,
    Measuring,
    Tracking,
    Terminated,
}

/// Errors raised during Sidelink positioning and ranging computations.
#[derive(Debug, Clone, PartialEq)]
pub enum SlPositioningError {
    InvalidCombOffset {
        offset: u8,
        comb_size: u8,
    },
    InvalidSymbolRange {
        start: u8,
        duration: u8,
    },
    NegativeRttDistance {
        rtt_ns: f64,
    },
    InsufficientAnchors {
        required: usize,
        provided: usize,
    },
    SingularMatrix,
    AngleOutOfRange(f64),
    SessionStateConflict {
        current: SlSessionState,
        action: &'static str,
    },
}

impl fmt::Display for SlPositioningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlPositioningError::InvalidCombOffset { offset, comb_size } => {
                write!(
                    f,
                    "Invalid comb offset {} for comb size {} (must be < comb size)",
                    offset, comb_size
                )
            }
            SlPositioningError::InvalidSymbolRange { start, duration } => {
                write!(
                    f,
                    "Invalid symbol range: start {} + duration {} exceeds slot boundary (14)",
                    start, duration
                )
            }
            SlPositioningError::NegativeRttDistance { rtt_ns } => {
                write!(
                    f,
                    "Calculated negative RTT propagation delay: {:.4} ns",
                    rtt_ns
                )
            }
            SlPositioningError::InsufficientAnchors { required, provided } => {
                write!(
                    f,
                    "Insufficient anchor UEs for multilateration: need at least {}, got {}",
                    required, provided
                )
            }
            SlPositioningError::SingularMatrix => {
                write!(
                    f,
                    "Singular normal matrix encountered during multilateration inversion"
                )
            }
            SlPositioningError::AngleOutOfRange(ang) => {
                write!(
                    f,
                    "Phase difference argument produces out-of-range angle: {}",
                    ang
                )
            }
            SlPositioningError::SessionStateConflict { current, action } => {
                write!(
                    f,
                    "Cannot execute action '{}' while ranging session is in state {:?}",
                    action, current
                )
            }
        }
    }
}

impl std::error::Error for SlPositioningError {}

/// 31-bit Pseudo-Random Gold Sequence Generator (3GPP TS 38.211 §5.2.1).
#[derive(Debug, Clone)]
pub struct GoldSequenceGenerator {
    x1: u32,
    x2: u32,
}

impl GoldSequenceGenerator {
    /// Initializes Gold code with $c_{init}$.
    /// Pre-advances $N_c = 1600$ steps per TS 38.211 §5.2.1.
    pub fn new(c_init: u32) -> Self {
        let x1 = 1u32; // x1(0) = 1, others 0
        let x2 = c_init & 0x7FFF_FFFF;

        let mut seq_gen = Self { x1, x2 };
        // Advance 1600 steps
        for _ in 0..1600 {
            seq_gen.step();
        }
        seq_gen
    }

    /// Advances the LFSR by 1 step and returns the output bit $c(n)$.
    #[inline]
    pub fn step(&mut self) -> u8 {
        // x1 generator polynomial: x^31 + x^3 + 1
        let b1 = ((self.x1 >> 3) ^ self.x1) & 1;
        self.x1 = (self.x1 >> 1) | (b1 << 30);

        // x2 generator polynomial: x^31 + x^3 + x^2 + x + 1
        let b2 = ((self.x2 >> 3) ^ (self.x2 >> 2) ^ (self.x2 >> 1) ^ self.x2) & 1;
        self.x2 = (self.x2 >> 1) | (b2 << 30);

        ((b1 ^ b2) & 1) as u8
    }

    /// Generates $N$ pseudo-random bits.
    pub fn generate_bits(&mut self, n: usize) -> Vec<u8> {
        let mut bits = Vec::with_capacity(n);
        for _ in 0..n {
            bits.push(self.step());
        }
        bits
    }
}

/// Sidelink Positioning Reference Signal (SL-PRS) Configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlPrsConfig {
    pub prs_id: u16,
    pub comb_size: SlCombSize,
    pub comb_offset: u8,
    pub start_symbol: u8,
    pub num_symbols: u8,
    pub num_prbs: u16,
    pub slot_idx: u32,
}

impl SlPrsConfig {
    pub fn new(
        prs_id: u16,
        comb_size: SlCombSize,
        comb_offset: u8,
        start_symbol: u8,
        num_symbols: u8,
        num_prbs: u16,
        slot_idx: u32,
    ) -> Result<Self, SlPositioningError> {
        if comb_offset >= comb_size.as_u8() {
            return Err(SlPositioningError::InvalidCombOffset {
                offset: comb_offset,
                comb_size: comb_size.as_u8(),
            });
        }
        if start_symbol >= NR_SYMBOLS_PER_SLOT as u8
            || num_symbols == 0
            || (start_symbol + num_symbols) > NR_SYMBOLS_PER_SLOT as u8
        {
            return Err(SlPositioningError::InvalidSymbolRange {
                start: start_symbol,
                duration: num_symbols,
            });
        }

        Ok(Self {
            prs_id,
            comb_size,
            comb_offset,
            start_symbol,
            num_symbols,
            num_prbs,
            slot_idx,
        })
    }

    /// Computes sequence initialization seed $c_{init}$ per TS 38.211 §8.4.1.
    pub fn c_init(&self, symbol: u8) -> u32 {
        let n_id = (self.prs_id as u32) & 0x0FFF; // 12-bit PRS ID
        let l = symbol as u32;
        let slot = self.slot_idx;

        // c_init = (2^22 * n_id + 2^10 * (14 * slot + l + 1) + 1) mod 2^31
        let term1 = (n_id << 22) & 0x7FFF_FFFF;
        let term2 = (((14 * slot + l + 1) & 0xFFF) << 10) & 0x7FFF_FFFF;
        (term1.wrapping_add(term2).wrapping_add(1)) & 0x7FFF_FFFF
    }

    /// Generates complex QPSK-modulated SL-PRS subcarrier values for a given symbol.
    pub fn generate_symbol_re_pattern(&self, symbol: u8) -> Vec<(usize, (f64, f64))> {
        let c_init = self.c_init(symbol);
        let mut gold = GoldSequenceGenerator::new(c_init);

        let subcarriers_per_symbol =
            ((self.num_prbs as usize) * NR_SUBCARRIERS_PER_PRB) / (self.comb_size.as_u8() as usize);
        let bits = gold.generate_bits(subcarriers_per_symbol * 2);

        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        let mut result = Vec::with_capacity(subcarriers_per_symbol);

        let comb = self.comb_size.as_u8() as usize;
        let offset = self.comb_offset as usize;

        for m in 0..subcarriers_per_symbol {
            let re_idx = m * comb + offset;
            let b0 = bits[2 * m];
            let b1 = bits[2 * m + 1];

            // QPSK: r = (1 - 2*b0)/sqrt(2) + j * (1 - 2*b1)/sqrt(2)
            let i = if b0 == 0 { inv_sqrt2 } else { -inv_sqrt2 };
            let q = if b1 == 0 { inv_sqrt2 } else { -inv_sqrt2 };

            result.push((re_idx, (i, q)));
        }

        result
    }
}

/// Sidelink Round-Trip Time (SL-RTT) Two-Way Measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlRttMeasurement {
    /// Initiator departure timestamp (t1) in nanoseconds.
    pub t1_tx_ns: f64,
    /// Responder arrival timestamp (t2) in nanoseconds.
    pub t2_rx_ns: f64,
    /// Responder departure timestamp (t3) in nanoseconds.
    pub t3_tx_ns: f64,
    /// Initiator arrival timestamp (t4) in nanoseconds.
    pub t4_rx_ns: f64,
    /// Initiator hardware transceiver internal delay calibration in nanoseconds.
    pub initiator_cal_delay_ns: f64,
    /// Responder hardware transceiver internal delay calibration in nanoseconds.
    pub responder_cal_delay_ns: f64,
}

impl SlRttMeasurement {
    pub fn new(
        t1_tx_ns: f64,
        t2_rx_ns: f64,
        t3_tx_ns: f64,
        t4_rx_ns: f64,
        initiator_cal_delay_ns: f64,
        responder_cal_delay_ns: f64,
    ) -> Self {
        Self {
            t1_tx_ns,
            t2_rx_ns,
            t3_tx_ns,
            t4_rx_ns,
            initiator_cal_delay_ns,
            responder_cal_delay_ns,
        }
    }

    /// Computes one-way propagation time-of-flight (ToF) in nanoseconds.
    pub fn calculate_tof_ns(&self) -> Result<f64, SlPositioningError> {
        let total_round_trip = self.t4_rx_ns - self.t1_tx_ns;
        let responder_turnaround = self.t3_tx_ns - self.t2_rx_ns;
        let cal_overhead = self.initiator_cal_delay_ns + self.responder_cal_delay_ns;

        let net_two_way_flight = total_round_trip - responder_turnaround - cal_overhead;
        if net_two_way_flight < 0.0 {
            return Err(SlPositioningError::NegativeRttDistance {
                rtt_ns: net_two_way_flight,
            });
        }

        Ok(net_two_way_flight / 2.0)
    }

    /// Computes estimated physical distance in meters.
    pub fn calculate_distance_m(&self) -> Result<f64, SlPositioningError> {
        let tof_ns = self.calculate_tof_ns()?;
        let tof_sec = tof_ns * 1e-9;
        Ok(tof_sec * SPEED_OF_LIGHT_M_S)
    }
}

/// Sidelink Angle of Arrival (SL-AoA) & Departure (SL-AoD) Measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlAoAMeasurement {
    /// Phase difference between adjacent antenna elements along horizontal azimuth in radians.
    pub azimuth_phase_diff_rad: f64,
    /// Phase difference between adjacent antenna elements along vertical elevation in radians.
    pub elevation_phase_diff_rad: f64,
    /// RF carrier frequency in Hz (e.g. 5.9 GHz for V2X PC5).
    pub carrier_frequency_hz: f64,
    /// Antenna inter-element spacing in meters (typically lambda / 2).
    pub antenna_spacing_m: f64,
}

impl SlAoAMeasurement {
    pub fn new(
        azimuth_phase_diff_rad: f64,
        elevation_phase_diff_rad: f64,
        carrier_frequency_hz: f64,
        antenna_spacing_m: f64,
    ) -> Self {
        Self {
            azimuth_phase_diff_rad,
            elevation_phase_diff_rad,
            carrier_frequency_hz,
            antenna_spacing_m,
        }
    }

    /// Standard antenna spacing of half-wavelength ($\lambda / 2$).
    pub fn half_wavelength(carrier_frequency_hz: f64) -> f64 {
        let lambda = SPEED_OF_LIGHT_M_S / carrier_frequency_hz;
        lambda / 2.0
    }

    /// Computes azimuth angle in degrees relative to antenna boresight [-90.0, +90.0].
    pub fn calculate_azimuth_deg(&self) -> Result<f64, SlPositioningError> {
        let lambda = SPEED_OF_LIGHT_M_S / self.carrier_frequency_hz;
        // delta_phi = (2 * pi * d / lambda) * sin(theta)
        // sin(theta) = delta_phi * lambda / (2 * pi * d)
        let ratio = (self.azimuth_phase_diff_rad * lambda)
            / (2.0 * std::f64::consts::PI * self.antenna_spacing_m);

        if ratio.abs() > 1.0 {
            return Err(SlPositioningError::AngleOutOfRange(ratio));
        }

        let theta_rad = ratio.asin();
        Ok(theta_rad.to_degrees())
    }

    /// Computes elevation angle in degrees relative to horizon [-90.0, +90.0].
    pub fn calculate_elevation_deg(&self) -> Result<f64, SlPositioningError> {
        let lambda = SPEED_OF_LIGHT_M_S / self.carrier_frequency_hz;
        let ratio = (self.elevation_phase_diff_rad * lambda)
            / (2.0 * std::f64::consts::PI * self.antenna_spacing_m);

        if ratio.abs() > 1.0 {
            return Err(SlPositioningError::AngleOutOfRange(ratio));
        }

        let phi_rad = ratio.asin();
        Ok(phi_rad.to_degrees())
    }
}

/// Sidelink Anchor UE descriptor with known reference coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlAnchorUe {
    pub anchor_id: u32,
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
}

/// 3D Target Positioning Estimate resolved from Sidelink Multilateration.
#[derive(Debug, Clone, PartialEq)]
pub struct SlPositionEstimate {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub residual_rms_m: f64,
    pub gdop: f64,
    pub iterations_used: usize,
}

/// Cooperative Sidelink Multilateration Solver using Gauss-Newton Optimization.
#[derive(Debug, Clone)]
pub struct SlMultilaterationSolver {
    pub max_iterations: usize,
    pub convergence_epsilon_m: f64,
}

impl Default for SlMultilaterationSolver {
    fn default() -> Self {
        Self {
            max_iterations: 25,
            convergence_epsilon_m: 1e-4,
        }
    }
}

impl SlMultilaterationSolver {
    pub fn new(max_iterations: usize, convergence_epsilon_m: f64) -> Self {
        Self {
            max_iterations,
            convergence_epsilon_m,
        }
    }

    /// Solves 3D target coordinates from at least 3 anchor measurements using non-linear least squares.
    pub fn solve_position(
        &self,
        anchors: &[(SlAnchorUe, f64)], // (Anchor, measured_range_m)
        initial_guess: Option<(f64, f64, f64)>,
    ) -> Result<SlPositionEstimate, SlPositioningError> {
        let n = anchors.len();
        if n < 3 {
            return Err(SlPositioningError::InsufficientAnchors {
                required: 3,
                provided: n,
            });
        }

        // Initial guess: centroid of anchors or provided point
        let mut x = initial_guess
            .map(|g| g.0)
            .unwrap_or_else(|| anchors.iter().map(|(a, _)| a.x_m).sum::<f64>() / (n as f64));
        let mut y = initial_guess
            .map(|g| g.1)
            .unwrap_or_else(|| anchors.iter().map(|(a, _)| a.y_m).sum::<f64>() / (n as f64));
        let mut z = initial_guess
            .map(|g| g.2)
            .unwrap_or_else(|| anchors.iter().map(|(a, _)| a.z_m).sum::<f64>() / (n as f64));

        let mut iterations = 0;

        for iter in 0..self.max_iterations {
            iterations = iter + 1;

            // Compute residuals f_i = sqrt((x - xi)^2 + (y - yi)^2 + (z - zi)^2) - r_i
            // and Jacobian elements J_i = [(x - xi)/d, (y - yi)/d, (z - zi)/d]
            let mut jtj = [[0.0f64; 3]; 3];
            let mut jtf = [0.0f64; 3];

            for &(a, r_meas) in anchors {
                let dx = x - a.x_m;
                let dy = y - a.y_m;
                let dz = z - a.z_m;
                let d_est = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-6);

                let residual = d_est - r_meas;

                let j0 = dx / d_est;
                let j1 = dy / d_est;
                let j2 = dz / d_est;

                // JT * J
                jtj[0][0] += j0 * j0;
                jtj[0][1] += j0 * j1;
                jtj[0][2] += j0 * j2;

                jtj[1][0] += j1 * j0;
                jtj[1][1] += j1 * j1;
                jtj[1][2] += j1 * j2;

                jtj[2][0] += j2 * j0;
                jtj[2][1] += j2 * j1;
                jtj[2][2] += j2 * j2;

                // JT * f
                jtf[0] += j0 * residual;
                jtf[1] += j1 * residual;
                jtf[2] += j2 * residual;
            }

            // Invert 3x3 JTJ matrix using Cramer's rule
            let inv_jtj = Self::invert_3x3(&jtj).ok_or(SlPositioningError::SingularMatrix)?;

            // Delta = (JTJ)^-1 * (-JTF)
            let delta_x =
                -(inv_jtj[0][0] * jtf[0] + inv_jtj[0][1] * jtf[1] + inv_jtj[0][2] * jtf[2]);
            let delta_y =
                -(inv_jtj[1][0] * jtf[0] + inv_jtj[1][1] * jtf[1] + inv_jtj[1][2] * jtf[2]);
            let delta_z =
                -(inv_jtj[2][0] * jtf[0] + inv_jtj[2][1] * jtf[1] + inv_jtj[2][2] * jtf[2]);

            x += delta_x;
            y += delta_y;
            z += delta_z;

            let step_norm = (delta_x * delta_x + delta_y * delta_y + delta_z * delta_z).sqrt();
            if step_norm < self.convergence_epsilon_m {
                break;
            }
        }

        // Calculate residual RMS and GDOP
        let mut sum_sq_err = 0.0;
        for &(a, r_meas) in anchors {
            let dx = x - a.x_m;
            let dy = y - a.y_m;
            let dz = z - a.z_m;
            let d_est = (dx * dx + dy * dy + dz * dz).sqrt();
            let err = d_est - r_meas;
            sum_sq_err += err * err;
        }
        let residual_rms_m = (sum_sq_err / (n as f64)).sqrt();

        // GDOP estimation
        let jtj_final = Self::compute_jtj(anchors, x, y, z);
        let gdop = Self::invert_3x3(&jtj_final)
            .map(|inv| (inv[0][0] + inv[1][1] + inv[2][2]).max(0.0).sqrt())
            .unwrap_or(1.0);

        Ok(SlPositionEstimate {
            x_m: x,
            y_m: y,
            z_m: z,
            residual_rms_m,
            gdop,
            iterations_used: iterations,
        })
    }

    #[inline]
    fn compute_jtj(anchors: &[(SlAnchorUe, f64)], x: f64, y: f64, z: f64) -> [[f64; 3]; 3] {
        let mut jtj = [[0.0f64; 3]; 3];
        for &(a, _) in anchors {
            let dx = x - a.x_m;
            let dy = y - a.y_m;
            let dz = z - a.z_m;
            let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-6);
            let j = [dx / d, dy / d, dz / d];
            for r in 0..3 {
                for c in 0..3 {
                    jtj[r][c] += j[r] * j[c];
                }
            }
        }
        jtj
    }

    #[inline]
    fn invert_3x3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);

        if det.abs() < 1e-12 {
            return None;
        }

        let inv_det = 1.0 / det;
        let mut inv = [[0.0f64; 3]; 3];

        inv[0][0] = (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det;
        inv[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det;
        inv[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det;

        inv[1][0] = (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det;
        inv[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det;
        inv[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det;

        inv[2][0] = (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det;
        inv[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det;
        inv[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det;

        Some(inv)
    }
}

/// 2D Kinematic Tracker for Sidelink Relative Trajectory.
/// Tracks state $\mathbf{x} = [x, y, v_x, v_y]^T$.
#[derive(Debug, Clone)]
pub struct SlKinematicTracker {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub pos_variance: f64,
    pub vel_variance: f64,
    pub process_noise: f64,
}

impl SlKinematicTracker {
    pub fn new(x0: f64, y0: f64, initial_variance: f64, process_noise: f64) -> Self {
        Self {
            x: x0,
            y: y0,
            vx: 0.0,
            vy: 0.0,
            pos_variance: initial_variance,
            vel_variance: initial_variance,
            process_noise,
        }
    }

    /// Predicts state forward by $\Delta t$ seconds.
    pub fn predict(&mut self, dt_sec: f64) {
        self.x += self.vx * dt_sec;
        self.y += self.vy * dt_sec;

        // Propagate variances
        self.pos_variance += (self.vel_variance * dt_sec * dt_sec) + (self.process_noise * dt_sec);
        self.vel_variance += self.process_noise * dt_sec;
    }

    /// Updates state with a position measurement $(z_x, z_y)$ and measurement variance $\sigma_m^2$.
    pub fn update_measurement(&mut self, zx: f64, zy: f64, meas_variance: f64, dt_sec: f64) {
        // Kalman gain K = P / (P + R)
        let k_pos = self.pos_variance / (self.pos_variance + meas_variance);
        let err_x = zx - self.x;
        let err_y = zy - self.y;

        self.x += k_pos * err_x;
        self.y += k_pos * err_y;

        if dt_sec > 0.0 {
            let k_vel = (self.vel_variance / (self.vel_variance + meas_variance * 4.0)).min(0.5);
            self.vx += k_vel * (err_x / dt_sec);
            self.vy += k_vel * (err_y / dt_sec);
        }

        self.pos_variance *= 1.0 - k_pos;
    }

    /// Current velocity magnitude in m/s (speed).
    pub fn speed_mps(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }
}

/// Sidelink Direct Ranging Session between Peer UEs (PC5 interface).
#[derive(Debug, Clone)]
pub struct SlRangingSession {
    pub session_id: u32,
    pub local_ue_id: u32,
    pub peer_ue_id: u32,
    pub state: SlSessionState,
    pub prs_config: SlPrsConfig,
    pub total_measurements: u32,
    pub last_distance_m: Option<f64>,
    pub running_avg_distance_m: f64,
}

impl SlRangingSession {
    pub fn new(
        session_id: u32,
        local_ue_id: u32,
        peer_ue_id: u32,
        prs_config: SlPrsConfig,
    ) -> Self {
        Self {
            session_id,
            local_ue_id,
            peer_ue_id,
            state: SlSessionState::Idle,
            prs_config,
            total_measurements: 0,
            last_distance_m: None,
            running_avg_distance_m: 0.0,
        }
    }

    /// Initiates ranging negotiation with peer UE.
    pub fn start_negotiation(&mut self) -> Result<(), SlPositioningError> {
        if self.state != SlSessionState::Idle {
            return Err(SlPositioningError::SessionStateConflict {
                current: self.state,
                action: "start_negotiation",
            });
        }
        self.state = SlSessionState::Negotiating;
        Ok(())
    }

    /// Completes negotiation and enters active measurement mode.
    pub fn confirm_negotiation(&mut self) -> Result<(), SlPositioningError> {
        if self.state != SlSessionState::Negotiating {
            return Err(SlPositioningError::SessionStateConflict {
                current: self.state,
                action: "confirm_negotiation",
            });
        }
        self.state = SlSessionState::Measuring;
        Ok(())
    }

    /// Records an RTT ranging measurement.
    pub fn record_rtt_measurement(
        &mut self,
        meas: SlRttMeasurement,
    ) -> Result<f64, SlPositioningError> {
        if self.state != SlSessionState::Measuring && self.state != SlSessionState::Tracking {
            return Err(SlPositioningError::SessionStateConflict {
                current: self.state,
                action: "record_rtt_measurement",
            });
        }

        let dist_m = meas.calculate_distance_m()?;
        self.last_distance_m = Some(dist_m);
        self.total_measurements += 1;

        if self.total_measurements == 1 {
            self.running_avg_distance_m = dist_m;
        } else {
            // Exponential moving average (alpha = 0.2)
            self.running_avg_distance_m = 0.8 * self.running_avg_distance_m + 0.2 * dist_m;
        }

        if self.total_measurements >= 5 {
            self.state = SlSessionState::Tracking;
        }

        Ok(dist_m)
    }

    /// Terminates the ranging session.
    pub fn terminate(&mut self) {
        self.state = SlSessionState::Terminated;
    }
}
