//! Integration Tests for 3GPP Rel-18 High-Speed Train Multi-TRP Doppler & SFN Engine.

use toy_tcpip::nr_hst_sfn::*;

#[test]
fn test_hst_train_kinematics_and_geometry() {
    let speed_kmh = 500.0;
    let train = TrainKinematics::new("CR400AF_Hexie", 0.0, speed_kmh, 3.0);

    // Speed in m/s should be 500 / 3.6 = 138.88889 m/s
    assert!((train.velocity_mps - 138.888889).abs() < 1e-4);
    assert!((train.speed_kmh() - 500.0).abs() < 1e-4);

    let pole = TrackPoint::new(1000.0, 15.0, 20.0);
    let train_antenna = train.antenna_point();

    // Distance dx = 1000, dy = 15, dz = 17
    // dist = sqrt(1000^2 + 15^2 + 17^2) = sqrt(1000000 + 225 + 289) = sqrt(1000514) = 1000.257 m
    let dist = train_antenna.distance_to(&pole);
    assert!((dist - 1000.257).abs() < 0.01);
}

#[test]
fn test_dual_doppler_spectrum_approaching_vs_receding() {
    let speed_kmh = 500.0;
    let fc = 3.5e9; // 3.5 GHz FR1 carrier
    let scs_khz = 30; // 30 kHz SCS

    // Train placed exactly in the middle (x = 500m) between Pole 1 (x = 0m) and Pole 2 (x = 1000m)
    let train = TrainKinematics::new("Fuxing_Smart", 500.0, speed_kmh, 3.0);
    let mgr = HstSfnManager::new(
        HstScenario::OpenSpace,
        fc,
        scs_khz,
        train,
        1000.0, // 1 km pole spacing
        15.0,   // 15 m track offset
        20.0,   // 20 m antenna height
        3,
    )
    .expect("Failed to initialize HST SFN manager");

    // Theoretical maximum Doppler: (v * fc) / c
    let expected_max_fd = (138.8888889 * 3.5e9) / SPEED_OF_LIGHT_M_S;
    assert!(
        (mgr.max_doppler_hz() - expected_max_fd).abs() < 1e-2,
        "Max Doppler should be ~{:.2} Hz, got {:.2} Hz",
        expected_max_fd,
        mgr.max_doppler_hz()
    );

    let dual = mgr.compute_dual_doppler();

    // Approaching TRP (Pole 2 at x = 1000m) has positive Doppler
    assert!(
        dual.doppler_approaching_hz > 1600.0,
        "Approaching Doppler should be > 1600 Hz, got {:.2} Hz",
        dual.doppler_approaching_hz
    );

    // Receding TRP (Pole 1 at x = 0m) has negative Doppler
    assert!(
        dual.doppler_receding_hz < -1600.0,
        "Receding Doppler should be < -1600 Hz, got {:.2} Hz",
        dual.doppler_receding_hz
    );

    // Total Doppler spread across SFN is approaching minus receding (~3240 Hz)
    assert!(
        dual.doppler_spread_hz > 3200.0 && dual.doppler_spread_hz < 3300.0,
        "Doppler spread should be ~3240 Hz, got {:.2} Hz",
        dual.doppler_spread_hz
    );
}

#[test]
fn test_doppler_drift_rate_at_closest_approach() {
    let speed_kmh = 350.0; // 350 km/h
    let fc = 3.5e9;

    // Train starts before Pole 1 and passes it
    let mut train = TrainKinematics::new("Shinkansen_E5", -100.0, speed_kmh, 3.0);
    let mut mgr = HstSfnManager::new(
        HstScenario::OpenSpace,
        fc,
        30,
        train.clone(),
        1000.0,
        15.0,
        20.0,
        3,
    )
    .unwrap();

    // When approaching Pole 1 from negative X, Doppler to Pole 1 is positive
    let (_, _, fd_before) = mgr.calculate_trp_link(0);
    assert!(
        fd_before > 1000.0,
        "Doppler should be positive when approaching, got {:.2} Hz",
        fd_before
    );

    // Advance train past Pole 1 to x = +100m
    train.position_x_m = 100.0;
    mgr.train = train;

    let (_, _, fd_after) = mgr.calculate_trp_link(0);
    assert!(
        fd_after < -1000.0,
        "Doppler should be negative after passing pole, got {:.2} Hz",
        fd_after
    );
}

#[test]
fn test_sfn_differential_propagation_delay_and_cp_budget() {
    let speed_kmh = 350.0;
    let fc = 3.5e9;

    // 1. Train exactly in middle (x = 500m)
    let train_mid = TrainKinematics::new("ICE_4", 500.0, speed_kmh, 3.0);
    let mgr_mid = HstSfnManager::new(
        HstScenario::OpenSpace,
        fc,
        30, // 30 kHz SCS (CP = 2.34 us)
        train_mid,
        1000.0,
        15.0,
        20.0,
        3,
    )
    .unwrap();

    let delay_mid = mgr_mid.compute_sfn_delay_spread();
    assert!(
        delay_mid.delay_difference_us < 0.05,
        "Delay difference at midpoint should be ~0, got {:.4} us",
        delay_mid.delay_difference_us
    );
    assert!(!delay_mid.exceeds_cp);

    // 2. Train near pole 1 (x = 100m, so 100m from Pole 1, 900m from Pole 2)
    // Delay difference is approximately (900m - 100m) / c = 800 / 3e8 = 2.668 us
    let train_near = TrainKinematics::new("ICE_4", 100.0, speed_kmh, 3.0);

    // With 15 kHz SCS (CP = 4.69 us), 2.668 us fits within CP
    let mgr_15k = HstSfnManager::new(
        HstScenario::OpenSpace,
        fc,
        15,
        train_near.clone(),
        1000.0,
        15.0,
        20.0,
        3,
    )
    .unwrap();
    let delay_15k = mgr_15k.compute_sfn_delay_spread();
    assert!(delay_15k.delay_difference_us > 2.6 && delay_15k.delay_difference_us < 2.8);
    assert!(
        !delay_15k.exceeds_cp,
        "15 kHz CP (4.69 us) should accommodate 2.67 us"
    );

    // With 30 kHz SCS (CP = 2.34 us), 2.668 us EXCEEDS CP!
    let mgr_30k = HstSfnManager::new(
        HstScenario::OpenSpace,
        fc,
        30,
        train_near,
        1000.0,
        15.0,
        20.0,
        3,
    )
    .unwrap();
    let delay_30k = mgr_30k.compute_sfn_delay_spread();
    assert!(
        delay_30k.exceeds_cp,
        "30 kHz CP (2.34 us) must detect overflow when delay difference is {:.2} us",
        delay_30k.delay_difference_us
    );
}

