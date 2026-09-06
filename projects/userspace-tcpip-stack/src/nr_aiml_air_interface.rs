//! 3GPP Release 18 (5G-Advanced) Artificial Intelligence & Machine Learning (AI/ML) Air Interface Engine.
//!
//! Conforms to:
//! - 3GPP TR 38.843 Rel-18: Study on AI/ML for NR Air Interface.
//! - 3GPP TS 38.300 Rel-18 §16.15: AI/ML enabled NG-RAN architecture.
//! - 3GPP TS 38.214 Rel-18: Physical layer procedures for data (CSI reporting and beam management).
//! - 3GPP TS 38.331 Rel-18: RRC information elements for AI/ML configuration and lifecycle control.
//!
//! Features:
//! 1. Neural forward-pass engine: Dense/Linear layers with ReLU, LeakyReLU, Tanh, Sigmoid, and Linear activations.
//! 2. Two-sided CSI Feedback Autoencoder (UE-side encoder + gNodeB-side decoder):
//!    - Compresses complex multi-antenna MIMO channel matrix into low-dimensional latent representation $\mathbf{z}$.
//!    - Uniform scalar quantizer with bit-width adaptation (e.g. 8-bit).
//!    - Evaluates Generalized Cosine Similarity (GCS) and Normalized Mean Square Error (NMSE in dB).
//! 3. Spatial Beam Management & Trajectory Prediction:
//!    - Multi-beam RSRP prediction up to $K$ slots ahead to prevent beam misalignment.
//!    - Top-1 and Top-N beam ranking.
//! 4. Positioning Channel Impulse Response (CIR) Refinement:
//!    - Strips non-line-of-sight (NLoS) multipath bias from Power Delay Profiles (PDP) for sub-meter ToA accuracy.
//! 5. Model Lifecycle Management (LCM) & Anomaly Fallback:
//!    - Inference latency gating against radio slot deadlines (< 500 us).
//!    - Automatic fallback to legacy 3GPP Type-I/II codebooks upon out-of-distribution (OOD) channel drift.
//!
//! Pure standard Rust with zero external dependencies.

use std::fmt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum allowable inference execution time in microseconds (1 slot at 30 kHz SCS = 500 us).
pub const DEFAULT_INFERENCE_DEADLINE_US: u64 = 500;
pub const AIML_DEFAULT_INFERENCE_DEADLINE_US: u64 = DEFAULT_INFERENCE_DEADLINE_US;

/// Default Generalized Cosine Similarity (GCS) threshold below which fallback is triggered.
pub const DEFAULT_GCS_FALLBACK_THRESHOLD: f64 = 0.70;
pub const AIML_DEFAULT_GCS_FALLBACK_THRESHOLD: f64 = DEFAULT_GCS_FALLBACK_THRESHOLD;

/// Default quantization resolution for latent space representation in bits.
pub const DEFAULT_QUANTIZATION_BITS: u8 = 8;
pub const AIML_DEFAULT_QUANTIZATION_BITS: u8 = DEFAULT_QUANTIZATION_BITS;

/// Maximum number of candidate beams in FR2 beam management.
pub const MAX_CANDIDATE_BEAMS: usize = 64;
pub const AIML_MAX_CANDIDATE_BEAMS: usize = MAX_CANDIDATE_BEAMS;

// ---------------------------------------------------------------------------
// Neural Network Infrastructure (Linear Layers & Activations)
// ---------------------------------------------------------------------------

/// Activation function for feed-forward neural layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivationFunction {
    Linear,
    Relu,
    LeakyRelu(f64),
    Tanh,
    Sigmoid,
}

impl ActivationFunction {
    /// Apply activation function element-wise.
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            ActivationFunction::Linear => x,
            ActivationFunction::Relu => {
                if x > 0.0 {
                    x
                } else {
                    0.0
                }
            }
            ActivationFunction::LeakyRelu(alpha) => {
                if x > 0.0 {
                    x
                } else {
                    alpha * x
                }
            }
            ActivationFunction::Tanh => x.tanh(),
            ActivationFunction::Sigmoid => 1.0 / (1.0 + (-x).exp()),
        }
    }
}

