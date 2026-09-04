//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR Dual Connectivity & Fast SCG Engine.

use toy_tcpip::nr_scg_engine::*;

#[test]
fn test_nr_scg_mac_ce_serialization_and_parsing() {
    // 1. Activate SCG with SCell 1 and SCell 3 enabled (bitmap = 0x05)
    let byte_act = NrScgEngine::format_scg_activation_mac_ce(true, 0x05);
    assert_eq!(byte_act, 0x85); // 0x80 | 0x05

    let (act, bitmap) = NrScgEngine::parse_scg_activation_mac_ce(byte_act);
    assert!(act);
    assert_eq!(bitmap, 0x05);

    // 2. Deactivate SCG with all SCells cleared
    let byte_deact = NrScgEngine::format_scg_activation_mac_ce(false, 0x00);
    assert_eq!(byte_deact, 0x00);

    let (deact, deact_bitmap) = NrScgEngine::parse_scg_activation_mac_ce(byte_deact);
    assert!(!deact);
    assert_eq!(deact_bitmap, 0x00);

    // 3. Activate SCG with all 7 SCells enabled (bitmap = 0x7F)
    let byte_all = NrScgEngine::format_scg_activation_mac_ce(true, 0x7F);
    assert_eq!(byte_all, 0xFF);

    let (act_all, bitmap_all) = NrScgEngine::parse_scg_activation_mac_ce(byte_all);
    assert!(act_all);
    assert_eq!(bitmap_all, 0x7F);

    // 4. Deactivate SCG while preserving arbitrary SCell bitmap bits
    let byte_deact_custom = NrScgEngine::format_scg_activation_mac_ce(false, 0x3A);
    assert_eq!(byte_deact_custom, 0x3A);

    let (deact_cust, bitmap_cust) = NrScgEngine::parse_scg_activation_mac_ce(byte_deact_custom);
    assert!(!deact_cust);
    assert_eq!(bitmap_cust, 0x3A);
}

#[test]
fn test_nr_scg_fast_deactivation_quiescent_state() {
    let pscell = ScgCellConfig {
        pci: 100,
        arfcn: 632000,
        serv_cell_index: 1,
        timing_advance: 31,
        is_pscell: true,
        active: true,
    };

    let config = ScgEngineConfig {
        activation_delay_sync_slots: 8,
        activation_delay_rach_slots: 24,
        deactivation_flush_slots: 2,
        ul_buffer_activation_threshold: 1500,
    };

    let mut engine = NrScgEngine::new(0x2345, pscell, config);

    // Add an SCell
    let scell = ScgCellConfig {
        pci: 101,
        arfcn: 633000,
        serv_cell_index: 2,
        timing_advance: 31,
        is_pscell: false,
        active: true,
    };
    engine.add_scell(scell);

    // Verify initial active state
    assert_eq!(engine.state, ScgState::Activated);
    assert!(engine.is_pdcch_monitored());
    assert!(engine.is_csi_reporting_active());
    assert!(engine.is_srs_active());
    assert!(engine.is_pusch_enabled());

    // Send SCG Deactivation MAC CE
    let deact_ce = NrScgEngine::format_scg_activation_mac_ce(false, 0x00);
    let event = engine.handle_mac_ce(deact_ce);
    assert!(matches!(event, Some(ScgEngineEvent::StateChanged { .. })));

    match engine.state {
        ScgState::Deactivating { slots_remaining } => assert_eq!(slots_remaining, 2),
        ref other => panic!("Expected Deactivating state, got: {:?}", other),
    }

    // Step 1 slot
    assert_eq!(engine.step_slot(), None);
    assert!(matches!(
        engine.state,
        ScgState::Deactivating { slots_remaining: 1 }
    ));

    // Step 2nd slot -> DeactivationCompleted
    let deact_evt = engine.step_slot();
    assert_eq!(deact_evt, Some(ScgEngineEvent::DeactivationCompleted));
    assert_eq!(engine.state, ScgState::Deactivated);
    assert_eq!(engine.total_deactivations, 1);

    // Verify Quiescent RF indicators
    assert!(!engine.is_pdcch_monitored());
    assert!(!engine.is_csi_reporting_active());
    assert!(!engine.is_srs_active());
    assert!(!engine.is_pusch_enabled());

    // Verify RRC configuration and identities are fully preserved!
    assert_eq!(engine.pscell.pci, 100);
    assert_eq!(engine.pscell.arfcn, 632000);
    assert_eq!(engine.scells[0].pci, 101);
    assert_eq!(engine.scells[0].arfcn, 633000);
}