#[test]
fn test_ici_mitigation_and_pre_compensation() {
    let train = TrainKinematics::new("TGV_Duplex", 500.0, 500.0, 3.0);
    let mgr = HstSfnManager::new(
        HstScenario::OpenSpace,
        3.5e9,
        30, // 30 kHz SCS
        train,
        1000.0,
        15.0,
        20.0,
        3,
    )
    .unwrap();

    // Mode 1: No compensation -> Full ~3.24 kHz Doppler spread
    let ici_none = mgr.compute_ici(HstCompensationMode::None);
    assert!(
        ici_none.residual_doppler_hz > 3200.0,
        "Uncompensated residual should be > 3200 Hz"
    );
    // Normalized ICI power: (pi^2 / 6) * (3240 / 30000)^2 = 1.6449 * 0.01166 = 0.01918 (-17.1 dB)
    assert!(
        ici_none.ici_power_ratio_db > -20.0,
        "Uncompensated ICI should be > -20 dB, got {:.2} dB",
        ici_none.ici_power_ratio_db
    );

    // Mode 2: TRP Pre-compensation -> residual Doppler reduced to ~2%
    let ici_pre = mgr.compute_ici(HstCompensationMode::TrpPreCompensation);
    assert!(
        ici_pre.residual_doppler_hz < 70.0,
        "Pre-compensated residual should be < 70 Hz, got {:.2} Hz",
        ici_pre.residual_doppler_hz
    );
    // ICI power ratio drops to below -40 dB
    assert!(
        ici_pre.ici_power_ratio_db < -40.0,
        "Pre-compensated ICI should be < -40 dB, got {:.2} dB",
        ici_pre.ici_power_ratio_db
    );

    // Mode 3: UE Dual-Branch Equalization -> residual Doppler ~5 Hz
    let ici_ue = mgr.compute_ici(HstCompensationMode::UeDualBranchEqualization);
    assert_eq!(ici_ue.residual_doppler_hz, 5.0);
    assert!(
        ici_ue.ici_power_ratio_db < -60.0,
        "UE dual-branch equalized ICI should be < -60 dB, got {:.2} dB",
        ici_ue.ici_power_ratio_db
    );
}

#[test]
fn test_active_sfn_trp_switching_along_track() {
    let train = TrainKinematics::new("AGV_Italo", 200.0, 360.0, 3.0); // 100 m/s
    let mut mgr = HstSfnManager::new(
        HstScenario::OpenSpace,
        3.5e9,
        30,
        train,
        1000.0, // Poles at 0, 1000, 2000, 3000, 4000 m
        15.0,
        20.0,
        5,
    )
    .unwrap();

    // At x = 200m, active pair is (0, 1) -> Pole 1 (x=0) and Pole 2 (x=1000)
    assert_eq!(mgr.active_pair, (0, 1));

    // Advance train by 9.0 seconds at 100 m/s: position becomes 200 + 900 = 1100m
    mgr.step_time(9.0);
    assert!((mgr.train.position_x_m - 1100.0).abs() < 1e-4);

    // Now train is past Pole 2 (x=1000m) and before Pole 3 (x=2000m).
    // Active pair must automatically switch to (1, 2)!
    assert_eq!(mgr.active_pair, (1, 2));

    // Pre-compensation values should be non-zero and active on the new TRP pair
    assert!(mgr.trps[1].pre_compensation_hz.abs() > 100.0);
    assert!(mgr.trps[2].pre_compensation_hz.abs() > 100.0);
}

#[test]
fn test_error_handling_and_parameter_validation() {
    let train = TrainKinematics::new("Test", 0.0, 300.0, 3.0);

    // Invalid carrier frequency (0 Hz)
    let err_fc = HstSfnManager::new(
        HstScenario::OpenSpace,
        0.0,
        30,
        train.clone(),
        1000.0,
        15.0,
        20.0,
        3,
    );
    assert!(matches!(err_fc, Err(HstError::InvalidCarrierFrequency(_))));

    // Invalid subcarrier spacing (45 kHz)
    let err_scs = HstSfnManager::new(
        HstScenario::OpenSpace,
        3.5e9,
        45,
        train.clone(),
        1000.0,
        15.0,
        20.0,
        3,
    );
    assert!(matches!(
        err_scs,
        Err(HstError::InvalidSubcarrierSpacing(45))
    ));

    // Insufficient poles (< 2)
    let err_poles = HstSfnManager::new(
        HstScenario::OpenSpace,
        3.5e9,
        30,
        train,
        1000.0,
        15.0,
        20.0,
        1,
    );
    assert!(matches!(
        err_poles,
        Err(HstError::InsufficientTrps {
            count: 1,
            min_required: 2
        })
    ));
}
