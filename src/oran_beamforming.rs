//! O-RAN WG4 Open Fronthaul Massive MIMO Digital & Hybrid Beamforming Control Engine.
//!
//! Compliant with:
//! - O-RAN-WG4.CUS.0 Section 7.5.3 (Section Extension 1, 2, and 5)
//! - 3GPP TR 38.901 Section 7.3 ("Antenna Array Topology: ULA and URA / UPA")
//! - Multi-User MIMO (MU-MIMO) Zero-Forcing (ZF) and MMSE Precoding
//!
//! Features:
//! 1. 3D spatial antenna array modeling for Uniform Linear Array (ULA) and Uniform Planar Array (UPA).
//! 2. Cross-polarized antenna elements (+45° / -45°) supporting 32T32R, 64T64R, and 128T128R arrays.
//! 3. Spatial steering vector generation across azimuth (-90°..+90°) and elevation (-30°..+30°).
//! 4. 3D Grid-of-Beams (GoB) codebook generation, management, and fast beamId indexing.
//! 5. Multi-User MIMO Zero-Forcing (ZF) and Regularized ZF / MMSE precoding matrix computation
//!    in pure Rust with matrix inversion and interference nulling (> 25 dB SIR).
//! 6. Power normalization across transceivers and Block Floating Point (BFP) / fixed-point
//!    weight quantization directly interfacing with C-Plane Section Extension 1.

use crate::oran_cplane_ext::{
    BfwBundle, BfwCompressionMethod, BfwWeight, SectionExtension1, SectionExtension2,
};
use std::f64::consts::PI;
use std::fmt;

// ---------------------------------------------------------------------------
// Pure Rust Complex Number Engine
// ---------------------------------------------------------------------------

/// Lightweight Complex Number for beamforming computations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ComplexNumber {
    pub re: f64,
    pub im: f64,
}

impl ComplexNumber {
    pub const ZERO: ComplexNumber = ComplexNumber { re: 0.0, im: 0.0 };
    pub const ONE: ComplexNumber = ComplexNumber { re: 1.0, im: 0.0 };

    #[inline]
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Creates a complex number on the unit circle: exp(j * theta).
    #[inline]
    pub fn from_polar(r: f64, theta_rad: f64) -> Self {
        Self {
            re: r * theta_rad.cos(),
            im: r * theta_rad.sin(),
        }
    }

    #[inline]
    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    #[inline]
    pub fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    #[inline]
    pub fn add(&self, rhs: &Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }

    #[inline]
    pub fn sub(&self, rhs: &Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }

    #[inline]
    pub fn mul(&self, rhs: &Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }

    #[inline]
    pub fn scale(&self, factor: f64) -> Self {
        Self {
            re: self.re * factor,
            im: self.im * factor,
        }
    }

    #[inline]
    pub fn div(&self, rhs: &Self) -> Self {
        let denom = rhs.norm_sq();
        if denom == 0.0 {
            Self::ZERO
        } else {
            Self {
                re: (self.re * rhs.re + self.im * rhs.im) / denom,
                im: (self.im * rhs.re - self.re * rhs.im) / denom,
            }
        }
    }
}

impl fmt::Display for ComplexNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{:.4} + {:.4}j", self.re, self.im)
        } else {
            write!(f, "{:.4} - {:.4}j", self.re, -self.im)
        }
    }
}

// ---------------------------------------------------------------------------
// Antenna Array Topology & Polarization
// ---------------------------------------------------------------------------

/// Antenna polarization model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntennaPolarization {
    /// Single polarization (e.g. vertical 0°).
    SinglePol,
    /// Cross-polarized dual polarization (+45° / -45°).
    DualPolCross45,
}

/// Geometric antenna array topology (3GPP TR 38.901 Section 7.3).
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayTopology {
    /// 1D Uniform Linear Array (ULA).
    UniformLinearArray {
        num_elements: usize,
        element_spacing_wavelength: f64,
    },
    /// 2D Uniform Planar Array (UPA / URA).
    UniformPlanarArray {
        num_cols: usize,
        num_rows: usize,
        col_spacing_wavelength: f64,
        row_spacing_wavelength: f64,
    },
}

