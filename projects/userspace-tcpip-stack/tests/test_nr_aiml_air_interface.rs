//! Integration tests for 3GPP Rel-18 5G-Advanced AI/ML Air Interface Engine.
//!
//! Tests neural forward-pass layers, two-sided CSI feedback compression (GCS/NMSE),
//! spatial beam prediction, positioning CIR refinement, and Model Lifecycle Management (LCM)
//! with automated fallback in pure standard Rust.

use toy_tcpip::nr_aiml_air_interface::*;

#[test]
fn test_neural_layer_forward_pass_and_activations() {
    // 1. Test activation functions
    assert_eq!(ActivationFunction::Linear.apply(-2.5), -2.5);
    assert_eq!(ActivationFunction::Relu.apply(-2.5), 0.0);
    assert_eq!(ActivationFunction::Relu.apply(3.5), 3.5);
    assert_eq!(ActivationFunction::LeakyRelu(0.1).apply(-2.0), -0.2);
    assert!((ActivationFunction::Tanh.apply(0.0)).abs() < 1e-6);
    assert!((ActivationFunction::Sigmoid.apply(0.0) - 0.5).abs() < 1e-6);

    // 2. 2-in, 2-out Linear Layer:
    // W = [[1.0, 2.0], [-1.0, 1.0]], b = [0.5, -0.5]
    let layer = NeuralLayer::new(
        2,
        2,
        vec![1.0, 2.0, -1.0, 1.0],
        vec![0.5, -0.5],
        ActivationFunction::Relu,
    )
    .expect("Layer creation must succeed");

    // Input: [2.0, 3.0]
    // y0 = 1.0 * 2.0 + 2.0 * 3.0 + 0.5 = 8.5 -> Relu: 8.5
    // y1 = -1.0 * 2.0 + 1.0 * 3.0 - 0.5 = 0.5 -> Relu: 0.5
    let out = layer
        .forward(&[2.0, 3.0])
        .expect("Forward pass must succeed");
    assert_eq!(out.len(), 2);
    assert!((out[0] - 8.5).abs() < 1e-6);
    assert!((out[1] - 0.5).abs() < 1e-6);

    // 3. Sequential MLP with 2 layers
    let layer2 = NeuralLayer::new(2, 1, vec![0.5, -1.0], vec![1.0], ActivationFunction::Linear)
        .expect("Layer 2 creation must succeed");

    let mlp = NeuralNetwork::new(vec![layer, layer2]);
    // Input: [2.0, 3.0] -> Layer 1: [8.5, 0.5]
    // Layer 2: 0.5 * 8.5 + (-1.0) * 0.5 + 1.0 = 4.25 - 0.5 + 1.0 = 4.75
    let mlp_out = mlp.forward(&[2.0, 3.0]).expect("MLP forward must succeed");
    assert_eq!(mlp_out.len(), 1);
    assert!((mlp_out[0] - 4.75).abs() < 1e-6);
}

#[test]
fn test_two_sided_csi_autoencoder_compression_and_gcs() {
    // 2x2 MIMO channel: 4 complex coefficients = 8 real values
    // Encoder: 8 inputs -> 2 latent dimensions
    // Decoder: 2 latent dimensions -> 8 reconstructed outputs
    let num_rx = 2;
    let num_tx = 2;
    let latent_dim = 2;

    // Simple projection autoencoder weights
    // Encoder: averages pairs of inputs into 2 latent variables
    #[rustfmt::skip]
    let enc_w = vec![
        0.25, 0.25, 0.25, 0.25, 0.00, 0.00, 0.00, 0.00,
        0.00, 0.00, 0.00, 0.00, 0.25, 0.25, 0.25, 0.25,
    ];
    let enc_b = vec![0.0, 0.0];
    let encoder_layer = NeuralLayer::new(8, 2, enc_w, enc_b, ActivationFunction::Linear).unwrap();
    let ue_encoder = NeuralNetwork::new(vec![encoder_layer]);

    // Decoder: expands 2 latent variables back to 8 outputs
    #[rustfmt::skip]
    let dec_w = vec![
        1.0, 0.0,
        1.0, 0.0,
        1.0, 0.0,
        1.0, 0.0,
        0.0, 1.0,
        0.0, 1.0,
        0.0, 1.0,
        0.0, 1.0,
    ];
    let dec_b = vec![0.0; 8];
    let decoder_layer = NeuralLayer::new(2, 8, dec_w, dec_b, ActivationFunction::Linear).unwrap();
    let gnb_decoder = NeuralNetwork::new(vec![decoder_layer]);

    let quantizer = UniformQuantizer::new(-2.0, 2.0, 8);
    let autoencoder = CsiAutoencoder::new(
        ue_encoder,
        gnb_decoder,
        quantizer,
        num_rx,
        num_tx,
        latent_dim,
    );

    // Construct coherent channel matrix
    let true_channel = MimoChannelMatrix::new(
        num_rx,
        num_tx,
        vec![
            ComplexElement::new(0.5, 0.5),
            ComplexElement::new(0.5, 0.5),
            ComplexElement::new(-0.5, -0.5),
            ComplexElement::new(-0.5, -0.5),
        ],
    )
    .unwrap();

    // 1. UE compresses channel matrix to 2 quantized bytes
    let compressed_bytes = autoencoder
        .ue_compress(&true_channel)
        .expect("UE compression must succeed");
    assert_eq!(compressed_bytes.len(), 2);

    // 2. gNodeB reconstructs channel matrix
    let reconstructed = autoencoder
        .gnb_reconstruct(&compressed_bytes)
        .expect("gNB reconstruction must succeed");
    assert_eq!(reconstructed.elements.len(), 4);

    // 3. Generalized Cosine Similarity (GCS)
    let gcs = CsiAutoencoder::calculate_gcs(&true_channel, &reconstructed);
    assert!(
        gcs > 0.95,
        "GCS must exceed 0.95 for coherent reconstruction! Got: {:.4}",
        gcs
    );

    // 4. NMSE in dB
    let nmse_db = CsiAutoencoder::calculate_nmse_db(&true_channel, &reconstructed);
    assert!(
        nmse_db < -15.0,
        "NMSE must be below -15 dB! Got: {:.2} dB",
        nmse_db
    );
}

