//! Integration tests for 3GPP Release 18 Multi-Panel Uplink Transmit Switching
//! and Sounding Reference Signal (SRS) Engine.

use toy_tcpip::nr_ul_tx_switching::{
    ReciprocityComplex, SrsCombStructure, SrsFrequencyHopper, SrsResource, SrsResourceSet,
    SrsResourceUsage, SrsTimeDomainBehavior, SwitchingPeriodUs, UlTxSwitchingCapability,
    UlTxSwitchingEngine, UlTxSwitchingError,
};

// ---------------------------------------------------------------------------
// Test 1: SRS Transmission Comb Structures & Subcarrier Allocation
// ---------------------------------------------------------------------------
#[test]
fn test_srs_comb_structure_and_subcarrier_offsets() {
    // Comb-2: 6 subcarriers per PRB (step = 2)
    let c2_off0 = SrsCombStructure::new(2, 0).expect("Valid Comb-2 offset 0");
    let c2_off1 = SrsCombStructure::new(2, 1).expect("Valid Comb-2 offset 1");
    assert_eq!(c2_off0.subcarriers_per_prb(), 6);
    assert_eq!(c2_off0.subcarrier_indices_in_prb(), vec![0, 2, 4, 6, 8, 10]);
    assert_eq!(c2_off1.subcarrier_indices_in_prb(), vec![1, 3, 5, 7, 9, 11]);

    // Comb-4: 3 subcarriers per PRB (step = 4)
    let c4_off2 = SrsCombStructure::new(4, 2).expect("Valid Comb-4 offset 2");
    assert_eq!(c4_off2.subcarriers_per_prb(), 3);
    assert_eq!(c4_off2.subcarrier_indices_in_prb(), vec![2, 6, 10]);

    // Comb-8: step = 8
    let c8_off5 = SrsCombStructure::new(8, 5).expect("Valid Comb-8 offset 5");
    assert_eq!(c8_off5.subcarrier_indices_in_prb(), vec![5]);

    // Invalid offsets
    assert_eq!(
        SrsCombStructure::new(2, 2),
        Err(UlTxSwitchingError::InvalidCombNumber(2))
    );
    assert_eq!(
        SrsCombStructure::new(4, 4),
        Err(UlTxSwitchingError::InvalidCombNumber(4))
    );
    assert_eq!(
        SrsCombStructure::new(3, 0),
        Err(UlTxSwitchingError::InvalidCombNumber(3))
    );
}

// ---------------------------------------------------------------------------
// Test 2: SRS Cyclic Shift Validation & Multiplexing Capacity
// ---------------------------------------------------------------------------
#[test]
fn test_srs_cyclic_shifts_orthogonality() {
    let comb2 = SrsCombStructure::Comb2 { offset: 0 };
    let comb4 = SrsCombStructure::Comb4 { offset: 0 };
    let comb8 = SrsCombStructure::Comb8 { offset: 0 };

    assert_eq!(comb2.max_cyclic_shifts(), 8);
    assert_eq!(comb4.max_cyclic_shifts(), 12);
    assert_eq!(comb8.max_cyclic_shifts(), 6);

    // Valid SRS resource with cyclic shift within limits
    let res = SrsResource::new(
        1,
        1,
        comb4,
        11, // Max valid cyclic shift for Comb-4 (0..11)
        12, // Start symbol
        1,  // Num symbols
        0,
        50,
        vec![0],
    );
    assert!(res.is_ok());

    // Out-of-bounds cyclic shift
    let res_invalid = SrsResource::new(
        2,
        1,
        comb4,
        12, // Out of bounds for Comb-4
        12,
        1,
        0,
        50,
        vec![0],
    );
    assert_eq!(
        res_invalid.unwrap_err(),
        UlTxSwitchingError::InvalidCyclicShift(12)
    );
}

// ---------------------------------------------------------------------------
// Test 3: SRS Frequency Hopping Subband Cycling
// ---------------------------------------------------------------------------
#[test]
fn test_srs_frequency_hopping_subband_cycling() {
    let subband_size_prbs = 16;
    let num_subbands = 4;

    // Case 1: Frequency hopping disabled (b_hop >= B_srs)
    let prb_no_hop =
        SrsFrequencyHopper::calculate_hopping_prb(2, 1, 5, num_subbands, subband_size_prbs);
    assert_eq!(prb_no_hop, 0);

    // Case 2: Frequency hopping active (b_hop < B_srs)
    let prb_0 = SrsFrequencyHopper::calculate_hopping_prb(0, 1, 0, num_subbands, subband_size_prbs);
    let prb_1 = SrsFrequencyHopper::calculate_hopping_prb(0, 1, 1, num_subbands, subband_size_prbs);
    let prb_2 = SrsFrequencyHopper::calculate_hopping_prb(0, 1, 2, num_subbands, subband_size_prbs);
    let prb_3 = SrsFrequencyHopper::calculate_hopping_prb(0, 1, 3, num_subbands, subband_size_prbs);
    let prb_4 = SrsFrequencyHopper::calculate_hopping_prb(0, 1, 4, num_subbands, subband_size_prbs);

    assert_eq!(prb_0, 0);
    assert_eq!(prb_1, 16);
    assert_eq!(prb_2, 32);
    assert_eq!(prb_3, 48);
    assert_eq!(prb_4, 0); // Wraps back around to subband 0
}