/// Antenna array configuration for Massive MIMO O-RU.
#[derive(Debug, Clone, PartialEq)]
pub struct AntennaArrayConfig {
    pub topology: ArrayTopology,
    pub polarization: AntennaPolarization,
    pub carrier_frequency_hz: f64,
    pub max_tx_power_dbm: f64,
}

impl AntennaArrayConfig {
    /// Standard 64T64R Massive MIMO configuration (8 columns x 4 rows, dual-pol, 0.5 lambda spacing).
    pub fn default_64t64r(carrier_frequency_hz: f64) -> Self {
        Self {
            topology: ArrayTopology::UniformPlanarArray {
                num_cols: 8,
                num_rows: 4,
                col_spacing_wavelength: 0.5,
                row_spacing_wavelength: 0.5,
            },
            polarization: AntennaPolarization::DualPolCross45,
            carrier_frequency_hz,
            max_tx_power_dbm: 46.0, // 40 Watts
        }
    }

    /// Standard 32T32R Massive MIMO configuration (8 columns x 2 rows, dual-pol).
    pub fn default_32t32r(carrier_frequency_hz: f64) -> Self {
        Self {
            topology: ArrayTopology::UniformPlanarArray {
                num_cols: 8,
                num_rows: 2,
                col_spacing_wavelength: 0.5,
                row_spacing_wavelength: 0.5,
            },
            polarization: AntennaPolarization::DualPolCross45,
            carrier_frequency_hz,
            max_tx_power_dbm: 43.0, // 20 Watts
        }
    }

