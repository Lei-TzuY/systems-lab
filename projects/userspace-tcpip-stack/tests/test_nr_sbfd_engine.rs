//! Integration tests for 3GPP Rel-18 5G-Advanced Subband Non-Overlapping Full Duplex (SBFD) Engine.
//!
//! Verifies SBFD slot structure, multi-stage Self-Interference Cancellation (SIC),
//! Cross-Link Interference (CLI) pathloss models, ultra-low latency scheduling,
//! link adaptation (MCS mapping), and error boundary handling in pure standard Rust.

use toy_tcpip::nr_sbfd_engine::*;

#[test]
fn test_sbfd_subband_allocation_and_guard_band() {
    let carrier_prbs = 273; // 100 MHz at 30 kHz SCS
    let fc = 3_500_000_000.0;
    let scs = 30_000.0;

    // Subband 1: Downlink (PRB 0..=139 = 140 PRBs)
    let dl_subband = SbfdSubband::new(1, SbfdSubbandType::Downlink, 0, 140, fc, scs, carrier_prbs);
    assert_eq!(dl_subband.start_prb, 0);
    assert_eq!(dl_subband.end_prb(), 139);
    assert_eq!(dl_subband.num_prbs, 140);

    // Guard Band: PRB 140..=149 (10 PRBs)
    let guard_subband = SbfdSubband::new(
        2,
        SbfdSubbandType::GuardBand,
        140,
        10,
        fc,
        scs,
        carrier_prbs,
    );
    assert_eq!(guard_subband.start_prb, 140);
    assert_eq!(guard_subband.end_prb(), 149);

    // Subband 3: Uplink (PRB 150..=272 = 123 PRBs)
    let ul_subband = SbfdSubband::new(3, SbfdSubbandType::Uplink, 150, 123, fc, scs, carrier_prbs);
    assert_eq!(ul_subband.start_prb, 150);
    assert_eq!(ul_subband.end_prb(), 272);
    assert_eq!(ul_subband.num_prbs, 123);

    // Check separation between DL and UL: 150 - 139 - 1 = 10 PRBs >= min 6 PRBs
    assert!(!dl_subband.overlaps_with(&guard_subband));
    assert!(!guard_subband.overlaps_with(&ul_subband));
    assert!(!dl_subband.overlaps_with(&ul_subband));

    // Construct valid SBFD slot config
    let slot_cfg = SbfdSlotConfig::new(
        1,
        SbfdSlotType::SbfdFullDuplex,
        carrier_prbs,
        vec![dl_subband, guard_subband, ul_subband],
        6, // minimum 6 guard PRBs
    )
    .expect("SBFD slot configuration must be valid");

    assert_eq!(slot_cfg.total_dl_prbs(), 140);
    assert_eq!(slot_cfg.total_ul_prbs(), 123);
}

#[test]
fn test_three_stage_self_interference_cancellation() {
    let sic = SelfInterferenceCancellationModel::new(
        48.0, // Stage 1: Spatial cross-polarization isolation (48 dB)
        36.0, // Stage 2: Analog RF vector attenuation (36 dB)
        36.0, // Stage 3: Digital Volterra baseband filter (36 dB)
    );

    // Total cancellation: 48 + 36 + 36 = 120 dB
    assert_eq!(sic.total_cancellation_db(), 120.0);

    // gNodeB transmitting at +46 dBm (40 W)
    let tx_power_dbm = 46.0;
    let rsi_dbm = sic.calculate_residual_self_interference_dbm(tx_power_dbm);

    // RSI = 46 - 120 = -74 dBm
    assert_eq!(rsi_dbm, -74.0);

    // Verify default SIC model
    let def_sic = SelfInterferenceCancellationModel::default();
    assert_eq!(def_sic.total_cancellation_db(), 115.0);
    assert_eq!(
        def_sic.calculate_residual_self_interference_dbm(46.0),
        -69.0
    );
}

#[test]
fn test_effective_sinr_and_noise_rise() {
    let num_prbs = 50; // 50 PRBs = 18 MHz
    let nf_db = 5.0;

    // Thermal noise: -174 + 10*log10(18e6) + 5 = -96.447 dBm
    let thermal_noise_dbm =
        SelfInterferenceCancellationModel::calculate_thermal_noise_dbm(num_prbs, nf_db);
    assert!((thermal_noise_dbm - (-96.447)).abs() < 0.01);

    // High SIC scenario: 125 dB total cancellation -> RSI = 46 - 125 = -79 dBm
    let sic = SelfInterferenceCancellationModel::new(50.0, 38.0, 37.0);
    let noise_rise_db = sic.calculate_noise_floor_rise_db(46.0, num_prbs, nf_db);

    // Noise rise should be strictly positive
    assert!(noise_rise_db > 0.0);
}

