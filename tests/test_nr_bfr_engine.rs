//! Integration tests for 3GPP Rel-17 5G NR Beam Failure Detection (BFD) & Recovery (BFR) Engine.

use toy_tcpip::nr_bfr_engine::{
    BeamFailureRecoveryConfig, BeamIdentifier, BeamMeasurement, BfrEvent, BfrState,
    BfrTransmissionType, CandidateBeamConfig, NrBfrEngine, ReferenceSignalType,
};

#[test]
fn test_nr_bfr_healthy_tracking_and_timer_decay() {
    let config = BeamFailureRecoveryConfig {
        bfi_max_count: 3,
        bfd_timer_slots: 10,
        bfr_timer_slots: 20,
        q_out_threshold_dbm: -110,
        q_in_threshold_dbm: -90,
        q0_serving_beams: vec![BeamIdentifier::ssb(10)],
        q1_candidate_beams: vec![CandidateBeamConfig {
            beam: BeamIdentifier::ssb(11),
            dedicated_preamble_index: Some(4),
            prach_occasion_slot: Some(25),
        }],
        recovery_search_space_id: 2,
    };

    let mut engine = NrBfrEngine::new(0x4242, BeamIdentifier::ssb(10), config);
    assert_eq!(engine.state, BfrState::Normal);
    assert_eq!(engine.bfi_counter, 0);

    // 1. Healthy measurement (-85 dBm >= -110 dBm): No BFI event
    let healthy_meas = [BeamMeasurement {
        beam: BeamIdentifier::ssb(10),
        rsrp_dbm: -85,
    }];
    let res = engine.process_l1_measurements(&healthy_meas, &[]);
    assert_eq!(res, None);
    assert_eq!(engine.bfi_counter, 0);
    assert_eq!(engine.state, BfrState::Normal);

    // 2. Degraded measurement (-115 dBm < -110 dBm): First BFI
    let degraded_meas = [BeamMeasurement {
        beam: BeamIdentifier::ssb(10),
        rsrp_dbm: -115,
    }];
    let res1 = engine.process_l1_measurements(&degraded_meas, &[]);
    assert_eq!(res1, Some(BfrEvent::BfiDetected { counter: 1 }));
    assert_eq!(engine.bfi_counter, 1);
    assert_eq!(engine.bfd_timer_remaining, 10);
    assert_eq!(engine.state, BfrState::EvaluatingFailure);

    // 3. Second BFI
    let res2 = engine.process_l1_measurements(&degraded_meas, &[]);
    assert_eq!(res2, Some(BfrEvent::BfiDetected { counter: 2 }));
    assert_eq!(engine.bfi_counter, 2);
    assert_eq!(engine.bfd_timer_remaining, 10);

    // 4. Advance time by 9 slots: soak timer decrements but does not expire yet
    for _ in 0..9 {
        assert_eq!(engine.step_slot(), None);
    }
    assert_eq!(engine.bfd_timer_remaining, 1);
    assert_eq!(engine.bfi_counter, 2);

    // 5. 10th slot: soak timer expires, resetting BFI counter and returning to Normal
    assert_eq!(engine.step_slot(), None);
    assert_eq!(engine.bfd_timer_remaining, 0);
    assert_eq!(engine.bfi_counter, 0);
    assert_eq!(engine.state, BfrState::Normal);
}