    /// Returns the total number of physical antenna transceiver ports.
    pub fn total_transceivers(&self) -> usize {
        let physical_elements = match &self.topology {
            ArrayTopology::UniformLinearArray { num_elements, .. } => *num_elements,
            ArrayTopology::UniformPlanarArray {
                num_cols, num_rows, ..
            } => num_cols * num_rows,
        };

        match self.polarization {
            AntennaPolarization::SinglePol => physical_elements,
            AntennaPolarization::DualPolCross45 => physical_elements * 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Spatial Steering Vectors & Beam Calculations
// ---------------------------------------------------------------------------

/// 3D Spatial Angles (Azimuth and Elevation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialAngle {
    /// Azimuth angle in degrees (-90.0° to +90.0°, boresight = 0.0°).
    pub azimuth_deg: f64,
    /// Elevation angle in degrees (-30.0° to +30.0°, downtilt is positive or negative).
    pub elevation_deg: f64,
}

impl SpatialAngle {
    pub fn new(azimuth_deg: f64, elevation_deg: f64) -> Self {
        Self {
            azimuth_deg,
            elevation_deg,
        }
    }
}

/// Beam weight vector for a single spatial beam.
#[derive(Debug, Clone, PartialEq)]
pub struct BeamWeightVector {
    pub beam_id: u16,
    pub weights: Vec<ComplexNumber>,
}

impl BeamWeightVector {
    /// Normalizes the weight vector to unit power sum: ||w||^2 = 1.0.
    pub fn normalize_unit_power(&mut self) {
        let sum_sq: f64 = self.weights.iter().map(|w| w.norm_sq()).sum();
        if sum_sq > 0.0 {
            let inv_norm = 1.0 / sum_sq.sqrt();
            for w in &mut self.weights {
                *w = w.scale(inv_norm);
            }
        }
    }

    /// Computes total radiated power.
    pub fn total_power(&self) -> f64 {
        self.weights.iter().map(|w| w.norm_sq()).sum()
    }

    /// Quantizes complex floating weights into `BfwBundle` for Section Extension 1.
    ///
    /// Supports Block Floating Point (BFP) and fixed-point 16-bit or 8-bit integers.
    pub fn quantize(&self, method: BfwCompressionMethod, bit_width: u8) -> BfwBundle {
        let max_val = self
            .weights
            .iter()
            .map(|w| w.re.abs().max(w.im.abs()))
            .fold(0.0f64, |a, b| a.max(b));

        let scale = if max_val > 0.0 {
            let max_int = match bit_width {
                8 => 127.0,
                _ => 32767.0,
            };
            max_int / max_val
        } else {
            1.0
        };

        let exponent = if method == BfwCompressionMethod::BlockFloatingPoint {
            // Compute 4-bit BFP exponent based on peak amplitude
            let bits_needed = if max_val > 0.0 {
                (max_val * 32767.0).log2().ceil() as i32
            } else {
                0
            };
            (16 - bits_needed.clamp(1, 16)) as u8
        } else {
            0
        };

        let bfw_weights: Vec<BfwWeight> = self
            .weights
            .iter()
            .map(|w| {
                let re = (w.re * scale).round().clamp(-32768.0, 32767.0) as i16;
                let im = (w.im * scale).round().clamp(-32768.0, 32767.0) as i16;
                BfwWeight::new(re, im)
            })
            .collect();

        BfwBundle::new(exponent, bfw_weights)
    }
}

// ---------------------------------------------------------------------------
// 3D Grid-of-Beams (GoB) Codebook
// ---------------------------------------------------------------------------

/// 3D Grid-of-Beams Codebook containing pre-computed spatial steering weights.
#[derive(Debug, Clone)]
pub struct GridOfBeamsCodebook {
    pub beams: Vec<BeamWeightVector>,
    pub angles: Vec<SpatialAngle>,
}

impl GridOfBeamsCodebook {
    /// Generates a standard Grid-of-Beams (GoB) codebook covering a sector.
    pub fn generate(
        array_cfg: &AntennaArrayConfig,
        num_azimuth_beams: usize,
        num_elevation_beams: usize,
        azimuth_span_deg: (f64, f64),
        elevation_span_deg: (f64, f64),
    ) -> Self {
        let mut beams = Vec::new();
        let mut angles = Vec::new();
        let mut beam_id: u16 = 0;

        let az_step = if num_azimuth_beams > 1 {
            (azimuth_span_deg.1 - azimuth_span_deg.0) / (num_azimuth_beams - 1) as f64
        } else {
            0.0
        };

        let el_step = if num_elevation_beams > 1 {
            (elevation_span_deg.1 - elevation_span_deg.0) / (num_elevation_beams - 1) as f64
        } else {
            0.0
        };

        for el_idx in 0..num_elevation_beams {
            let el = elevation_span_deg.0 + (el_idx as f64) * el_step;
            for az_idx in 0..num_azimuth_beams {
                let az = azimuth_span_deg.0 + (az_idx as f64) * az_step;
                let angle = SpatialAngle::new(az, el);

                let weights = compute_steering_vector(array_cfg, &angle);
                let mut vec = BeamWeightVector { beam_id, weights };
                vec.normalize_unit_power();

                beams.push(vec);
                angles.push(angle);
                beam_id += 1;
            }
        }

        Self { beams, angles }
    }

    /// Finds the closest beam in the codebook for a given spatial direction.
    pub fn find_nearest_beam(&self, target_angle: &SpatialAngle) -> Option<&BeamWeightVector> {
        if self.angles.is_empty() {
            return None;
        }

        let mut best_idx = 0;
        let mut min_dist_sq = f64::MAX;

        for (idx, angle) in self.angles.iter().enumerate() {
            let d_az = angle.azimuth_deg - target_angle.azimuth_deg;
            let d_el = angle.elevation_deg - target_angle.elevation_deg;
            let dist_sq = d_az * d_az + d_el * d_el;
            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                best_idx = idx;
            }
        }

        self.beams.get(best_idx)
    }

    /// Retrieves a beam by its exact beam_id.
    pub fn get_beam(&self, beam_id: u16) -> Option<&BeamWeightVector> {
        self.beams.get(beam_id as usize)
    }
}

/// Computes the complex steering vector for the specified array and angle.
pub fn compute_steering_vector(
    config: &AntennaArrayConfig,
    angle: &SpatialAngle,
) -> Vec<ComplexNumber> {
    let az_rad = angle.azimuth_deg.to_radians();
    let el_rad = angle.elevation_deg.to_radians();

    // Wave vector components in spherical coordinates:
    // k_x = 2pi * sin(az) * cos(el)
    // k_y = 2pi * sin(el)
    let k_x = 2.0 * PI * az_rad.sin() * el_rad.cos();
    let k_y = 2.0 * PI * el_rad.sin();

    let mut spatial_weights = Vec::new();

    match &config.topology {
        ArrayTopology::UniformLinearArray {
            num_elements,
            element_spacing_wavelength,
        } => {
            for m in 0..*num_elements {
                let x_pos = (m as f64) * element_spacing_wavelength;
                let phase = k_x * x_pos;
                spatial_weights.push(ComplexNumber::from_polar(1.0, phase));
            }
        }
        ArrayTopology::UniformPlanarArray {
            num_cols,
            num_rows,
            col_spacing_wavelength,
            row_spacing_wavelength,
        } => {
            for row in 0..*num_rows {
                let y_pos = (row as f64) * row_spacing_wavelength;
                for col in 0..*num_cols {
                    let x_pos = (col as f64) * col_spacing_wavelength;
                    let phase = k_x * x_pos + k_y * y_pos;
                    spatial_weights.push(ComplexNumber::from_polar(1.0, phase));
                }
            }
        }
    }

    // Expand for dual polarization if configured (+45° and -45°)
    match config.polarization {
        AntennaPolarization::SinglePol => spatial_weights,
        AntennaPolarization::DualPolCross45 => {
            let mut full_weights = Vec::with_capacity(spatial_weights.len() * 2);
            // Pol 1 (+45°): direct steering vector
            full_weights.extend_from_slice(&spatial_weights);
            // Pol 2 (-45°): cross-pol reference phase (pi / 2 phase offset)
            for w in &spatial_weights {
                full_weights.push(w.mul(&ComplexNumber::from_polar(1.0, PI / 2.0)));
            }
            full_weights
        }
    }
}

// ---------------------------------------------------------------------------
// Multi-User MIMO (MU-MIMO) Zero-Forcing Precoding
// ---------------------------------------------------------------------------

/// Result of Multi-User MIMO Precoding computation.
#[derive(Debug, Clone)]
pub struct MuMimoPrecodingResult {
    /// Precoding weight vector per user (K vectors of length M).
    pub user_weights: Vec<BeamWeightVector>,
    /// Achieved Signal-to-Interference Ratio (SIR) per user in dB.
    pub user_sir_db: Vec<f64>,
    /// Effective channel matrix (H * W) of size K x K.
    pub effective_channel: Vec<Vec<ComplexNumber>>,
}

/// Multi-User MIMO Zero-Forcing (ZF) and Regularized ZF (MMSE) Precoding Engine.
pub struct MuMimoPrecoder;

impl MuMimoPrecoder {
    /// Computes Zero-Forcing or Regularized ZF (MMSE) precoding weights:
    ///
    /// W = H^H * (H * H^H + alpha * I)^(-1)
    ///
    /// Where:
    /// - H is the channel matrix of size K x M (K = users, M = antennas, K <= M).
    /// - alpha >= 0.0 is the regularization factor (alpha = 0.0 for pure ZF).
    pub fn compute_precoding(
        channel_matrix_h: &[Vec<ComplexNumber>],
        alpha_regularization: f64,
    ) -> Result<MuMimoPrecodingResult, &'static str> {
        let k = channel_matrix_h.len();
        if k == 0 {
            return Err("Channel matrix must have at least one user");
        }
        let m = channel_matrix_h[0].len();
        if m < k {
            return Err("Number of antenna elements M must be >= number of users K");
        }
        for row in channel_matrix_h {
            if row.len() != m {
                return Err("Channel matrix rows have inconsistent lengths");
            }
        }

