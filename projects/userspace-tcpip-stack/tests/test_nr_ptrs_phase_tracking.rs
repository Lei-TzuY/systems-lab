//! Integration tests for 3GPP Release 18 Multi-Port Phase Tracking Reference Signal (PT-RS)
//! and mmWave / FR2-2 Phase Noise Compensation Engine.

use std::f64::consts::PI;
use toy_tcpip::nr_ptrs_phase_tracking::{
    CommonPhaseErrorEstimator, Complex64, DftSOfdmPtrsConfig, GoldSequenceGenerator,
    PhaseDerotator, PtrsEngine, PtrsError, PtrsFrequencyBand, PtrsFrequencyDensity,
    PtrsResourceMapper, PtrsThresholdConfig, PtrsTimeDensity, PtrsWaveformType,
};

// ---------------------------------------------------------------------------
// Test 1: PT-RS Time Density (L_PTRS) from MCS Thresholds (TS 38.214 Table 5.1.6.3-1)
// ---------------------------------------------------------------------------
#[test]
fn test_ptrs_time_density_mcs_thresholds() {
    let cfg = PtrsThresholdConfig::default(); // [10, 16, 20, 29]

    // Below ptrsh-MCS1 (10): PT-RS disabled
    assert_eq!(cfg.determine_time_density(0), PtrsTimeDensity::Disabled);
    assert_eq!(cfg.determine_time_density(9), PtrsTimeDensity::Disabled);

    // [10, 16): Every 4 symbols (L = 4)
    assert_eq!(
        cfg.determine_time_density(10),
        PtrsTimeDensity::Every4Symbols
    );
    assert_eq!(
        cfg.determine_time_density(12),
        PtrsTimeDensity::Every4Symbols
    );
    assert_eq!(
        cfg.determine_time_density(15),
        PtrsTimeDensity::Every4Symbols
    );

    // [16, 20): Every 2 symbols (L = 2)
    assert_eq!(
        cfg.determine_time_density(16),
        PtrsTimeDensity::Every2Symbols
    );
    assert_eq!(
        cfg.determine_time_density(18),
        PtrsTimeDensity::Every2Symbols
    );
    assert_eq!(
        cfg.determine_time_density(19),
        PtrsTimeDensity::Every2Symbols
    );

    // >= 20: Every 1 symbol (L = 1)
    assert_eq!(
        cfg.determine_time_density(20),
        PtrsTimeDensity::Every1Symbol
    );
    assert_eq!(
        cfg.determine_time_density(28),
        PtrsTimeDensity::Every1Symbol
    );
    assert_eq!(
        cfg.determine_time_density(31),
        PtrsTimeDensity::Every1Symbol
    );

    // Custom threshold constructor validation
    let custom = PtrsThresholdConfig::new([8, 14, 22, 28], [16, 32]).expect("Valid thresholds");
    assert_eq!(custom.determine_time_density(7), PtrsTimeDensity::Disabled);
    assert_eq!(
        custom.determine_time_density(8),
        PtrsTimeDensity::Every4Symbols
    );
    assert_eq!(
        custom.determine_time_density(14),
        PtrsTimeDensity::Every2Symbols
    );
    assert_eq!(
        custom.determine_time_density(22),
        PtrsTimeDensity::Every1Symbol
    );

    // Invalid monotonic ordering check
    let invalid = PtrsThresholdConfig::new([16, 10, 20, 29], [24, 48]);
    assert!(matches!(
        invalid,
        Err(PtrsError::InvalidThresholdConfiguration(_))
    ));
}