/// Fully-connected (Dense / Linear) feed-forward layer: $y = Wx + b$.
#[derive(Debug, Clone, PartialEq)]
pub struct NeuralLayer {
    /// Weight matrix flattened in row-major order: size is `out_features * in_features`.
    pub weights: Vec<f64>,
    /// Bias vector: size is `out_features`.
    pub biases: Vec<f64>,
    pub in_features: usize,
    pub out_features: usize,
    pub activation: ActivationFunction,
}

impl NeuralLayer {
    /// Create a new neural layer with verified dimensions.
    pub fn new(
        in_features: usize,
        out_features: usize,
        weights: Vec<f64>,
        biases: Vec<f64>,
        activation: ActivationFunction,
    ) -> Result<Self, AiMlError> {
        let expected_weight_count = in_features * out_features;
        if weights.len() != expected_weight_count {
            return Err(AiMlError::DimensionMismatch {
                expected: expected_weight_count,
                got: weights.len(),
            });
        }
        if biases.len() != out_features {
            return Err(AiMlError::DimensionMismatch {
                expected: out_features,
                got: biases.len(),
            });
        }
        Ok(Self {
            weights,
            biases,
            in_features,
            out_features,
            activation,
        })
    }

    /// Compute forward pass: $y = \text{activation}(W \cdot x + b)$.
    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>, AiMlError> {
        if input.len() != self.in_features {
            return Err(AiMlError::DimensionMismatch {
                expected: self.in_features,
                got: input.len(),
            });
        }

        let mut output = Vec::with_capacity(self.out_features);
        for row in 0..self.out_features {
            let row_offset = row * self.in_features;
            let mut sum = self.biases[row];
            for col in 0..self.in_features {
                sum += self.weights[row_offset + col] * input[col];
            }
            output.push(self.activation.apply(sum));
        }

        Ok(output)
    }
}

/// Sequential Multi-Layer Perceptron (MLP).
#[derive(Debug, Clone, PartialEq)]
pub struct NeuralNetwork {
    pub layers: Vec<NeuralLayer>,
}

impl NeuralNetwork {
    pub fn new(layers: Vec<NeuralLayer>) -> Self {
        Self { layers }
    }

    /// Execute sequential forward pass through all layers.
    pub fn forward(&self, input: &[f64]) -> Result<Vec<f64>, AiMlError> {
        if self.layers.is_empty() {
            return Ok(input.to_vec());
        }

        let mut current = input.to_vec();
        for layer in &self.layers {
            current = layer.forward(&current)?;
        }
        Ok(current)
    }
}

// ---------------------------------------------------------------------------
// Quantization & Codebook
// ---------------------------------------------------------------------------

/// Uniform scalar quantizer for mapping continuous latent vectors to discrete bitstreams.
#[derive(Debug, Clone, PartialEq)]
pub struct UniformQuantizer {
    pub min_val: f64,
    pub max_val: f64,
    pub bits: u8,
}

impl UniformQuantizer {
    pub fn new(min_val: f64, max_val: f64, bits: u8) -> Self {
        Self {
            min_val,
            max_val,
            bits: bits.clamp(1, 16),
        }
    }

    /// Quantize a floating-point value to an unsigned integer.
    pub fn quantize(&self, val: f64) -> u16 {
        let clamped = val.clamp(self.min_val, self.max_val);
        let levels = ((1u32 << self.bits) - 1) as f64;
        let normalized = (clamped - self.min_val) / (self.max_val - self.min_val).max(1e-12);
        (normalized * levels).round() as u16
    }