        // 1. Compute Gram Matrix G = H * H^H + alpha * I (size K x K)
        let mut g = vec![vec![ComplexNumber::ZERO; k]; k];
        for i in 0..k {
            for j in 0..k {
                let mut sum = ComplexNumber::ZERO;
                for col in 0..m {
                    let h_i = channel_matrix_h[i][col];
                    let h_j_conj = channel_matrix_h[j][col].conj();
                    sum = sum.add(&h_i.mul(&h_j_conj));
                }
                if i == j {
                    sum = sum.add(&ComplexNumber::new(alpha_regularization, 0.0));
                }
                g[i][j] = sum;
            }
        }

        // 2. Invert Gram Matrix G -> G_inv (size K x K) using Gauss-Jordan with partial pivoting
        let g_inv = complex_matrix_invert(&g)?;

        // 3. Compute W = H^H * G_inv (size M x K)
        // Column k of W is user k's precoding vector (length M)
        let mut user_weights = Vec::with_capacity(k);
        for u in 0..k {
            let mut weights = Vec::with_capacity(m);
            for ant in 0..m {
                let mut sum = ComplexNumber::ZERO;
                for j in 0..k {
                    let h_j_ant_conj = channel_matrix_h[j][ant].conj();
                    let g_inv_elem = g_inv[j][u];
                    sum = sum.add(&h_j_ant_conj.mul(&g_inv_elem));
                }
                weights.push(sum);
            }

            let mut beam = BeamWeightVector {
                beam_id: u as u16,
                weights,
            };
            beam.normalize_unit_power();
            user_weights.push(beam);
        }