#[test]
fn test_cross_link_interference_gnb_and_ue() {
    let cli = CrossLinkInterferenceModel::new(3.5); // 3.5 GHz carrier

    // 1. gNB-to-gNB Line-of-Sight CLI at 500 meters
    let pl_gnb = cli.gnb_to_gnb_pathloss_db(500.0);
    // PL = 32.4 + 20*log10(3.5) + 20*log10(500) = 32.4 + 10.881 + 53.979 = 97.26 dB
    assert!((pl_gnb - 97.26).abs() < 0.1);

    let gnb_cli_power = cli.calculate_gnb_cli_power_dbm(46.0, 500.0);
    assert!((gnb_cli_power - (-51.26)).abs() < 0.1);

    // 2. UE-to-UE Non-Line-of-Sight CLI at 20 meters
    let pl_ue = cli.ue_to_ue_pathloss_db(20.0);
    // PL = 35.3 + 22.4*log10(20) + 21.3*log10(3.5) = 35.3 + 29.143 + 11.589 = 76.03 dB
    assert!((pl_ue - 76.03).abs() < 0.1);

    let ue_cli_power = cli.calculate_ue_cli_power_dbm(23.0, 20.0);
    assert!((ue_cli_power - (-53.03)).abs() < 0.1);
}

#[test]
fn test_sbfd_urgent_ul_scheduling_latency_reduction() {
    let carrier_prbs = 273;
    let fc = 3_500_000_000.0;
    let scs = 30_000.0;

    let sic = SelfInterferenceCancellationModel::new(50.0, 42.0, 43.0); // 135 dB cancellation -> RSI = -89 dBm
    let cli = CrossLinkInterferenceModel::new(3.5);

    let mut engine = SbfdEngine::new(carrier_prbs, 46.0, 5.0, sic, cli);

    // Define standard 5-slot TDD pattern:
    // Slot 0: LegacyDl
    // Slot 1: SbfdFullDuplex (DL 140 PRB, Guard 10 PRB, UL 123 PRB)
    // Slot 2: LegacyDl
    // Slot 3: LegacyDl
    // Slot 4: LegacyUl
    let dl_sub = SbfdSubband::new(1, SbfdSubbandType::Downlink, 0, 140, fc, scs, carrier_prbs);
    let guard_sub = SbfdSubband::new(
        2,
        SbfdSubbandType::GuardBand,
        140,
        10,
        fc,
        scs,
        carrier_prbs,
    );
    let ul_sub = SbfdSubband::new(3, SbfdSubbandType::Uplink, 150, 123, fc, scs, carrier_prbs);

    engine.add_slot_config(
        SbfdSlotConfig::new(0, SbfdSlotType::LegacyDl, carrier_prbs, Vec::new(), 6).unwrap(),
    );
    engine.add_slot_config(
        SbfdSlotConfig::new(
            1,
            SbfdSlotType::SbfdFullDuplex,
            carrier_prbs,
            vec![dl_sub, guard_sub, ul_sub],
            6,
        )
        .unwrap(),
    );
    engine.add_slot_config(
        SbfdSlotConfig::new(2, SbfdSlotType::LegacyDl, carrier_prbs, Vec::new(), 6).unwrap(),
    );
    engine.add_slot_config(
        SbfdSlotConfig::new(3, SbfdSlotType::LegacyDl, carrier_prbs, Vec::new(), 6).unwrap(),
    );
    engine.add_slot_config(
        SbfdSlotConfig::new(4, SbfdSlotType::LegacyUl, carrier_prbs, Vec::new(), 6).unwrap(),
    );

    // Scenario A: Urgent UL packet arrives at Slot 1 (SbfdFullDuplex)
    // Must be scheduled IMMEDIATELY in slot 1 (0.0 ms wait)!
    let decision_sbfd = engine
        .schedule_urgent_ul(1, -75.0, None)
        .expect("SBFD scheduling must succeed");

    assert_eq!(decision_sbfd.slot_number, 1);
    assert_eq!(decision_sbfd.is_sbfd, true);
    assert_eq!(decision_sbfd.allocated_prbs, 123);
    assert_eq!(decision_sbfd.wait_latency_ms, 0.0);
    assert!(decision_sbfd.transport_block_bytes > 1000);

    // Scenario B: Urgent UL packet arrives at Slot 2 (LegacyDl)
    // Must wait until Slot 4 (LegacyUl) = 2 slots wait = 1.0 ms!
    let decision_dl = engine
        .schedule_urgent_ul(2, -75.0, None)
        .expect("Legacy fallback scheduling must succeed");

    assert_eq!(decision_dl.slot_number, 4);
    assert_eq!(decision_dl.is_sbfd, false);
    assert_eq!(decision_dl.allocated_prbs, 273);
    assert_eq!(decision_dl.wait_latency_ms, 1.0); // 2 slots * 0.5 ms

    // Verify metrics
    assert_eq!(engine.metrics.total_slots_processed, 2);
    assert_eq!(engine.metrics.sbfd_slots_count, 1);
    assert_eq!(engine.metrics.legacy_dl_slots_count, 1);
}