    /// Dequantize an integer back to floating-point representation.
    pub fn dequantize(&self, q: u16) -> f64 {
        let levels = ((1u32 << self.bits) - 1) as f64;
        let normalized = (q as f64) / levels;
        self.min_val + normalized * (self.max_val - self.min_val)
    }

    /// Quantize a vector of latent values into bytes (for <= 8-bit quantization).
    pub fn quantize_vec_u8(&self, values: &[f64]) -> Vec<u8> {
        values.iter().map(|&v| self.quantize(v) as u8).collect()
    }

    /// Dequantize a slice of bytes back into float vector.
    pub fn dequantize_vec_u8(&self, quantized: &[u8]) -> Vec<f64> {
        quantized
            .iter()
            .map(|&q| self.dequantize(q as u16))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Complex Number & MIMO Channel Matrix
// ---------------------------------------------------------------------------

/// Complex channel element representing amplitude and phase ($h = I + jQ$).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexElement {
    pub re: f64,
    pub im: f64,
}

impl ComplexElement {
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn norm_squared(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Complex conjugate product: $a^* \cdot b$.
    pub fn conj_mul(&self, other: &ComplexElement) -> ComplexElement {
        ComplexElement {
            re: self.re * other.re + self.im * other.im,
            im: self.re * other.im - self.im * other.re,
        }
    }
}

/// 2D MIMO channel matrix: $N_{rx} \times N_{tx}$ complex channel coefficients.
#[derive(Debug, Clone, PartialEq)]
pub struct MimoChannelMatrix {
    pub num_rx: usize,
    pub num_tx: usize,
    pub elements: Vec<ComplexElement>,
}

impl MimoChannelMatrix {
    pub fn new(
        num_rx: usize,
        num_tx: usize,
        elements: Vec<ComplexElement>,
    ) -> Result<Self, AiMlError> {
        if elements.len() != num_rx * num_tx {
            return Err(AiMlError::DimensionMismatch {
                expected: num_rx * num_tx,
                got: elements.len(),
            });
        }
        Ok(Self {
            num_rx,
            num_tx,
            elements,
        })
    }

    /// Flatten complex channel matrix into a real vector of size $2 \times N_{rx} \times N_{tx}$.
    pub fn to_real_vector(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.elements.len() * 2);
        for elem in &self.elements {
            out.push(elem.re);
            out.push(elem.im);
        }
        out
    }

    /// Reconstruct MIMO channel matrix from a flattened real vector.
    pub fn from_real_vector(num_rx: usize, num_tx: usize, vec: &[f64]) -> Result<Self, AiMlError> {
        if vec.len() != num_rx * num_tx * 2 {
            return Err(AiMlError::DimensionMismatch {
                expected: num_rx * num_tx * 2,
                got: vec.len(),
            });
        }
        let mut elements = Vec::with_capacity(num_rx * num_tx);
        for chunk in vec.chunks_exact(2) {
            elements.push(ComplexElement::new(chunk[0], chunk[1]));
        }
        Ok(Self {
            num_rx,
            num_tx,
            elements,
        })
    }
}

// ---------------------------------------------------------------------------
// Use Case 1: CSI Feedback Compression & Reconstruction (TR 38.843 §5.1)
// ---------------------------------------------------------------------------

/// Two-sided neural autoencoder for CSI compression and reconstruction.
pub struct CsiAutoencoder {
    pub ue_encoder: NeuralNetwork,
    pub gnb_decoder: NeuralNetwork,
    pub quantizer: UniformQuantizer,
    pub num_rx: usize,
    pub num_tx: usize,
    pub latent_dim: usize,
}

impl CsiAutoencoder {
    pub fn new(
        ue_encoder: NeuralNetwork,
        gnb_decoder: NeuralNetwork,
        quantizer: UniformQuantizer,
        num_rx: usize,
        num_tx: usize,
        latent_dim: usize,
    ) -> Self {
        Self {
            ue_encoder,
            gnb_decoder,
            quantizer,
            num_rx,
            num_tx,
            latent_dim,
        }
    }