// ---------------------------------------------------------------------------
// Test 2: PT-RS Frequency Density (K_PTRS) from PRB Thresholds (TS 38.214 Table 5.1.6.3-2)
// ---------------------------------------------------------------------------
#[test]
fn test_ptrs_frequency_density_prb_thresholds() {
    let cfg = PtrsThresholdConfig::default(); // N_RB thresholds [24, 48]

    // Below N_RB0 (24): PT-RS disabled
    assert_eq!(
        cfg.determine_frequency_density(1),
        PtrsFrequencyDensity::Disabled
    );
    assert_eq!(
        cfg.determine_frequency_density(12),
        PtrsFrequencyDensity::Disabled
    );
    assert_eq!(
        cfg.determine_frequency_density(23),
        PtrsFrequencyDensity::Disabled
    );

    // [24, 48): Every 4 PRBs (K = 4)
    assert_eq!(
        cfg.determine_frequency_density(24),
        PtrsFrequencyDensity::Every4PRBs
    );
    assert_eq!(
        cfg.determine_frequency_density(36),
        PtrsFrequencyDensity::Every4PRBs
    );
    assert_eq!(
        cfg.determine_frequency_density(47),
        PtrsFrequencyDensity::Every4PRBs
    );

    // >= 48: Every 2 PRBs (K = 2)
    assert_eq!(
        cfg.determine_frequency_density(48),
        PtrsFrequencyDensity::Every2PRBs
    );
    assert_eq!(
        cfg.determine_frequency_density(100),
        PtrsFrequencyDensity::Every2PRBs
    );
    assert_eq!(
        cfg.determine_frequency_density(273),
        PtrsFrequencyDensity::Every2PRBs
    );

    assert_eq!(PtrsFrequencyDensity::Every2PRBs.step_prb(), 2);
    assert_eq!(PtrsFrequencyDensity::Every4PRBs.step_prb(), 4);
    assert!(!PtrsFrequencyDensity::Disabled.is_enabled());
}

// ---------------------------------------------------------------------------
// Test 3: PT-RS Resource Mapping and Gold Sequence QPSK Generation
// ---------------------------------------------------------------------------
#[test]
fn test_ptrs_resource_mapping_and_gold_sequence() {
    // DMRS port offsets
    assert_eq!(PtrsResourceMapper::get_subcarrier_offset(1000), 0);
    assert_eq!(PtrsResourceMapper::get_subcarrier_offset(1001), 2);
    assert_eq!(PtrsResourceMapper::get_subcarrier_offset(1002), 6);
    assert_eq!(PtrsResourceMapper::get_subcarrier_offset(1003), 8);

    // Map PT-RS subcarriers for 50 PRBs, K=2 (every 2 PRBs), DMRS port 1000, cell_id=0
    let scs =
        PtrsResourceMapper::map_ptrs_subcarriers(0, 50, PtrsFrequencyDensity::Every2PRBs, 1000, 0);
    // 50 PRBs with step 2 -> 25 subcarriers
    assert_eq!(scs.len(), 25);
    assert_eq!(scs[0], 0); // PRB 0, sc 0
    assert_eq!(scs[1], 24); // PRB 2, sc 0 -> 2 * 12 + 0 = 24
    assert_eq!(scs[2], 48); // PRB 4, sc 0 -> 4 * 12 + 0 = 48
    assert_eq!(scs[24], 48 * 12); // PRB 48 -> 576

    // Gold sequence generation: verify unit power QPSK symbols
    let c_init = PtrsResourceMapper::calculate_c_init(0, 0, 42);
    let mut gold1 = GoldSequenceGenerator::new(c_init);
    let mut gold2 = GoldSequenceGenerator::new(c_init);

    for _ in 0..100 {
        let sym1 = PtrsResourceMapper::generate_qpsk_symbol(&mut gold1);
        let sym2 = PtrsResourceMapper::generate_qpsk_symbol(&mut gold2);

        // Deterministic reproducibility
        assert_eq!(sym1, sym2);

        // Unit magnitude: |s|^2 = (1/sqrt(2))^2 + (1/sqrt(2))^2 = 0.5 + 0.5 = 1.0
        let mag_sq = sym1.norm_sq();
        assert!((mag_sq - 1.0).abs() < 1e-9);
    }
}

