use toy_tcpip::ptp_time_error::{PtpTimeErrorEngine, TelecomClockClass};

#[test]
fn test_ptp_time_error_cte_and_peak_to_peak() {
    let mut engine = PtpTimeErrorEngine::new(5);

    // Add 5 samples: 10, 20, 15, 25, 30 ns
    engine.add_sample(10);
    engine.add_sample(20);
    engine.add_sample(15);
    engine.add_sample(25);
    engine.add_sample(30);

    let cte = engine.calculate_cte();
    assert_eq!(cte, 20.0);

    let p2p = engine.calculate_peak_to_peak_te();
    assert_eq!(p2p, 20); // 30 - 10 = 20 ns

    // Meets Class C (|cTE| <= 30ns, |TE| <= 55ns)
    assert!(engine.verify_compliance(TelecomClockClass::ClassC));
    // Fails Class D (|cTE| <= 15ns)
    assert!(!engine.verify_compliance(TelecomClockClass::ClassD));

    // Ring buffer overflow test
    engine.add_sample(5); // Window becomes [20, 15, 25, 30, 5]
    assert_eq!(engine.samples.len(), 5);
    assert_eq!(engine.calculate_cte(), 19.0);
}

#[test]
fn test_ptp_mtie_calculation_and_curve() {
    let mut engine = PtpTimeErrorEngine::new(20);

    // 10 samples with controlled fluctuations
    let data = [10, 15, 12, 20, 18, 25, 22, 14, 16, 19];
    for &val in &data {
        engine.add_sample(val);
    }

    // Observation interval tau=1: max - min in 1-sample window is 0
    assert_eq!(engine.calculate_mtie(1), Some(0.0));

    // Observation interval tau=2: max adjacent delta is 8 (12 to 20, or 22 to 14)
    assert_eq!(engine.calculate_mtie(2), Some(8.0));

    // Observation interval tau=5: max span within 5 contiguous samples is 13 (25 - 12)
    assert_eq!(engine.calculate_mtie(5), Some(13.0));

    // Observation interval tau=10: span across all samples: 25 - 10 = 15
    assert_eq!(engine.calculate_mtie(10), Some(15.0));

    // Exceeding sample count returns None
    assert_eq!(engine.calculate_mtie(11), None);

    // Compute full MTIE curve
    let curve = engine.compute_mtie_curve(&[1, 2, 5, 10]);
    assert_eq!(curve.len(), 4);
    assert_eq!(curve[0].mtie_ns, 0.0);
    assert_eq!(curve[1].mtie_ns, 8.0);
    assert_eq!(curve[2].mtie_ns, 13.0);
    assert_eq!(curve[3].mtie_ns, 15.0);
}

#[test]
fn test_ptp_tdev_stability_and_drift() {
    let mut stable_engine = PtpTimeErrorEngine::new(20);
    // Constant time error samples: zero second-difference
    for _ in 0..12 {
        stable_engine.add_sample(25);
    }
    let tdev_stable = stable_engine.calculate_tdev(1).unwrap();
    assert!((tdev_stable - 0.0).abs() < 1e-6);

    // Linear drift: constant frequency offset has zero second-difference
    let mut drift_engine = PtpTimeErrorEngine::new(20);
    for i in 1..=12 {
        drift_engine.add_sample(i * 5); // 5, 10, 15, 20, ...
    }
    let tdev_drift = drift_engine.calculate_tdev(1).unwrap();
    assert!((tdev_drift - 0.0).abs() < 1e-6);

    // Jittery samples produce positive TDEV
    let mut noisy_engine = PtpTimeErrorEngine::new(20);
    let noisy_data = [5, 15, 8, 22, 11, 28, 14, 25, 9, 18, 12, 20];
    for &val in &noisy_data {
        noisy_engine.add_sample(val);
    }
    let tdev_noisy = noisy_engine.calculate_tdev(2).unwrap();
    assert!(tdev_noisy > 0.0);

    let tdev_curve = noisy_engine.compute_tdev_curve(&[1, 2, 3]);
    assert_eq!(tdev_curve.len(), 3);
    for pt in &tdev_curve {
        assert!(pt.tdev_ns > 0.0);
    }
}

#[test]
fn test_ptp_telecom_sync_mask_compliance() {
    use toy_tcpip::ptp_time_error::TelecomSyncMask;

    let mut engine = PtpTimeErrorEngine::new(20);
    // Fronthaul eCPRI T-BC Class C: MTIE <= 22 ns for tau <= 100s
    let compliant_samples = [10, 14, 12, 18, 16, 21, 15, 19, 13, 17];
    for &s in &compliant_samples {
        engine.add_sample(s);
    }

    // Sampling period 0.1s (10 Hz sync rate)
    let mask_c = TelecomSyncMask::G8273_2ClassC;
    assert!(engine.verify_mtie_mask(&mask_c, &[1, 2, 5, 8], 0.1));

    let mask_d = TelecomSyncMask::G8273_2ClassD; // 10 ns limit
    // Max span is 21 - 10 = 11 ns (> 10 ns), should fail Class D
    assert!(!engine.verify_mtie_mask(&mask_d, &[1, 2, 5, 8], 0.1));

    // Custom piecewise mask
    let custom_mask = TelecomSyncMask::PiecewiseTwoStage {
        threshold_sec: 1.0,
        limit_below_ns: 12.0,
        limit_above_ns: 25.0,
    };
    assert!(engine.verify_mtie_mask(&custom_mask, &[1, 2, 5], 0.1));
}
