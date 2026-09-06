//! Integration Tests for 3GPP Rel-17/18 Conditional Handover (CHO) & Dual Connectivity CPC Engine.

use toy_tcpip::nr_conditional_handover::*;

#[test]
fn test_l3_filter_convergence_and_coefficients() {
    let mut filter = L3Filter::new(4); // k = 4, a = 0.5
    assert_eq!(filter.coeff_k(), 4);
    assert_eq!(filter.value(), None);

    // First sample initializes filter directly
    let f1 = filter.filter(-80.0);
    assert!((f1 - -80.0).abs() < 1e-6);

    // Second sample: 0.5 * (-80.0) + 0.5 * (-90.0) = -85.0
    let f2 = filter.filter(-90.0);
    assert!((f2 - -85.0).abs() < 1e-6);

    // Third sample: 0.5 * (-85.0) + 0.5 * (-90.0) = -87.5
    let f3 = filter.filter(-90.0);
    assert!((f3 - -87.5).abs() < 1e-6);

    // Reset clears filter
    filter.reset();
    assert_eq!(filter.value(), None);
}

#[test]
fn test_event_a3_condition_evaluation_with_hysteresis() {
    let cond = CondExecutionCondition::EventA3 {
        offset_db: 3.0,
        hysteresis_db: 1.0,
    };

    let spcell_rsrp = -80.0;

    // Neighbor at -83 dBm:
    // Mn - Hys = -84.0, Ms + Offset = -77.0 -> False
    assert!(!cond.evaluate_entering(spcell_rsrp, -83.0));

    // Neighbor at -75 dBm:
    // Mn - Hys = -76.0, Ms + Offset = -77.0 -> True (Entering)
    assert!(cond.evaluate_entering(spcell_rsrp, -75.0));

    // Test Leaving condition:
    // Mn + Hys < Ms + Offset => Mn + 1.0 < -77.0 => Mn < -78.0
    // At -76 dBm, it should NOT leave yet (hysteresis protection)
    assert!(!cond.evaluate_leaving(spcell_rsrp, -76.0));
    // At -79 dBm, it leaves
    assert!(cond.evaluate_leaving(spcell_rsrp, -79.0));
}

#[test]
fn test_event_a5_condition_evaluation_with_dual_thresholds() {
    let cond = CondExecutionCondition::EventA5 {
        threshold1_dbm: -95.0,
        threshold2_dbm: -85.0,
        hysteresis_db: 2.0,
    };

    // Case 1: Serving cell too good (-90 dBm > -95 dBm)
    assert!(!cond.evaluate_entering(-90.0, -80.0));

    // Case 2: Serving cell degraded (-100 dBm), but neighbor is also poor (-90 dBm < -85 dBm)
    assert!(!cond.evaluate_entering(-100.0, -90.0));

    // Case 3: Serving cell degraded (-100 dBm) AND neighbor is strong (-80 dBm)
    // Ms + Hys = -98 < -95 AND Mn - Hys = -82 > -85 -> True
    assert!(cond.evaluate_entering(-100.0, -80.0));

    // Test Leaving condition
    // Serving recovers to -92 dBm (Ms - Hys = -94 > -95) -> Leaves
    assert!(cond.evaluate_leaving(-92.0, -80.0));
}

#[test]
fn test_time_to_trigger_progression_and_transient_reset() {
    let mut engine = ChoEngine::new("UE_CHO_01", 100);
    engine.update_spcell_measurement(-90.0);

    let candidate = CondReconfigCandidate::new(
        1,
        200,
        3500.0,
        ChoType::MasterCellGroupHandover,
        CondExecutionCondition::EventA3 {
            offset_db: 2.0,
            hysteresis_db: 1.0,
        },
        80, // TTT = 80 ms
        Some(12),
        0xC001,
        10_000,
    );
    engine.add_candidate(candidate).unwrap();

    // Neighbor measurement qualifies: -85 dBm (-85 - 1 = -86 > -90 + 2 = -88)
    engine.update_candidate_measurement(1, -85.0).unwrap();

    // Step 40 ms: TTT running, not yet triggered
    let trig_1 = engine.step_time(40);
    assert_eq!(trig_1, None);
    assert_eq!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::TttActive { elapsed_ms: 40 }
    );

    // Transient drop in neighbor signal (leaves condition): -95 dBm
    engine.update_candidate_measurement(1, -95.0).unwrap();
    let trig_bounce = engine.step_time(10);
    assert_eq!(trig_bounce, None);
    // TTT should have been reset back to Configured
    assert_eq!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::Configured
    );
    assert_eq!(engine.metrics.ttt_resets, 1);

    // Neighbor recovers to strong signal: -80 dBm
    engine.update_candidate_measurement(1, -80.0).unwrap();
    engine.step_time(40);
    assert_eq!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::TttActive { elapsed_ms: 40 }
    );

    // Advance remaining 40 ms: total TTT = 80 ms reaches threshold!
    let trig_final = engine.step_time(40);
    assert_eq!(trig_final, Some(1));
    assert_eq!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::ConditionMet
    );
}