#[test]
fn test_csi_feedback_overhead_reduction() {
    // 4x4 MIMO: 16 complex = 32 float32 = 128 bytes raw uncompressed CSI
    let raw_bytes = 16 * 2 * 4; // 128 bytes
    let latent_dim = 4; // 4 latent dimensions
    let compressed_bytes = latent_dim; // 4 bytes with 8-bit quantization

    let overhead_reduction = (1.0 - (compressed_bytes as f64 / raw_bytes as f64)) * 100.0;
    assert_eq!(overhead_reduction, 96.875); // 96.875% overhead reduction!
}

#[test]
fn test_spatial_beam_prediction_and_trajectory() {
    // 4 candidate beams, history of 2 slots -> 8 input features
    let num_beams = 4;
    let history_slots = 2;

    // Weight matrix predicting rising trend for Beam 2
    // Let's project input so Beam 2 has highest output
    // Project recent slot t RSRPs (indices 4..7) directly
    #[rustfmt::skip]
    let weights = vec![
        0.0, 0.0, 0.0, 0.0,  1.0, 0.0, 0.0, 0.0, // Beam 0 -> predicts -90.0
        0.0, 0.0, 0.0, 0.0,  0.0, 1.0, 0.0, 0.0, // Beam 1 -> predicts -86.0
        0.0, 0.0, 0.0, 0.0,  0.0, 0.0, 1.0, 0.0, // Beam 2 -> predicts -72.0 (strongest)
        0.0, 0.0, 0.0, 0.0,  0.0, 0.0, 0.0, 1.0, // Beam 3 -> predicts -95.0
    ];
    let biases = vec![0.0, 0.0, 0.0, 0.0];

    let layer = NeuralLayer::new(8, 4, weights, biases, ActivationFunction::Linear).unwrap();
    let network = NeuralNetwork::new(vec![layer]);
    let engine = BeamPredictionEngine::new(network, num_beams, history_slots);

    // History: RSRPs in dBm for [Slot t-1 (4 beams), Slot t (4 beams)]
    let history = vec![-92.0, -88.0, -78.0, -96.0, -90.0, -86.0, -72.0, -95.0];

    let (best_beam_id, best_rsrp_dbm, ranked) = engine
        .predict_beams(&history)
        .expect("Beam prediction must succeed");

    assert_eq!(best_beam_id, 2); // Beam 2 is top choice
    assert!((best_rsrp_dbm - (-72.0)).abs() < 1e-6);
    assert_eq!(ranked.len(), 4);
    assert_eq!(ranked[0].0, 2);
    assert_eq!(ranked[1].0, 1);
    assert_eq!(ranked[2].0, 0);
    assert_eq!(ranked[3].0, 3);
}

#[test]
fn test_positioning_cir_refinement_nlos_mitigation() {
    // 4 CIR PDP bins -> 1 output (refined ToA offset in ns)
    // Direct path peak is at bin 1, but multipath reflection peak is at bin 3
    let weights = vec![0.0, 1.0, 0.2, 0.05]; // Learns to focus on early direct peak
    let biases = vec![0.0];
    let layer = NeuralLayer::new(4, 1, weights, biases, ActivationFunction::Linear).unwrap();
    let network = NeuralNetwork::new(vec![layer]);
    let refiner = PositioningCirRefiner::new(network, 4);

    let cir_pdp = [0.1, 0.85, 0.3, 0.95]; // Bin 3 has larger amplitude (NLoS), but Bin 1 is direct path
    let refined_toa_ns = refiner
        .refine_toa(&cir_pdp)
        .expect("CIR refinement must succeed");

    // Weighted result: 0.85 * 1.0 + 0.3 * 0.2 + 0.95 * 0.05 = 0.85 + 0.06 + 0.0475 = 0.9575
    assert!((refined_toa_ns - 0.9575).abs() < 1e-4);
}