#[test]
fn test_nr_bfr_consecutive_bfi_and_candidate_selection_cfra() {
    let config = BeamFailureRecoveryConfig {
        bfi_max_count: 3,
        bfd_timer_slots: 15,
        bfr_timer_slots: 30,
        q_out_threshold_dbm: -105,
        q_in_threshold_dbm: -88,
        q0_serving_beams: vec![BeamIdentifier::ssb(1)],
        q1_candidate_beams: vec![
            CandidateBeamConfig {
                beam: BeamIdentifier::csi_rs(20),
                dedicated_preamble_index: Some(15),
                prach_occasion_slot: Some(40),
            },
            CandidateBeamConfig {
                beam: BeamIdentifier::ssb(5),
                dedicated_preamble_index: Some(12),
                prach_occasion_slot: Some(42),
            },
        ],
        recovery_search_space_id: 3,
    };

    let mut engine = NrBfrEngine::new(0x1234, BeamIdentifier::ssb(1), config);

    let degraded_serving = [BeamMeasurement {
        beam: BeamIdentifier::ssb(1),
        rsrp_dbm: -118,
    }];

    // Candidate measurements: CSI-RS 20 is best (-80 dBm), SSB 5 is good (-85 dBm), SSB 9 fails Q_in (-95 dBm)
    let candidates = [
        BeamMeasurement {
            beam: BeamIdentifier::ssb(9),
            rsrp_dbm: -95,
        },
        BeamMeasurement {
            beam: BeamIdentifier::csi_rs(20),
            rsrp_dbm: -80,
        },
        BeamMeasurement {
            beam: BeamIdentifier::ssb(5),
            rsrp_dbm: -85,
        },
    ];

    // Event 1 & 2: BFI instances
    assert_eq!(
        engine.process_l1_measurements(&degraded_serving, &candidates),
        Some(BfrEvent::BfiDetected { counter: 1 })
    );
    assert_eq!(
        engine.process_l1_measurements(&degraded_serving, &candidates),
        Some(BfrEvent::BfiDetected { counter: 2 })
    );

    // Event 3: 3rd consecutive BFI triggers failure & dispatches CFRA recovery request
    let res3 = engine.process_l1_measurements(&degraded_serving, &candidates);
    match res3 {
        Some(BfrEvent::RecoveryRequestDispatched {
            candidate_beam,
            transmission,
        }) => {
            assert_eq!(candidate_beam, BeamIdentifier::csi_rs(20));
            assert_eq!(
                transmission,
                BfrTransmissionType::CfraPreamble {
                    preamble_index: 15,
                    prach_slot: 40,
                }
            );
        }
        other => panic!("Expected RecoveryRequestDispatched, got: {:?}", other),
    }

    assert_eq!(engine.bfr_timer_remaining, 30);
    assert_eq!(engine.total_recovery_attempts, 1);
    match engine.state {
        BfrState::AwaitingResponse {
            candidate_beam,
            transmission_type,
        } => {
            assert_eq!(candidate_beam, BeamIdentifier::csi_rs(20));
            assert_eq!(
                transmission_type,
                BfrTransmissionType::CfraPreamble {
                    preamble_index: 15,
                    prach_slot: 40,
                }
            );
        }
        ref st => panic!("Unexpected engine state: {:?}", st),
    }
}

#[test]
fn test_nr_bfr_pdcch_recovery_response_and_beam_switchover() {
    let config = BeamFailureRecoveryConfig {
        bfi_max_count: 2,
        bfd_timer_slots: 10,
        bfr_timer_slots: 20,
        q_out_threshold_dbm: -100,
        q_in_threshold_dbm: -85,
        q0_serving_beams: vec![BeamIdentifier::ssb(0)],
        q1_candidate_beams: vec![CandidateBeamConfig {
            beam: BeamIdentifier::csi_rs(8),
            dedicated_preamble_index: Some(21),
            prach_occasion_slot: Some(50),
        }],
        recovery_search_space_id: 1,
    };

    let ue_crnti = 0x8899;
    let mut engine = NrBfrEngine::new(ue_crnti, BeamIdentifier::ssb(0), config);

    let degraded = [BeamMeasurement {
        beam: BeamIdentifier::ssb(0),
        rsrp_dbm: -112,
    }];
    let candidate = [BeamMeasurement {
        beam: BeamIdentifier::csi_rs(8),
        rsrp_dbm: -80,
    }];

    // Reach failure
    engine.process_l1_measurements(&degraded, &candidate);
    engine.process_l1_measurements(&degraded, &candidate);

    assert!(matches!(engine.state, BfrState::AwaitingResponse { .. }));

    // Wrong C-RNTI PDCCH grant fails
    let err_res = engine.notify_pdcch_recovery_response(0x9999, BeamIdentifier::csi_rs(8));
    assert!(err_res.is_err());

    // Correct C-RNTI response succeeds
    let ok_res = engine.notify_pdcch_recovery_response(ue_crnti, BeamIdentifier::csi_rs(8));
    assert_eq!(
        ok_res,
        Ok(BfrEvent::RecoverySuccess {
            old_beam: BeamIdentifier::ssb(0),
            new_beam: BeamIdentifier::csi_rs(8),
        })
    );

    // Active beam updated, timers cleared, state is Recovered
    assert_eq!(engine.current_active_tci_beam, BeamIdentifier::csi_rs(8));
    assert_eq!(engine.bfi_counter, 0);
    assert_eq!(engine.bfd_timer_remaining, 0);
    assert_eq!(engine.bfr_timer_remaining, 0);
    assert_eq!(engine.successful_recoveries, 1);
    assert_eq!(
        engine.state,
        BfrState::Recovered {
            active_tci_beam: BeamIdentifier::csi_rs(8)
        }
    );
}