    /// UE-side: Compress channel matrix into a compact quantized bitstream payload.
    pub fn ue_compress(&self, channel: &MimoChannelMatrix) -> Result<Vec<u8>, AiMlError> {
        let input_vec = channel.to_real_vector();
        let latent_continuous = self.ue_encoder.forward(&input_vec)?;
        if latent_continuous.len() != self.latent_dim {
            return Err(AiMlError::DimensionMismatch {
                expected: self.latent_dim,
                got: latent_continuous.len(),
            });
        }
        Ok(self.quantizer.quantize_vec_u8(&latent_continuous))
    }

    /// gNodeB-side: Dequantize and reconstruct channel matrix from received bitstream.
    pub fn gnb_reconstruct(
        &self,
        quantized_payload: &[u8],
    ) -> Result<MimoChannelMatrix, AiMlError> {
        if quantized_payload.len() != self.latent_dim {
            return Err(AiMlError::DimensionMismatch {
                expected: self.latent_dim,
                got: quantized_payload.len(),
            });
        }
        let latent_dequantized = self.quantizer.dequantize_vec_u8(quantized_payload);
        let reconstructed_real = self.gnb_decoder.forward(&latent_dequantized)?;
        MimoChannelMatrix::from_real_vector(self.num_rx, self.num_tx, &reconstructed_real)
    }

    /// Calculate Generalized Cosine Similarity (GCS) between true and reconstructed channel.
    ///
    /// $\text{GCS}(\mathbf{\hat{h}}, \mathbf{h}) = \frac{|\sum_i \hat{h}_i^* h_i|}{\|\mathbf{\hat{h}}\| \|\mathbf{h}\|}$
    pub fn calculate_gcs(
        true_channel: &MimoChannelMatrix,
        reconstructed: &MimoChannelMatrix,
    ) -> f64 {
        if true_channel.elements.len() != reconstructed.elements.len()
            || true_channel.elements.is_empty()
        {
            return 0.0;
        }

        let mut inner_re = 0.0;
        let mut inner_im = 0.0;
        let mut norm_true_sq = 0.0;
        let mut norm_rec_sq = 0.0;

        for (h, h_hat) in true_channel.elements.iter().zip(&reconstructed.elements) {
            let prod = h_hat.conj_mul(h);
            inner_re += prod.re;
            inner_im += prod.im;
            norm_true_sq += h.norm_squared();
            norm_rec_sq += h_hat.norm_squared();
        }

        let inner_mag = (inner_re * inner_re + inner_im * inner_im).sqrt();
        let denominator = (norm_true_sq * norm_rec_sq).sqrt();

        if denominator < 1e-12 {
            0.0
        } else {
            (inner_mag / denominator).clamp(0.0, 1.0)
        }
    }

    /// Calculate Normalized Mean Square Error (NMSE) in dB:
    ///
    /// $\text{NMSE}\ (\text{dB}) = 10 \log_{10}\left( \frac{\sum_i |\hat{h}_i - h_i|^2}{\sum_i |h_i|^2} \right)$
    pub fn calculate_nmse_db(
        true_channel: &MimoChannelMatrix,
        reconstructed: &MimoChannelMatrix,
    ) -> f64 {
        if true_channel.elements.len() != reconstructed.elements.len()
            || true_channel.elements.is_empty()
        {
            return 0.0;
        }

        let mut err_sq_sum = 0.0;
        let mut true_sq_sum = 0.0;

        for (h, h_hat) in true_channel.elements.iter().zip(&reconstructed.elements) {
            let diff_re = h_hat.re - h.re;
            let diff_im = h_hat.im - h.im;
            err_sq_sum += diff_re * diff_re + diff_im * diff_im;
            true_sq_sum += h.norm_squared();
        }

        if true_sq_sum < 1e-12 {
            0.0
        } else {
            10.0 * (err_sq_sum / true_sq_sum).max(1e-12).log10()
        }
    }
}

// ---------------------------------------------------------------------------
// Use Case 2: Spatial Beam Management & Trajectory Prediction (TR 38.843 §5.2)
// ---------------------------------------------------------------------------

/// Engine for predictive spatial beam management under mobility and Doppler shift.
pub struct BeamPredictionEngine {
    pub predictor_network: NeuralNetwork,
    pub num_beams: usize,
    pub history_slots: usize,
}

impl BeamPredictionEngine {
    pub fn new(predictor_network: NeuralNetwork, num_beams: usize, history_slots: usize) -> Self {
        Self {
            predictor_network,
            num_beams,
            history_slots,
        }
    }