#[test]
fn test_model_lifecycle_management_and_ood_fallback() {
    let mut lcm = ModelLifecycleManager::new(
        1001,
        1,
        AIML_DEFAULT_INFERENCE_DEADLINE_US,  // 500 us
        AIML_DEFAULT_GCS_FALLBACK_THRESHOLD, // 0.70
    );

    assert_eq!(lcm.status, ModelStatus::Loaded);
    lcm.activate();
    assert_eq!(lcm.status, ModelStatus::Active);

    // 1. Test inference deadline check
    assert!(lcm.check_execution_deadline(250).is_ok());
    let err_deadline = lcm.check_execution_deadline(650).unwrap_err();
    assert!(matches!(
        err_deadline,
        AiMlError::InferenceDeadlineExceeded {
            limit_us: 500,
            actual_us: 650
        }
    ));

    // 2. Normal operational conditions (GCS = 0.92 >= 0.70)
    let ok = lcm.monitor_gcs_and_evaluate_fallback(0.92);
    assert!(ok);
    assert_eq!(lcm.status, ModelStatus::Active);
    assert!(!lcm.is_fallback_active());

    // 3. Out-Of-Distribution (OOD) channel drift: GCS drops to 0.55 < 0.70
    let ok_drift = lcm.monitor_gcs_and_evaluate_fallback(0.55);
    assert!(!ok_drift);
    assert_eq!(lcm.status, ModelStatus::FallbackSuspended);
    assert!(lcm.is_fallback_active());
    assert_eq!(lcm.fallback_trigger_count, 1);
}

#[test]
fn test_error_handling_and_boundary_cases() {
    // 1. Layer dimension mismatch on weights
    let err_w = NeuralLayer::new(
        2,
        2,
        vec![1.0, 2.0],
        vec![0.0, 0.0],
        ActivationFunction::Linear,
    )
    .unwrap_err();
    assert!(matches!(
        err_w,
        AiMlError::DimensionMismatch {
            expected: 4,
            got: 2
        }
    ));
    assert!(format!("{}", err_w).contains("expected 4, got 2"));

    // 2. Forward pass input dimension mismatch
    let layer = NeuralLayer::new(
        2,
        2,
        vec![1.0, 0.0, 0.0, 1.0],
        vec![0.0, 0.0],
        ActivationFunction::Linear,
    )
    .unwrap();
    let err_in = layer.forward(&[1.0, 2.0, 3.0]).unwrap_err();
    assert!(matches!(
        err_in,
        AiMlError::DimensionMismatch {
            expected: 2,
            got: 3
        }
    ));

    // 3. MimoChannelMatrix dimension mismatch
    let err_mat = MimoChannelMatrix::new(2, 2, vec![ComplexElement::new(0.0, 0.0)]).unwrap_err();
    assert!(matches!(
        err_mat,
        AiMlError::DimensionMismatch {
            expected: 4,
            got: 1
        }
    ));

    // 4. Uniform quantizer clamping
    let quantizer = UniformQuantizer::new(-1.0, 1.0, 8);
    assert_eq!(quantizer.quantize(5.0), 255);
    assert_eq!(quantizer.quantize(-5.0), 0);
    assert!((quantizer.dequantize(0) - (-1.0)).abs() < 1e-6);
    assert!((quantizer.dequantize(255) - 1.0).abs() < 1e-6);

    // 5. Error formatting
    let err_status = AiMlError::ModelNotActive(ModelStatus::FallbackSuspended);
    assert!(format!("{}", err_status).contains("FallbackSuspended"));

    let err_empty = AiMlError::EmptyInferenceOutput;
    assert_eq!(
        format!("{}", err_empty),
        "AI/ML model produced empty inference output"
    );

    let err_q = AiMlError::QuantizationError("overflow".to_string());
    assert!(format!("{}", err_q).contains("overflow"));

    // 6. Constants verification
    assert_eq!(AIML_DEFAULT_INFERENCE_DEADLINE_US, 500);
    assert_eq!(AIML_DEFAULT_GCS_FALLBACK_THRESHOLD, 0.70);
    assert_eq!(AIML_DEFAULT_QUANTIZATION_BITS, 8);
    assert_eq!(AIML_MAX_CANDIDATE_BEAMS, 64);
}