#[test]
fn test_dynamic_link_adaptation_and_spectral_efficiency() {
    // 1. High SINR (28 dB) -> MCS 27 (256QAM)
    let mcs27 = SbfdLinkAdapter::select_mcs(28.0).expect("Must select MCS 27");
    assert_eq!(mcs27.mcs_index, 27);
    assert_eq!(mcs27.modulation_order, 8); // 256QAM
    assert!((mcs27.spectral_efficiency_bits_s_hz - 6.5703).abs() < 0.001);

    // 2. Medium SINR (16 dB) -> MCS 19 (64QAM)
    let mcs19 = SbfdLinkAdapter::select_mcs(16.0).expect("Must select MCS 19");
    assert_eq!(mcs19.mcs_index, 19);
    assert_eq!(mcs19.modulation_order, 6); // 64QAM

    // 3. Low SINR (4 dB) -> MCS 9 (QPSK)
    let mcs9 = SbfdLinkAdapter::select_mcs(4.0).expect("Must select MCS 9");
    assert_eq!(mcs9.mcs_index, 9);
    assert_eq!(mcs9.modulation_order, 2); // QPSK

    // 4. Edge coverage (-5 dB) -> MCS 0 (QPSK)
    let mcs0 = SbfdLinkAdapter::select_mcs(-5.0).expect("Must select MCS 0");
    assert_eq!(mcs0.mcs_index, 0);

    // 5. Out of coverage (-12 dB) -> None
    assert!(SbfdLinkAdapter::select_mcs(-12.0).is_none());
}

#[test]
fn test_error_handling_and_boundary_checks() {
    let carrier_prbs = 273;
    let fc = 3_500_000_000.0;
    let scs = 30_000.0;

    // 1. PRB out of bounds
    let oob_sub = SbfdSubband::new(1, SbfdSubbandType::Downlink, 250, 50, fc, scs, carrier_prbs); // ends at 299 > 273
    let err_oob =
        SbfdSlotConfig::new(0, SbfdSlotType::LegacyDl, carrier_prbs, vec![oob_sub], 6).unwrap_err();
    assert!(matches!(
        err_oob,
        SbfdError::CarrierPrbOutOfBounds {
            requested: 299,
            max_prbs: 273
        }
    ));
    assert!(format!("{}", err_oob).contains("exceeds carrier bandwidth"));

    // 2. Overlapping subbands
    let sub_a = SbfdSubband::new(1, SbfdSubbandType::Downlink, 0, 100, fc, scs, carrier_prbs);
    let sub_b = SbfdSubband::new(2, SbfdSubbandType::Uplink, 50, 100, fc, scs, carrier_prbs);
    let err_overlap = SbfdSlotConfig::new(
        0,
        SbfdSlotType::SbfdFullDuplex,
        carrier_prbs,
        vec![sub_a, sub_b],
        6,
    )
    .unwrap_err();
    assert!(matches!(
        err_overlap,
        SbfdError::PrbOverlap {
            subband_a: 1,
            subband_b: 2,
            prb: 50
        }
    ));
    assert!(format!("{}", err_overlap).contains("overlap at PRB 50"));

    // 3. Insufficient guard band
    let sub_dl = SbfdSubband::new(1, SbfdSubbandType::Downlink, 0, 100, fc, scs, carrier_prbs); // ends 99
    let sub_ul = SbfdSubband::new(2, SbfdSubbandType::Uplink, 103, 100, fc, scs, carrier_prbs); // starts 103 -> gap = 3 PRBs < 6 required
    let err_guard = SbfdSlotConfig::new(
        0,
        SbfdSlotType::SbfdFullDuplex,
        carrier_prbs,
        vec![sub_dl, sub_ul],
        6,
    )
    .unwrap_err();
    assert!(matches!(
        err_guard,
        SbfdError::InsufficientGuardBand {
            actual_prbs: 3,
            required_prbs: 6
        }
    ));
    assert!(format!("{}", err_guard).contains("Insufficient SBFD guard band: 3 PRBs"));

    // 4. SIC Breakdown error formatting
    let err_sic = SbfdError::SicFailure {
        tx_power_dbm: 46.0,
        rsi_dbm: -60.0,
        max_tolerable_dbm: -85.0,
    };
    assert!(
        format!("{}", err_sic).contains("SIC breakdown: Tx power 46.0 dBm yields RSI -60.0 dBm")
    );

    // 5. Constants verification
    assert_eq!(SBFD_DEFAULT_SCS_HZ, 30_000.0);
    assert_eq!(SBFD_SUBCARRIERS_PER_PRB, 12);
    assert_eq!(SBFD_PRB_BANDWIDTH_30KHZ_HZ, 360_000.0);
    assert_eq!(SBFD_MAX_PRBS_100MHZ_30KHZ, 273);
    assert_eq!(SBFD_DEFAULT_MIN_GUARD_PRBS, 6);
    assert_eq!(SBFD_DEFAULT_GNB_TX_POWER_DBM, 46.0);
    assert_eq!(SBFD_DEFAULT_UE_TX_POWER_DBM, 23.0);
    assert_eq!(SBFD_MAX_TOLERABLE_RSI_DBM, -85.0);
}