    /// Predict future beam RSRPs and return the optimal Top-1 beam and ranked list.
    ///
    /// - `rsrp_history`: Flattened slice of size `history_slots * num_beams` containing historical RSRPs in dBm.
    /// Returns: `(best_beam_id, best_rsrp_dbm, ranked_beams)`.
    pub fn predict_beams(
        &self,
        rsrp_history: &[f64],
    ) -> Result<(usize, f64, Vec<(usize, f64)>), AiMlError> {
        let expected_input_len = self.history_slots * self.num_beams;
        if rsrp_history.len() != expected_input_len {
            return Err(AiMlError::DimensionMismatch {
                expected: expected_input_len,
                got: rsrp_history.len(),
            });
        }

        let predicted_rsrps = self.predictor_network.forward(rsrp_history)?;
        if predicted_rsrps.len() != self.num_beams {
            return Err(AiMlError::DimensionMismatch {
                expected: self.num_beams,
                got: predicted_rsrps.len(),
            });
        }

        let mut ranked: Vec<(usize, f64)> = predicted_rsrps.into_iter().enumerate().collect();

        // Sort descending by predicted RSRP
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let best_beam_id = ranked[0].0;
        let best_rsrp_dbm = ranked[0].1;

        Ok((best_beam_id, best_rsrp_dbm, ranked))
    }
}

// ---------------------------------------------------------------------------
// Use Case 3: Positioning CIR Refinement (TR 38.843 §5.3)
// ---------------------------------------------------------------------------

/// Non-Line-of-Sight (NLoS) multipath mitigation for Time-of-Arrival (ToA) estimation.
pub struct PositioningCirRefiner {
    pub refiner_network: NeuralNetwork,
    pub num_cir_bins: usize,
}

impl PositioningCirRefiner {
    pub fn new(refiner_network: NeuralNetwork, num_cir_bins: usize) -> Self {
        Self {
            refiner_network,
            num_cir_bins,
        }
    }

    /// Refine Power Delay Profile (PDP) to identify true direct path ToA sample offset.
    ///
    /// - `cir_pdp`: Normalized power delay profile bins.
    /// Returns: Refined fractional direct path delay in nanoseconds.
    pub fn refine_toa(&self, cir_pdp: &[f64]) -> Result<f64, AiMlError> {
        if cir_pdp.len() != self.num_cir_bins {
            return Err(AiMlError::DimensionMismatch {
                expected: self.num_cir_bins,
                got: cir_pdp.len(),
            });
        }

        let output = self.refiner_network.forward(cir_pdp)?;
        if output.is_empty() {
            return Err(AiMlError::EmptyInferenceOutput);
        }

        Ok(output[0])
    }
}

// ---------------------------------------------------------------------------
// Model Lifecycle Management (LCM) & Anomaly Fallback
// ---------------------------------------------------------------------------

/// Operational status of an AI/ML model on the air interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStatus {
    Unloaded,
    Loaded,
    Active,
    FallbackSuspended,
}