// ---------------------------------------------------------------------------
// Test 4: 1T4R Antenna Switching Schedule
// ---------------------------------------------------------------------------
#[test]
fn test_one_tx_four_rx_antenna_switching_schedule() {
    let mut engine = UlTxSwitchingEngine::new(
        UlTxSwitchingCapability::OneTxFourRx,
        SwitchingPeriodUs::Guard14Us,
        30, // 30 kHz SCS
    );

    assert_eq!(engine.capability.total_rx_antennas(), 4);
    assert_eq!(engine.capability.simultaneous_tx_chains(), 1);
    assert_eq!(engine.capability.required_srs_resources(), 4);
    assert_eq!(engine.current_active_antennas, vec![0]);

    // Create 4 SRS resources for antennas [0], [1], [2], [3]
    let comb = SrsCombStructure::Comb4 { offset: 0 };
    let mut resources = Vec::new();
    for i in 0..4 {
        resources.push(
            SrsResource::new(i, 1, comb, 0, 13, 1, 0, 50, vec![i as usize])
                .expect("Valid resource"),
        );
    }

    let set = SrsResourceSet {
        set_id: 1,
        usage: SrsResourceUsage::AntennaSwitching,
        time_behavior: SrsTimeDomainBehavior::Aperiodic,
        resources,
    };
    engine.add_resource_set(set);

    // Schedule resource 0 (already on antenna [0]) -> no switch needed
    let ok0 = engine
        .schedule_srs_transmission(0, 0, false, false)
        .unwrap();
    assert!(ok0);
    assert_eq!(engine.current_active_antennas, vec![0]);
    assert_eq!(engine.metrics.total_switch_events, 0);

    // Schedule resource 1 (antenna [1]) -> switch occurs
    let ok1 = engine
        .schedule_srs_transmission(0, 1, false, false)
        .unwrap();
    assert!(ok1);
    assert_eq!(engine.current_active_antennas, vec![1]);
    assert_eq!(engine.metrics.total_switch_events, 1);

    // Schedule resource 2 (antenna [2]) -> switch occurs
    let ok2 = engine
        .schedule_srs_transmission(0, 2, false, false)
        .unwrap();
    assert!(ok2);
    assert_eq!(engine.current_active_antennas, vec![2]);
    assert_eq!(engine.metrics.total_switch_events, 2);

    // Schedule resource 3 (antenna [3]) -> switch occurs
    let ok3 = engine
        .schedule_srs_transmission(0, 3, false, false)
        .unwrap();
    assert!(ok3);
    assert_eq!(engine.current_active_antennas, vec![3]);
    assert_eq!(engine.metrics.total_switch_events, 3);
    assert_eq!(engine.metrics.total_srs_transmitted, 4);
}

// ---------------------------------------------------------------------------
// Test 5: 2T4R Antenna Switching Schedule
// ---------------------------------------------------------------------------
#[test]
fn test_two_tx_four_rx_antenna_switching_schedule() {
    let mut engine = UlTxSwitchingEngine::new(
        UlTxSwitchingCapability::TwoTxFourRx,
        SwitchingPeriodUs::Guard28Us,
        30,
    );

    assert_eq!(engine.capability.total_rx_antennas(), 4);
    assert_eq!(engine.capability.simultaneous_tx_chains(), 2);
    assert_eq!(engine.current_active_antennas, vec![0, 1]);

    // 2 resources with 2 ports each: Pair [0, 1] and Pair [2, 3]
    let comb = SrsCombStructure::Comb2 { offset: 0 };
    let r0 = SrsResource::new(0, 2, comb, 0, 12, 2, 0, 100, vec![0, 1]).unwrap();
    let r1 = SrsResource::new(1, 2, comb, 0, 12, 2, 0, 100, vec![2, 3]).unwrap();

    let set = SrsResourceSet {
        set_id: 1,
        usage: SrsResourceUsage::AntennaSwitching,
        time_behavior: SrsTimeDomainBehavior::Periodic {
            periodicity_slots: 20,
            offset_slots: 0,
        },
        resources: vec![r0, r1],
    };
    engine.add_resource_set(set);

    // Initial transmission on [0, 1] (no switch)
    let ok0 = engine
        .schedule_srs_transmission(0, 0, false, false)
        .unwrap();
    assert!(ok0);
    assert_eq!(engine.current_active_antennas, vec![0, 1]);
    assert_eq!(engine.metrics.total_switch_events, 0);

    // Next transmission on [2, 3] (switch occurs)
    let ok1 = engine
        .schedule_srs_transmission(0, 1, false, false)
        .unwrap();
    assert!(ok1);
    assert_eq!(engine.current_active_antennas, vec![2, 3]);
    assert_eq!(engine.metrics.total_switch_events, 1);
}

