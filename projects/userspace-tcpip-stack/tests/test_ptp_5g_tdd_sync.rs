//! Integration tests for 3GPP TS 38.104 5G NR TDD & Time Alignment Error (TAE) Synchronization Conformance Engine.

use toy_tcpip::ptp_5g_tdd_sync::{
    AntennaPortMeasurement, FronthaulBudgetPartition, NrTddSyncCategory, NrTddSyncEngine,
};

#[test]
fn test_5g_tdd_absolute_cell_sync_pass_and_violation() {
    let mut engine = NrTddSyncEngine::default();

    // 3 antenna ports with healthy absolute time error <= 1500 ns
    engine.add_measurement(AntennaPortMeasurement::new(1, 1, 3500.0, 250));
    engine.add_measurement(AntennaPortMeasurement::new(2, 1, 3500.0, -400));
    engine.add_measurement(AntennaPortMeasurement::new(3, 1, 3500.0, 750));

    let report1 = engine.evaluate_absolute_cell_sync();
    assert!(report1.is_compliant);
    assert_eq!(report1.max_abs_te_ns, 750);
    assert!(report1.violating_ports.is_empty());
    assert_eq!(report1.total_ports_evaluated, 3);

    // Port 4 experiences sync loss with TE = +1650 ns (> 1500 ns limit)
    engine.add_measurement(AntennaPortMeasurement::new(4, 1, 3500.0, 1650));
    let report2 = engine.evaluate_absolute_cell_sync();
    assert!(!report2.is_compliant);
    assert_eq!(report2.max_abs_te_ns, 1650);
    assert_eq!(report2.violating_ports.len(), 1);
    assert_eq!(report2.violating_ports[0], (4, 1650));
}

#[test]
fn test_5g_tdd_mimo_antenna_tae_strict_compliance() {
    let mut engine = NrTddSyncEngine::default();

    // 4T4R MIMO array in antenna group 1 (n78 band 3.5 GHz)
    // 3GPP 38.104 Section 6.5.3 requires TAE <= 65 ns
    engine.add_measurement(AntennaPortMeasurement::new(101, 1, 3500.0, 120));
    engine.add_measurement(AntennaPortMeasurement::new(102, 1, 3500.0, 145));
    engine.add_measurement(AntennaPortMeasurement::new(103, 1, 3500.0, 110));
    engine.add_measurement(AntennaPortMeasurement::new(104, 1, 3500.0, 155));

    let report_pass = engine
        .evaluate_group_tae(1, NrTddSyncCategory::MimoTransmission)
        .expect("MIMO report");

    // Worst pair = (103, 104): |110 - 155| = 45 ns <= 65 ns
    assert_eq!(report_pass.max_measured_tae_ns, 45);
    assert_eq!(report_pass.allowed_limit_ns, 65);
    assert!(report_pass.is_compliant);
    assert_eq!(report_pass.port_count, 4);

    // Degrade Port 104 calibration drift to +185 ns:
    // New worst pair = |185 - 110| = 75 ns > 65 ns
    engine.add_measurement(AntennaPortMeasurement::new(104, 1, 3500.0, 185));
    let report_fail = engine
        .evaluate_group_tae(1, NrTddSyncCategory::MimoTransmission)
        .expect("MIMO report failed");

    assert_eq!(report_fail.max_measured_tae_ns, 75);
    assert!(!report_fail.is_compliant);
    assert_eq!(report_fail.worst_pair, (103, 104));
}

#[test]
fn test_5g_tdd_carrier_aggregation_tae() {
    let mut engine = NrTddSyncEngine::default();

    // Group 2: Intra-band Contiguous Carrier Aggregation (CC1 + CC2)
    // 3GPP 38.104 requires TAE <= 260 ns between carrier components
    engine.add_measurement(AntennaPortMeasurement::new(201, 2, 3500.0, 180));
    engine.add_measurement(AntennaPortMeasurement::new(202, 2, 3600.0, 360));

    let report1 = engine
        .evaluate_group_tae(2, NrTddSyncCategory::IntraBandContiguousCa)
        .expect("CA report");

    // TAE = |360 - 180| = 180 ns <= 260 ns
    assert_eq!(report1.max_measured_tae_ns, 180);
    assert!(report1.is_compliant);

    // Increase CC2 port TE to +480 ns -> TAE = |480 - 180| = 300 ns > 260 ns
    engine.add_measurement(AntennaPortMeasurement::new(202, 2, 3600.0, 480));
    let report2 = engine
        .evaluate_group_tae(2, NrTddSyncCategory::IntraBandContiguousCa)
        .expect("CA report 2");

    assert_eq!(report2.max_measured_tae_ns, 300);
    assert!(!report2.is_compliant);
}

#[test]
fn test_5g_tdd_inter_cell_phase_sync() {
    let mut engine = NrTddSyncEngine::default();

    // Two adjacent cells in overlapping area
    // Basic TDD inter-cell relative phase error limit is 3000 ns
    engine.add_measurement(AntennaPortMeasurement::new(10, 1, 3500.0, 800));
    engine.add_measurement(AntennaPortMeasurement::new(20, 2, 3500.0, -600));

    // Relative difference = |800 - (-600)| = 1400 ns <= 3000 ns
    assert!(engine.evaluate_inter_cell_phase_sync(3000).is_ok());

    // Severe clock drift on cell 20 to -2500 ns
    // Relative difference = |800 - (-2500)| = 3300 ns > 3000 ns
    engine.add_measurement(AntennaPortMeasurement::new(20, 2, 3500.0, -2500));
    let result = engine.evaluate_inter_cell_phase_sync(3000);
    assert!(result.is_err());
    let (p1, p2, diff) = result.unwrap_err();
    assert_eq!((p1, p2), (10, 20));
    assert_eq!(diff, 3300);
}

#[test]
fn test_fronthaul_budget_diagnostics() {
    let budget = FronthaulBudgetPartition {
        prtc_budget_ns: 100,
        transport_network_budget_ns: 800,
        ru_internal_budget_ns: 150,
        radio_margin_ns: 450,
    };
    let engine = NrTddSyncEngine::new(budget);
    assert_eq!(engine.budget.total_budget_ns(), 1500);

    // 1. Healthy system within all segment budgets
    let diag_pass = engine.diagnose_fronthaul_budget(40, 350, 90);
    assert!(diag_pass.is_total_compliant);
    assert_eq!(diag_pass.total_measured_te_ns, 480);
    assert!(!diag_pass.prtc_exceeded);
    assert!(!diag_pass.transport_exceeded);
    assert!(!diag_pass.ru_exceeded);
    assert_eq!(diag_pass.bottleneck_segment, None);

    // 2. Transport network jitter bottleneck (> 800 ns)
    let diag_transport = engine.diagnose_fronthaul_budget(50, 920, 100);
    assert!(diag_transport.is_total_compliant); // 50 + 920 + 100 = 1070 <= 1500 total
    assert!(diag_transport.transport_exceeded); // but segment budget exceeded!
    assert_eq!(
        diag_transport.bottleneck_segment,
        Some("Fronthaul Transport Network (T-BC / Packet Jitter)")
    );

    // 3. PRTC grandmaster GPS loss / antenna multipath failure (> 100 ns)
    let diag_prtc = engine.diagnose_fronthaul_budget(180, 200, 80);
    assert!(diag_prtc.prtc_exceeded);
    assert_eq!(
        diag_prtc.bottleneck_segment,
        Some("PRTC / Primary Reference Grandmaster Clock")
    );
}