        // 4. Compute Effective Channel H_eff = H * W (size K x K) and evaluate SIR
        let mut effective_channel = vec![vec![ComplexNumber::ZERO; k]; k];
        let mut user_sir_db = Vec::with_capacity(k);

        for i in 0..k {
            for j in 0..k {
                let mut val = ComplexNumber::ZERO;
                for ant in 0..m {
                    let h_i_ant = channel_matrix_h[i][ant];
                    let w_ant_j = user_weights[j].weights[ant];
                    val = val.add(&h_i_ant.mul(&w_ant_j));
                }
                effective_channel[i][j] = val;
            }

            let signal_pwr = effective_channel[i][i].norm_sq();
            let mut interf_pwr = 0.0;
            for j in 0..k {
                if i != j {
                    interf_pwr += effective_channel[i][j].norm_sq();
                }
            }

            let sir_db = if interf_pwr > 1e-15 {
                10.0 * (signal_pwr / interf_pwr).log10()
            } else {
                120.0 // Near-infinite SIR for perfect ZF nulling
            };
            user_sir_db.push(sir_db);
        }

        Ok(MuMimoPrecodingResult {
            user_weights,
            user_sir_db,
            effective_channel,
        })
    }
}

/// Inverts a K x K complex matrix using Gauss-Jordan elimination with partial pivoting.
fn complex_matrix_invert(
    matrix: &[Vec<ComplexNumber>],
) -> Result<Vec<Vec<ComplexNumber>>, &'static str> {
    let n = matrix.len();
    if n == 0 {
        return Err("Matrix is empty");
    }

    // Augmented matrix [A | I]
    let mut aug = vec![vec![ComplexNumber::ZERO; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = matrix[i][j];
        }
        aug[i][n + i] = ComplexNumber::ONE;
    }

    for col in 0..n {
        // Partial pivoting: find row with largest pivot magnitude
        let mut max_row = col;
        let mut max_val = aug[col][col].norm_sq();
        for r in (col + 1)..n {
            let val = aug[r][col].norm_sq();
            if val > max_val {
                max_val = val;
                max_row = r;
            }
        }

        if max_val < 1e-20 {
            return Err("Matrix is singular or ill-conditioned");
        }

        if max_row != col {
            aug.swap(col, max_row);
        }

        // Scale pivot row so pivot element is 1.0
        let pivot = aug[col][col];
        for j in 0..(2 * n) {
            aug[col][j] = aug[col][j].div(&pivot);
        }

        // Eliminate column entries in all other rows
        for r in 0..n {
            if r != col {
                let factor = aug[r][col];
                for j in 0..(2 * n) {
                    let sub_val = factor.mul(&aug[col][j]);
                    aug[r][j] = aug[r][j].sub(&sub_val);
                }
            }
        }
    }

    // Extract right-hand half [I | A^(-1)]
    let mut inv = vec![vec![ComplexNumber::ZERO; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }

    Ok(inv)
}