// ---------------------------------------------------------------------------
// Test 6: Switching Guard Period and PUSCH Collision Arbitration
// ---------------------------------------------------------------------------
#[test]
fn test_switching_guard_period_and_pusch_collision_arbitration() {
    let mut engine = UlTxSwitchingEngine::new(
        UlTxSwitchingCapability::OneTxTwoRx,
        SwitchingPeriodUs::Guard28Us,
        30, // 30 kHz SCS -> symbol duration ~35.7 us, 28 us requires 1 guard symbol
    );

    let comb = SrsCombStructure::Comb2 { offset: 0 };
    let r0 = SrsResource::new(0, 1, comb, 0, 13, 1, 0, 50, vec![0]).unwrap();
    let r1 = SrsResource::new(1, 1, comb, 0, 13, 1, 0, 50, vec![1]).unwrap();

    let set = SrsResourceSet {
        set_id: 1,
        usage: SrsResourceUsage::AntennaSwitching,
        time_behavior: SrsTimeDomainBehavior::Aperiodic,
        resources: vec![r0, r1],
    };
    engine.add_resource_set(set);

    // Initial state is [0]. Schedule resource 1 (switch to [1]) with preceding normal PUSCH
    // Normal PUSCH can be punctured for guard symbol
    let ok1 = engine
        .schedule_srs_transmission(
            10, 1, true,  // has PUSCH prior symbol
            false, // not critical URLLC
        )
        .unwrap();
    assert!(ok1);
    assert_eq!(engine.metrics.guard_symbols_punctured, 1);
    assert_eq!(engine.metrics.pusch_sounding_conflicts_resolved, 1);

    // Now state is [1]. Schedule resource 0 (switch back to [0]) with critical URLLC PUSCH
    // Critical URLLC PUSCH cannot be punctured -> SRS is dropped
    let ok2 = engine
        .schedule_srs_transmission(
            11, 0, true, // has PUSCH prior symbol
            true, // is critical URLLC!
        )
        .unwrap();
    assert!(!ok2); // Sounding cancelled
    assert_eq!(engine.current_active_antennas, vec![1]); // Antenna remains unchanged
    assert_eq!(engine.metrics.pusch_sounding_conflicts_resolved, 2);
}

// ---------------------------------------------------------------------------
// Test 7: Reciprocity Channel Reconstruction & SVD Beamformer
// ---------------------------------------------------------------------------
#[test]
fn test_reciprocity_channel_reconstruction_and_svd_beamforming() {
    let mut engine = UlTxSwitchingEngine::new(
        UlTxSwitchingCapability::OneTxFourRx,
        SwitchingPeriodUs::Guard14Us,
        30,
    );

    // Sounded 4-antenna channel vector: h = [0.5+0.5j, 0.8-0.2j, -0.4+0.6j, 0.3-0.7j]
    let sounded = vec![
        (0, ReciprocityComplex::new(0.5, 0.5)),
        (1, ReciprocityComplex::new(0.8, -0.2)),
        (2, ReciprocityComplex::new(-0.4, 0.6)),
        (3, ReciprocityComplex::new(0.3, -0.7)),
    ];

    let profile = engine
        .reconstruct_reciprocal_channel(&sounded)
        .expect("Reconstruction success");

    assert_eq!(profile.num_antennas, 4);
    assert_eq!(profile.beamforming_weights.len(), 4);

    // Verify beamforming weights are normalized: sum(|w|^2) == 1.0
    let mut weight_norm_sq = 0.0;
    for w in &profile.beamforming_weights {
        weight_norm_sq += w.norm_sq();
    }
    assert!((weight_norm_sq - 1.0).abs() < 1e-9);

    // 4-antenna theoretical array gain: 10 * log10(4) = ~6.02 dB
    let expected_gain = 10.0 * (4.0_f64).log10();
    assert!((profile.array_gain_db - expected_gain).abs() < 1e-4);
    assert!((engine.metrics.average_reciprocity_gain_db - expected_gain).abs() < 1e-4);

    // Incomplete channel sounding error check
    let incomplete = vec![(0, ReciprocityComplex::new(0.5, 0.5))];
    let err = engine.reconstruct_reciprocal_channel(&incomplete);
    assert!(matches!(
        err,
        Err(UlTxSwitchingError::CalibrationFailure(_))
    ));
}