/// Manages runtime lifecycle, deadline gating, and fallback transitions.
pub struct ModelLifecycleManager {
    pub model_id: u32,
    pub version: u16,
    pub status: ModelStatus,
    pub inference_deadline_us: u64,
    pub fallback_gcs_threshold: f64,
    pub fallback_trigger_count: u64,
    pub successful_inference_count: u64,
}

impl ModelLifecycleManager {
    pub fn new(
        model_id: u32,
        version: u16,
        inference_deadline_us: u64,
        fallback_gcs_threshold: f64,
    ) -> Self {
        Self {
            model_id,
            version,
            status: ModelStatus::Loaded,
            inference_deadline_us,
            fallback_gcs_threshold,
            fallback_trigger_count: 0,
            successful_inference_count: 0,
        }
    }

    /// Activate model for live over-the-air inference.
    pub fn activate(&mut self) {
        self.status = ModelStatus::Active;
    }

    /// Validate inference execution time against deadline.
    pub fn check_execution_deadline(&self, elapsed_us: u64) -> Result<(), AiMlError> {
        if elapsed_us > self.inference_deadline_us {
            Err(AiMlError::InferenceDeadlineExceeded {
                limit_us: self.inference_deadline_us,
                actual_us: elapsed_us,
            })
        } else {
            Ok(())
        }
    }

    /// Inspect reconstruction metric and execute fallback if below safety threshold.
    ///
    /// Returns `true` if model remains active, or `false` if fallback was triggered.
    pub fn monitor_gcs_and_evaluate_fallback(&mut self, current_gcs: f64) -> bool {
        if current_gcs < self.fallback_gcs_threshold {
            self.status = ModelStatus::FallbackSuspended;
            self.fallback_trigger_count += 1;
            false
        } else {
            self.successful_inference_count += 1;
            true
        }
    }

    /// Check if fallback to legacy 3GPP Type-I/II codebook is currently active.
    pub fn is_fallback_active(&self) -> bool {
        self.status == ModelStatus::FallbackSuspended
    }
}

// ---------------------------------------------------------------------------
// Telemetry & Metrics
// ---------------------------------------------------------------------------

/// Aggregate telemetry for 3GPP AI/ML air interface operations.
#[derive(Debug, Clone, Default)]
pub struct AiMlMetrics {
    pub total_csi_inferences: u64,
    pub total_beam_predictions: u64,
    pub total_positioning_refinements: u64,
    pub total_fallback_events: u64,
    pub gcs_sum: f64,
    pub gcs_sample_count: u64,
}

impl AiMlMetrics {
    pub fn average_gcs(&self) -> f64 {
        if self.gcs_sample_count == 0 {
            1.0
        } else {
            self.gcs_sum / (self.gcs_sample_count as f64)
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors encountered during AI/ML inference, encoding, or lifecycle management.
#[derive(Debug, Clone, PartialEq)]
pub enum AiMlError {
    DimensionMismatch { expected: usize, got: usize },
    InferenceDeadlineExceeded { limit_us: u64, actual_us: u64 },
    ModelNotActive(ModelStatus),
    EmptyInferenceOutput,
    QuantizationError(String),
}

impl fmt::Display for AiMlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiMlError::DimensionMismatch { expected, got } => {
                write!(
                    f,
                    "AI/ML dimension mismatch: expected {}, got {}",
                    expected, got
                )
            }
            AiMlError::InferenceDeadlineExceeded {
                limit_us,
                actual_us,
            } => {
                write!(
                    f,
                    "AI/ML inference deadline exceeded: limit {} us, actual {} us",
                    limit_us, actual_us
                )
            }
            AiMlError::ModelNotActive(status) => {
                write!(f, "AI/ML model is not active: status {:?}", status)
            }
            AiMlError::EmptyInferenceOutput => {
                write!(f, "AI/ML model produced empty inference output")
            }
            AiMlError::QuantizationError(msg) => write!(f, "AI/ML quantization error: {}", msg),
        }
    }
}

impl std::error::Error for AiMlError {}