#[test]
fn test_autonomous_cho_execution_and_multi_candidate_cancellation() {
    let mut engine = ChoEngine::new("UE_CHO_02", 50);
    // Severely degraded source SpCell (-115 dBm)
    engine.update_spcell_measurement(-115.0);

    // Prepare 3 candidate target cells
    for id in 1..=3 {
        let cand = CondReconfigCandidate::new(
            id,
            100 + id as u32,
            3500.0,
            ChoType::MasterCellGroupHandover,
            CondExecutionCondition::EventA3 {
                offset_db: 1.0,
                hysteresis_db: 0.5,
            },
            40,
            Some(10 + id),
            0xC000 + id as u16,
            5000,
        );
        engine.add_candidate(cand).unwrap();
    }

    // Candidate 2 has strongest neighbor signal (-85 dBm)
    engine.update_candidate_measurement(1, -110.0).unwrap();
    engine.update_candidate_measurement(2, -85.0).unwrap();
    engine.update_candidate_measurement(3, -105.0).unwrap();

    // Advance 40 ms: Candidate 2 should trigger
    let trig = engine.step_time(40);
    assert_eq!(trig, Some(2));

    // Execute CHO to Candidate 2
    let report = engine.execute_cho(2).expect("Execution failed");
    assert_eq!(report.executed_candidate_id, 2);
    assert_eq!(report.target_pci, 102);
    assert_eq!(report.dedicated_preamble_index, Some(12));
    assert_eq!(report.target_c_rnti, 0xC002);

    // Serving cell PCI should have transitioned to target 102
    assert_eq!(engine.current_spcell_pci, 102);

    // Verify automatic Xn-AP cancellation of non-selected candidates (1 and 3)
    assert_eq!(report.cancelled_candidate_ids, vec![1, 3]);
    assert!(matches!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::Cancelled { .. }
    ));
    assert!(matches!(
        engine.candidates.get(&3).unwrap().state,
        CandidateState::Cancelled { .. }
    ));

    // Metrics should record avoided RLF because source cell was -115 dBm
    assert_eq!(engine.metrics.executions_succeeded, 1);
    assert_eq!(engine.metrics.avoided_rlf_count, 1);
    assert_eq!(engine.metrics.total_cancellations, 2);
}

#[test]
fn test_conditional_pscell_change_cpc_dual_connectivity() {
    let mut engine = ChoEngine::new("UE_CPC_01", 10);
    engine.update_spcell_measurement(-85.0);

    // Candidate configured as ConditionalPscellChange (SCG CPC)
    let cpc_cand = CondReconfigCandidate::new(
        5,
        500,
        28_000.0, // FR2 mmWave PSCell
        ChoType::ConditionalPscellChange,
        CondExecutionCondition::EventA3 {
            offset_db: 3.0,
            hysteresis_db: 1.0,
        },
        40,
        Some(33),
        0xD005,
        10_000,
    );
    engine.add_candidate(cpc_cand).unwrap();

    engine.update_candidate_measurement(5, -75.0).unwrap();
    let trig = engine.step_time(40);
    assert_eq!(trig, Some(5));

    let report = engine.execute_cho(5).expect("CPC execution failed");
    assert_eq!(report.cho_type, ChoType::ConditionalPscellChange);
    assert_eq!(report.target_pci, 500);

    // In CPC, the PCell PCI (10) remains unchanged!
    assert_eq!(engine.current_spcell_pci, 10);
}

#[test]
fn test_rach_failure_and_candidate_fallback() {
    let mut engine = ChoEngine::new("UE_FALLBACK", 1);
    engine.update_spcell_measurement(-100.0);

    // Candidate 1 and Candidate 2 both meet condition
    for id in 1..=2 {
        let cand = CondReconfigCandidate::new(
            id,
            id as u32 * 10,
            3500.0,
            ChoType::MasterCellGroupHandover,
            CondExecutionCondition::EventA3 {
                offset_db: 0.0,
                hysteresis_db: 0.0,
            },
            20,
            Some(id),
            0xF000 + id as u16,
            1000,
        );
        engine.add_candidate(cand).unwrap();
        engine.update_candidate_measurement(id, -80.0).unwrap();
    }

    // Step time: Candidate 1 and 2 both satisfy condition
    let _ = engine.step_time(20);
    assert_eq!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::ConditionMet
    );
    assert_eq!(
        engine.candidates.get(&2).unwrap().state,
        CandidateState::ConditionMet
    );

    // Simulate RACH failure on candidate 1
    let fallback_res = engine.handle_rach_failure(1).unwrap();
    // Candidate 2 is ready as fallback!
    assert_eq!(fallback_res, Some(2));
    assert!(matches!(
        engine.candidates.get(&1).unwrap().state,
        CandidateState::RachFailed { attempts: 1 }
    ));

    // Test validity timer expiration
    let mut cand3 = CondReconfigCandidate::new(
        3,
        30,
        3500.0,
        ChoType::MasterCellGroupHandover,
        CondExecutionCondition::EventA3 {
            offset_db: 0.0,
            hysteresis_db: 0.0,
        },
        500,
        None,
        0xF003,
        100, // short validity timer (100 ms)
    );
    cand3.elapsed_validity_ms = 90;
    engine.add_candidate(cand3).unwrap();

    // Advance 20 ms -> elapsed 110 ms >= 100 ms validity timer
    engine.step_time(20);
    assert_eq!(
        engine.candidates.get(&3).unwrap().state,
        CandidateState::Expired
    );
    assert_eq!(engine.metrics.expired_candidates, 1);
}