// ---------------------------------------------------------------------------
// Test 4: Uplink DFT-s-OFDM PT-RS Low-PAPR Chunks
// ---------------------------------------------------------------------------
#[test]
fn test_dft_s_ofdm_uplink_ptrs_chunks() {
    let dft_cfg = DftSOfdmPtrsConfig::new(4, 2);
    assert_eq!(dft_cfg.num_ptrs_groups, 4);
    assert_eq!(dft_cfg.samples_per_group, 2);

    let chunk0 = dft_cfg.generate_chunk_sequence(0, 4);
    let chunk1 = dft_cfg.generate_chunk_sequence(1, 4);

    assert_eq!(chunk0.len(), 2);
    assert_eq!(chunk1.len(), 2);

    // Each sample should have unit magnitude
    for s in chunk0.iter().chain(chunk1.iter()) {
        assert!((s.norm_sq() - 1.0).abs() < 1e-9);
    }

    // Different groups produce distinct phase rotations
    assert_ne!(chunk0[1], chunk1[1]);
}

// ---------------------------------------------------------------------------
// Test 5: Symbol-by-Symbol CPE Estimation and Phase Unwrapping Servo
// ---------------------------------------------------------------------------
#[test]
fn test_cpe_estimation_and_phase_unwrapping_servo() {
    let mut estimator = CommonPhaseErrorEstimator::new();

    // Transmit 16 PT-RS reference symbols
    let ref_symbols = vec![
        Complex64::new(1.0, 0.0),
        Complex64::new(0.0, 1.0),
        Complex64::new(-1.0, 0.0),
        Complex64::new(0.0, -1.0),
    ];

    // Simulate steady phase drift of 0.35 rad per symbol
    // Over 12 symbols, total phase will exceed pi (10 * 0.35 = 3.5 rad > 3.14159 rad)
    let phase_step = 0.35;
    let mut ground_truth_phase = 0.0;

    for sym_idx in 0..15 {
        ground_truth_phase += phase_step;

        // Apply ground truth phase to reference symbols
        let rx_symbols: Vec<Complex64> = ref_symbols
            .iter()
            .map(|x| x.rotate(ground_truth_phase))
            .collect();

        let est_phase = estimator
            .estimate_cpe(&rx_symbols, &ref_symbols)
            .expect("Estimation success");

        // The unwrapped phase must track the continuous phase drift without modulo 2*pi wrapping
        assert!(
            (est_phase - ground_truth_phase).abs() < 1e-6,
            "Symbol {}: est {}, true {}",
            sym_idx,
            est_phase,
            ground_truth_phase
        );
    }
}

// ---------------------------------------------------------------------------
// Test 6: Symbol Phase Derotation and EVM Improvement
// ---------------------------------------------------------------------------
#[test]
fn test_symbol_phase_derotation_and_evm_improvement() {
    // Generate a 16-element QPSK / QAM constellation
    let num_subcarriers = 120;
    let mut tx_symbols = Vec::with_capacity(num_subcarriers);
    for i in 0..num_subcarriers {
        let phase = ((i % 4) as f64) * PI / 2.0 + PI / 4.0;
        tx_symbols.push(Complex64::from_polar(1.0, phase));
    }

    // Apply severe Common Phase Error: theta = 0.82 rad (~47 degrees)
    let cpe_error = 0.82;
    let mut rx_grid: Vec<Complex64> = tx_symbols.iter().map(|s| s.rotate(cpe_error)).collect();

    // Raw EVM before derotation should be severe (> 75%)
    let raw_evm = PhaseDerotator::calculate_evm_percent(&rx_grid, &tx_symbols);
    assert!(raw_evm > 70.0, "Raw EVM was only {}%", raw_evm);

    // PT-RS subcarriers at indices 0, 24, 48, 72, 96
    let ptrs_indices = vec![0, 24, 48, 72, 96];
    let rx_ptrs: Vec<Complex64> = ptrs_indices.iter().map(|&idx| rx_grid[idx]).collect();
    let tx_ptrs: Vec<Complex64> = ptrs_indices.iter().map(|&idx| tx_symbols[idx]).collect();

    // Estimate CPE
    let mut estimator = CommonPhaseErrorEstimator::new();
    let est_cpe = estimator
        .estimate_cpe(&rx_ptrs, &tx_ptrs)
        .expect("Estimate CPE");
    assert!((est_cpe - cpe_error).abs() < 1e-6);

    // Derotate full grid
    PhaseDerotator::derotate_symbol(&mut rx_grid, est_cpe);

    // Derotated EVM should drop to virtually 0% (< 1e-5%)
    let derotated_evm = PhaseDerotator::calculate_evm_percent(&rx_grid, &tx_symbols);
    assert!(derotated_evm < 0.01, "Derotated EVM was {}%", derotated_evm);

    // Residual ICI calculation
    let derotated_ptrs: Vec<Complex64> = ptrs_indices.iter().map(|&idx| rx_grid[idx]).collect();
    let residual_ici = PhaseDerotator::calculate_residual_ici(&derotated_ptrs, &tx_ptrs);
    assert!(residual_ici < 1e-12);
}