#[test]
fn test_nr_bfr_mac_ce_serialization_and_multi_cell() {
    // 1. Single-Entry BFR MAC CE (TS 38.321 §6.1.3.23)
    let candidate = BeamIdentifier::csi_rs(45);
    let cell_idx = 3;
    let bytes = NrBfrEngine::format_single_entry_bfr_mac_ce(cell_idx, &candidate);
    assert_eq!(bytes.len(), 2);

    let (parsed_cell, parsed_cand) = NrBfrEngine::parse_single_entry_bfr_mac_ce(&bytes)
        .expect("Single-entry BFR MAC CE parse should succeed");
    assert_eq!(parsed_cell, cell_idx);
    assert_eq!(parsed_cand.signal_type, ReferenceSignalType::CsiRs);
    assert_eq!(parsed_cand.id, 45);

    // Test SSB variant
    let ssb_candidate = BeamIdentifier::ssb(28);
    let ssb_bytes = NrBfrEngine::format_single_entry_bfr_mac_ce(7, &ssb_candidate);
    let (scell_parsed, ssb_parsed) = NrBfrEngine::parse_single_entry_bfr_mac_ce(&ssb_bytes)
        .expect("SSB Single-entry BFR MAC CE parse should succeed");
    assert_eq!(scell_parsed, 7);
    assert_eq!(ssb_parsed.signal_type, ReferenceSignalType::Ssb);
    assert_eq!(ssb_parsed.id, 28);

    // 2. Multiple-Entry BFR MAC CE for Carrier Aggregation
    let failed_cells = vec![
        (1u8, Some(BeamIdentifier::ssb(14))),
        (4u8, Some(BeamIdentifier::csi_rs(7))),
        (6u8, None), // Beam failed, no suitable candidate meeting Q_in
    ];

    let multi_bytes = NrBfrEngine::format_multiple_entry_bfr_mac_ce(&failed_cells);
    // Bitmap should have bits 1, 4, 6 set -> (1<<1) | (1<<4) | (1<<6) = 2 | 16 | 64 = 82 = 0x52
    assert_eq!(multi_bytes[0], 0x52);
    // Total len = 1 byte bitmap + 3 candidate octets = 4 bytes
    assert_eq!(multi_bytes.len(), 4);

    let parsed_multi = NrBfrEngine::parse_multiple_entry_bfr_mac_ce(&multi_bytes)
        .expect("Multiple-entry BFR MAC CE parse should succeed");
    assert_eq!(parsed_multi.len(), 3);

    // Cell 1
    assert_eq!(parsed_multi[0].0, 1);
    assert_eq!(
        parsed_multi[0].1,
        Some(BeamIdentifier {
            signal_type: ReferenceSignalType::Ssb,
            id: 14
        })
    );

    // Cell 4
    assert_eq!(parsed_multi[1].0, 4);
    assert_eq!(
        parsed_multi[1].1,
        Some(BeamIdentifier {
            signal_type: ReferenceSignalType::CsiRs,
            id: 7
        })
    );

    // Cell 6
    assert_eq!(parsed_multi[2].0, 6);
    assert_eq!(parsed_multi[2].1, None);
}

#[test]
fn test_nr_bfr_recovery_timer_expiration_and_rlf() {
    let config = BeamFailureRecoveryConfig {
        bfi_max_count: 1, // Single BFI triggers recovery
        bfd_timer_slots: 5,
        bfr_timer_slots: 4, // 4-slot recovery window
        q_out_threshold_dbm: -100,
        q_in_threshold_dbm: -90,
        q0_serving_beams: vec![BeamIdentifier::ssb(0)],
        q1_candidate_beams: vec![CandidateBeamConfig {
            beam: BeamIdentifier::ssb(1),
            dedicated_preamble_index: Some(8),
            prach_occasion_slot: Some(10),
        }],
        recovery_search_space_id: 1,
    };

    let mut engine = NrBfrEngine::new(0x7777, BeamIdentifier::ssb(0), config);

    let degraded = [BeamMeasurement {
        beam: BeamIdentifier::ssb(0),
        rsrp_dbm: -120,
    }];
    let candidate = [BeamMeasurement {
        beam: BeamIdentifier::ssb(1),
        rsrp_dbm: -80,
    }];

    let res = engine.process_l1_measurements(&degraded, &candidate);
    assert!(matches!(
        res,
        Some(BfrEvent::RecoveryRequestDispatched { .. })
    ));
    assert_eq!(engine.bfr_timer_remaining, 4);
    assert!(matches!(engine.state, BfrState::AwaitingResponse { .. }));

    // Step 1 to 3: timer counts down
    assert_eq!(engine.step_slot(), None);
    assert_eq!(engine.bfr_timer_remaining, 3);
    assert_eq!(engine.step_slot(), None);
    assert_eq!(engine.bfr_timer_remaining, 2);
    assert_eq!(engine.step_slot(), None);
    assert_eq!(engine.bfr_timer_remaining, 1);

    // Step 4: timer reaches 0 -> Radio Link Failure!
    let rlf_event = engine.step_slot();
    assert_eq!(rlf_event, Some(BfrEvent::RadioLinkFailureDeclared));
    assert_eq!(engine.state, BfrState::RadioLinkFailure);
    assert_eq!(engine.rlf_events, 1);

    // Further measurements are ignored in RLF state
    assert_eq!(engine.process_l1_measurements(&degraded, &candidate), None);
}