#[test]
fn test_nr_scg_fast_activation_delay_and_resumption() {
    let pscell = ScgCellConfig {
        pci: 200,
        arfcn: 632000,
        serv_cell_index: 1,
        timing_advance: 15,
        is_pscell: true,
        active: false,
    };

    let config = ScgEngineConfig {
        activation_delay_sync_slots: 6,
        activation_delay_rach_slots: 18,
        deactivation_flush_slots: 1,
        ul_buffer_activation_threshold: 1000,
    };

    let mut engine = NrScgEngine::new(0x5678, pscell, config);
    // Move immediately to deactivated state
    engine.state = ScgState::Deactivated;

    let scell = ScgCellConfig {
        pci: 202,
        arfcn: 634000,
        serv_cell_index: 2,
        timing_advance: 15,
        is_pscell: false,
        active: false,
    };
    engine.add_scell(scell);

    // 1. MAC CE Fast Activation (synchronized PSCell, 6 slots delay)
    // SCell index 2 activated via bit 1 (1 << (2-1) = 2)
    let mac_ce = NrScgEngine::format_scg_activation_mac_ce(true, 0x02);
    let evt = engine.handle_mac_ce(mac_ce);
    assert!(matches!(evt, Some(ScgEngineEvent::StateChanged { .. })));

    match engine.state {
        ScgState::Activating {
            slots_remaining,
            needs_rach,
        } => {
            assert_eq!(slots_remaining, 6);
            assert!(!needs_rach);
        }
        ref other => panic!("Expected Activating state, got: {:?}", other),
    }

    // Step 5 slots: still activating
    for _ in 0..5 {
        assert_eq!(engine.step_slot(), None);
        assert!(!engine.is_pdcch_monitored());
    }

    // Step 6th slot: activation completes!
    let done_evt = engine.step_slot();
    assert_eq!(done_evt, Some(ScgEngineEvent::ActivationCompleted));
    assert_eq!(engine.state, ScgState::Activated);
    assert!(engine.is_pdcch_monitored());
    assert!(engine.is_csi_reporting_active());
    assert!(engine.is_srs_active());
    assert!(engine.is_pusch_enabled());
    assert!(engine.pscell.active);
    assert!(engine.scells[0].active);

    // 2. RRC Reconfiguration Activation with RACH (18 slots delay)
    engine.state = ScgState::Deactivated;
    let rrc_evt = engine.handle_rrc_reconfiguration_state(true);
    assert!(matches!(rrc_evt, Some(ScgEngineEvent::StateChanged { .. })));

    match engine.state {
        ScgState::Activating {
            slots_remaining,
            needs_rach,
        } => {
            assert_eq!(slots_remaining, 18);
            assert!(needs_rach);
        }
        ref other => panic!("Expected Activating with RACH, got: {:?}", other),
    }

    for _ in 0..17 {
        assert_eq!(engine.step_slot(), None);
    }
    assert_eq!(
        engine.step_slot(),
        Some(ScgEngineEvent::ActivationCompleted)
    );
    assert_eq!(engine.state, ScgState::Activated);
}

#[test]
fn test_nr_scg_ul_data_arrival_and_sr_trigger() {
    let pscell = ScgCellConfig {
        pci: 300,
        arfcn: 632000,
        serv_cell_index: 1,
        timing_advance: 20,
        is_pscell: true,
        active: false,
    };

    let config = ScgEngineConfig {
        activation_delay_sync_slots: 8,
        activation_delay_rach_slots: 24,
        deactivation_flush_slots: 2,
        ul_buffer_activation_threshold: 1500,
    };

    let mut engine = NrScgEngine::new(0x3344, pscell, config);
    engine.state = ScgState::Deactivated;

    // Register a split bearer (DRB 3)
    engine.add_bearer(ScgBearerConfig {
        drb_id: 3,
        bearer_type: ScgBearerType::SplitBearer,
        security_key_id: 1,
        pdcp_sn_bits: 18,
    });

    // 1. Low volume UL data arrival (800 bytes < 1500 threshold): no trigger
    let no_evt = engine.handle_ul_data_arrival(3, 800);
    assert_eq!(no_evt, None);

    // 2. High volume UL data arrival (3000 bytes >= 1500 threshold): triggers SR on MCG!
    let sr_evt = engine.handle_ul_data_arrival(3, 3000);
    assert_eq!(
        sr_evt,
        Some(ScgEngineEvent::SrTriggeredOnMcg {
            drb_id: 3,
            buffer_bytes: 3000,
        })
    );

    // 3. When SCG is already active, UL data does NOT trigger SR on MCG
    engine.state = ScgState::Activated;
    let active_res = engine.handle_ul_data_arrival(3, 5000);
    assert_eq!(active_res, None);
}

#[test]
fn test_nr_scg_failure_information_reporting_and_power_metrics() {
    let pscell = ScgCellConfig {
        pci: 400,
        arfcn: 632000,
        serv_cell_index: 1,
        timing_advance: 10,
        is_pscell: true,
        active: true,
    };

    let config = ScgEngineConfig::default();
    let mut engine = NrScgEngine::new(0x9900, pscell, config);

    // 1. Simulate 25 active slots
    for _ in 0..25 {
        engine.step_slot();
    }
    assert_eq!(engine.active_slots, 25);
    assert_eq!(engine.deactivated_slots, 0);
    assert_eq!(engine.get_power_savings_percentage(), 0.0);

    // Transition to Deactivated
    engine.state = ScgState::Deactivated;

    // Simulate 75 deactivated slots
    for _ in 0..75 {
        engine.step_slot();
    }
    assert_eq!(engine.active_slots, 25);
    assert_eq!(engine.deactivated_slots, 75);
    // 75 / (25 + 75) = 75.0%
    assert!((engine.get_power_savings_percentage() - 75.0).abs() < 1e-6);

    // 2. Trigger SCG Failure (T310 expiry)
    let fail_evt = engine.trigger_scg_failure(
        ScgFailureReason::T310Expiry,
        Some(-122), // RSRP in dBm
        Some(-19),  // RSRQ in dB
    );

    match fail_evt {
        ScgEngineEvent::ScgFailureReported(info) => {
            assert_eq!(info.failure_type, ScgFailureReason::T310Expiry);
            assert_eq!(info.failed_pscell_pci, 400);
            assert_eq!(info.meas_result_pscell_rsrp, Some(-122));
            assert_eq!(info.meas_result_pscell_rsrq, Some(-19));
        }
        ref other => panic!("Expected ScgFailureReported, got: {:?}", other),
    }

    assert_eq!(engine.failure_count, 1);
    assert_eq!(engine.state, ScgState::Deactivated);
    assert!(!engine.pscell.active);
}