// ---------------------------------------------------------------------------
// Test 7: Multi-Symbol PT-RS Engine Pipeline and Telemetry Metrics
// ---------------------------------------------------------------------------
#[test]
fn test_ptrs_engine_multi_symbol_pipeline_and_metrics() {
    let mut engine = PtrsEngine::new(
        PtrsWaveformType::CpOfdm,
        PtrsFrequencyBand::Fr2MmWave,
        101,  // Cell ID
        1000, // DMRS port 1000
        None, // Default thresholds
    )
    .expect("Engine init");

    let num_prbs = 48; // Will select Every2PRBs (K=2)
    let mcs = 25; // Will select Every1Symbol (L=1)
    let grid_size = (num_prbs * 12) as usize;

    let mut dmrs_symbols = [false; 14];
    dmrs_symbols[2] = true; // DMRS on symbol 2

    // Process a full 14-symbol slot with phase noise
    let mut current_drift = 0.0;
    for sym_idx in 0..14 {
        current_drift += 0.05; // 0.05 rad phase drift per symbol

        // Generate clean reference
        let mut tx_ref = vec![Complex64::new(1.0, 0.0); grid_size];

        // If symbol contains PT-RS, embed the actual PT-RS reference symbols into tx_ref
        let time_density = engine.threshold_config.determine_time_density(mcs);
        let freq_density = engine
            .threshold_config
            .determine_frequency_density(num_prbs);
        if PtrsResourceMapper::is_ptrs_symbol(sym_idx, &dmrs_symbols, time_density) {
            let ptrs_scs = PtrsResourceMapper::map_ptrs_subcarriers(
                0,
                num_prbs,
                freq_density,
                engine.dmrs_port,
                engine.cell_id,
            );
            let c_init = PtrsResourceMapper::calculate_c_init(0, sym_idx as u32, engine.cell_id);
            let mut gold = GoldSequenceGenerator::new(c_init);
            for &sc in ptrs_scs.iter() {
                if sc < grid_size {
                    tx_ref[sc] = PtrsResourceMapper::generate_qpsk_symbol(&mut gold);
                }
            }
        }

        // Apply phase noise
        let mut rx_grid: Vec<Complex64> = tx_ref.iter().map(|s| s.rotate(current_drift)).collect();

        let result = engine
            .process_symbol(
                0, // slot 0
                sym_idx,
                mcs,
                num_prbs,
                &dmrs_symbols,
                &mut rx_grid,
                &tx_ref,
            )
            .expect("Process symbol success");

        if sym_idx == 2 {
            // Symbol 2 is DMRS, so PT-RS is not present
            assert_eq!(result, None);
        } else {
            // Other symbols should have PT-RS present and successfully estimated
            assert!(result.is_some());
            let est = result.unwrap();
            assert!((est - current_drift).abs() < 0.01);
        }
    }

    // Verify Engine Telemetry Metrics
    assert_eq!(engine.metrics.total_symbols_processed, 14);
    assert_eq!(engine.metrics.ptrs_symbols_tracked, 13); // 14 minus 1 DMRS symbol
    assert!(engine.metrics.max_absolute_phase_drift_rad > 0.6);
    assert!(engine.metrics.average_raw_evm_percent > 20.0);
    assert!(engine.metrics.average_derotated_evm_percent < 0.1);
    assert!(engine.metrics.average_residual_ici_variance < 1e-6);
}