// ---------------------------------------------------------------------------
// Top-Level O-RAN Beamforming Engine
// ---------------------------------------------------------------------------

/// Performance and health metrics for O-RAN Beamforming Subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct BeamformingTelemetry {
    pub total_beams_generated: u64,
    pub total_cplane_ext1_packets: u64,
    pub total_ext2_attributes_mapped: u64,
    pub active_codebook_size: usize,
    pub peak_antenna_power_ratio: f64,
}

/// Top-Level O-RAN WG4 Open Fronthaul Beamforming Manager.
pub struct OranBeamformingEngine {
    pub array_config: AntennaArrayConfig,
    pub codebook: GridOfBeamsCodebook,
    pub telemetry: BeamformingTelemetry,
}

impl OranBeamformingEngine {
    /// Creates a new beamforming engine with pre-computed Grid-of-Beams.
    pub fn new(array_config: AntennaArrayConfig) -> Self {
        // Pre-compute standard 8x4 = 32 beam GoB codebook spanning sector
        let codebook =
            GridOfBeamsCodebook::generate(&array_config, 8, 4, (-60.0, 60.0), (-15.0, 15.0));

        let codebook_size = codebook.beams.len();

        Self {
            array_config,
            codebook,
            telemetry: BeamformingTelemetry {
                total_beams_generated: 0,
                total_cplane_ext1_packets: 0,
                total_ext2_attributes_mapped: 0,
                active_codebook_size: codebook_size,
                peak_antenna_power_ratio: 1.0,
            },
        }
    }

    /// Converts C-Plane Section Extension 2 (Beam Attributes: Azimuth & Elevation)
    /// into Section Extension 1 (Quantized Beamforming Weights).
    pub fn convert_ext2_to_ext1(
        &mut self,
        ext2: &SectionExtension2,
        comp_method: BfwCompressionMethod,
        bit_width: u8,
    ) -> SectionExtension1 {
        self.telemetry.total_ext2_attributes_mapped += 1;
        self.telemetry.total_beams_generated += 1;

        let target_angle = SpatialAngle::new(ext2.azimuth_deg as f64, ext2.elevation_deg as f64);
        let weights = compute_steering_vector(&self.array_config, &target_angle);
        let mut beam = BeamWeightVector {
            beam_id: ext2.bf_id,
            weights,
        };
        beam.normalize_unit_power();

        let bundle = beam.quantize(comp_method, bit_width);

        self.telemetry.total_cplane_ext1_packets += 1;
        SectionExtension1::new(comp_method, bit_width, vec![bundle])
    }

    /// Evaluates Peak-to-Average Power Ratio (PAPR) across antenna elements
    /// to safeguard Power Amplifiers (PAs) from non-linear distortion.
    pub fn evaluate_pa_power_balance(&mut self, beam: &BeamWeightVector) -> f64 {
        if beam.weights.is_empty() {
            return 1.0;
        }

        let powers: Vec<f64> = beam.weights.iter().map(|w| w.norm_sq()).collect();
        let max_pwr = powers.iter().fold(0.0f64, |a, &b| a.max(b));
        let avg_pwr = powers.iter().sum::<f64>() / (powers.len() as f64);

        let papr = if avg_pwr > 0.0 {
            max_pwr / avg_pwr
        } else {
            1.0
        };

        self.telemetry.peak_antenna_power_ratio = papr;
        papr
    }
}
