//! Integration Tests for 3GPP Release 18 e-RedCap & Low-Power Wake-Up Signal (LP-WUS) Engine.

use toy_tcpip::nr_eredcap_wus::*;

#[test]
fn test_eredcap_bandwidth_and_prb_scaling() {
    let bw5 = ERedCapBandwidth::Bw5Mhz;
    let bw10 = ERedCapBandwidth::Bw10Mhz;

    // PRB allocation for 15 kHz and 30 kHz SCS (TS 38.101-1 Table 5.3.2-1)
    assert_eq!(bw5.allocated_prbs(15), 25);
    assert_eq!(bw5.allocated_prbs(30), 12);
    assert_eq!(bw10.allocated_prbs(15), 52);
    assert_eq!(bw10.allocated_prbs(30), 24);

    // Peak DL bitrate: 5 MHz yields ~10-12 Mbps, 10 MHz yields ~21-25 Mbps
    let peak5_1r = bw5.theoretical_peak_dl_bps(30, 1);
    let peak5_2r = bw5.theoretical_peak_dl_bps(30, 2);
    let peak10_1r = bw10.theoretical_peak_dl_bps(30, 1);

    assert!(
        peak5_1r >= 10_000_000 && peak5_1r <= 20_000_000,
        "5 MHz 1R peak rate should be ~16 Mbps, got {peak5_1r} bps"
    );
    assert!(
        peak5_2r > peak5_1r,
        "Dual RX diversity increases throughput"
    );
    assert!(
        peak10_1r > peak5_1r * 18 / 10,
        "10 MHz bandwidth scales proportionally with PRBs"
    );
}

#[test]
fn test_lp_wus_sequence_generation_ook_and_fsk() {
    let pg_id = 42;
    let cell_id = 101;
    let seq_len = 63;
    let advance_ms = 20; // 20 ms advance before PO

    // 1. OOK sequence (chips in {0.0, 1.0})
    let wus_ook = LpWusSequence::generate(
        pg_id,
        cell_id,
        LpWusModulation::OnOfKeying,
        seq_len,
        advance_ms,
    )
    .expect("Generate OOK LP-WUS");
    assert_eq!(wus_ook.chip_values.len(), seq_len);
    for &chip in &wus_ook.chip_values {
        assert!(chip == 0.0 || chip == 1.0, "OOK chip must be 0.0 or 1.0");
    }

    // 2. 2-FSK sequence (chips in {-1.0, 1.0})
    let wus_fsk = LpWusSequence::generate(
        pg_id,
        cell_id,
        LpWusModulation::FrequencyShiftKeying,
        seq_len,
        advance_ms,
    )
    .expect("Generate FSK LP-WUS");
    assert_eq!(wus_fsk.chip_values.len(), seq_len);
    for &chip in &wus_fsk.chip_values {
        assert!(chip == -1.0 || chip == 1.0, "FSK chip must be -1.0 or 1.0");
    }

    // Different Paging Group IDs generate distinct sequences
    let wus_other = LpWusSequence::generate(
        99,
        cell_id,
        LpWusModulation::OnOfKeying,
        seq_len,
        advance_ms,
    )
    .expect("Generate other OOK LP-WUS");
    assert_ne!(wus_ook.chip_values, wus_other.chip_values);
}

#[test]
fn test_lp_wur_correlation_detector_detection_and_false_alarm() {
    let target_seq = LpWusSequence::generate(15, 200, LpWusModulation::OnOfKeying, 63, 20).unwrap();
    let mut detector = LpWurDetector::new(target_seq.clone(), 0.60);

    // Case 1: Clean matched signal received -> High correlation -> Wake up!
    let clean_signal = target_seq.chip_values.clone();
    let dec1 = detector.detect(&clean_signal, 0.05);
    match dec1 {
        LpWurDecision::WakeUpMainBaseband {
            paging_group_id,
            correlation_score,
            ..
        } => {
            assert_eq!(paging_group_id, 15);
            assert!(
                correlation_score > 0.95,
                "Clean signal correlation should be > 0.95"
            );
        }
        LpWurDecision::RemainAsleep { .. } => panic!("Detector should have triggered wake up!"),
    }

    // Case 2: Noisy matched signal (additive Gaussian noise sigma = 0.2) -> Still detects!
    let mut noisy_signal = clean_signal.clone();
    for (i, val) in noisy_signal.iter_mut().enumerate() {
        let pseudo_noise = (((i * 17 + 3) % 11) as f64 - 5.0) * 0.04;
        *val += pseudo_noise;
    }
    let dec2 = detector.detect(&noisy_signal, 0.2);
    assert!(matches!(dec2, LpWurDecision::WakeUpMainBaseband { .. }));

    // Case 3: Uncorrelated sequence (different PG_ID = 88) -> False alarm rejection -> Remain asleep!
    let other_seq = LpWusSequence::generate(88, 200, LpWusModulation::OnOfKeying, 63, 20).unwrap();
    let dec3 = detector.detect(&other_seq.chip_values, 0.1);
    match dec3 {
        LpWurDecision::RemainAsleep {
            max_correlation, ..
        } => {
            assert!(
                max_correlation < 0.60,
                "Orthogonal sequence correlation should be low"
            );
        }
        LpWurDecision::WakeUpMainBaseband { .. } => {
            panic!("Detector must NOT wake up on foreign PG-ID")
        }
    }

    // Case 4: Pure random noise -> Remain asleep
    let pure_noise: Vec<f64> = (0..63)
        .map(|i| (((i * 29) % 7) as f64 - 3.0) * 0.1)
        .collect();
    let dec4 = detector.detect(&pure_noise, 0.3);
    assert!(matches!(dec4, LpWurDecision::RemainAsleep { .. }));
}

#[test]
fn test_hypersfn_timing_and_edrx_cycle_evaluation() {
    let mut timing = HyperSfnTiming::new(0, 0, 0).expect("Initial timing");

    // Advance by 15 milliseconds (1.5 subframes)
    timing.advance_ms(15);
    assert_eq!(timing.subframe, 5);
    assert_eq!(timing.sfn, 1);
    assert_eq!(timing.h_sfn, 0);

    // Advance by 10,240 ms (exactly 1024 frames = 1 H-SFN)
    timing.advance_ms(10_240);
    assert_eq!(timing.h_sfn, 1);

    // eDRX Configuration: UE_ID = 0x8000_0000, eDRX cycle = 4 H-SFNs, PTW = 10.24 s
    let edrx = EDrxConfig::new(0x0000_1000, 4, 10.24, 1280).expect("Valid eDRX config");

    let t_match = HyperSfnTiming::new(8, 50, 0).unwrap(); // 8 % 4 == 0 -> Match!
    let t_no_match = HyperSfnTiming::new(9, 50, 0).unwrap(); // 9 % 4 == 1 -> No match

    assert!(edrx.is_paging_hsfn(&t_match));
    assert!(!edrx.is_paging_hsfn(&t_no_match));

    // Inside PTW (PTW length = 10.24 s = 1024 frames)
    let t_in_ptw = HyperSfnTiming::new(8, 500, 0).unwrap(); // 5 s elapsed < 10.24 s
    assert!(edrx.is_inside_ptw(&t_in_ptw));

    // Outside PTW: short PTW of 2.56 s (256 frames)
    let edrx_short = EDrxConfig::new(0x0000_1000, 4, 2.56, 1280).unwrap();
    assert!(!edrx_short.is_inside_ptw(&t_in_ptw)); // 5 s > 2.56 s -> Outside PTW!
}

#[test]
fn test_stationary_relaxed_rrm_measurement_evaluation() {
    let mut evaluator = RelaxedRrmEvaluator::new(2.0, -105.0, 16);

    // Initial readings: fewer than 5 samples -> not active yet
    for _ in 0..4 {
        evaluator.record_rsrp_sample(-85.0);
    }
    assert!(!evaluator.is_relaxation_active);

    // Feed steady stationary RSRP values around -85.0 dBm with minor noise (< 1.5 dB variance)
    let steady_samples = [-84.8, -85.2, -84.9, -85.1, -85.0, -84.7, -85.3];
    for &sample in &steady_samples {
        evaluator.record_rsrp_sample(sample);
    }

    assert!(
        evaluator.is_stationary,
        "Variance < 2.0 dB should qualify as stationary"
    );
    assert!(
        evaluator.is_relaxation_active,
        "Relaxed RRM should be activated"
    );

    // Verify measurement period scaling: baseline 5 seconds -> relaxed to 80 seconds (16x)
    let eff_period = evaluator.effective_measurement_period_s(5.0);
    assert_eq!(eff_period, 80.0);

    // Simulate device movement: sharp signal drop from -85 dBm to -98 dBm (variance = 13 dB > 2 dB)
    evaluator.record_rsrp_sample(-98.0);
    assert!(
        !evaluator.is_stationary,
        "Variance > 2.0 dB must cancel stationary state"
    );
    assert!(!evaluator.is_relaxation_active);

    // Measurement period immediately returns to baseline 5.0 s for quick handover search
    let fast_period = evaluator.effective_measurement_period_s(5.0);
    assert_eq!(fast_period, 5.0);
}

#[test]
fn test_small_data_transmission_in_rrc_inactive() {
    let target_wus = LpWusSequence::generate(1, 1, LpWusModulation::OnOfKeying, 31, 10).unwrap();
    let edrx = EDrxConfig::new(100, 2, 5.12, 1280).unwrap();
    let mut engine = ERedCapEngine::new(
        1001,
        ERedCapBandwidth::Bw5Mhz,
        AntennaConfiguration::OneTxOneRx,
        target_wus,
        edrx,
    );

    // Transmit Configured Grant SDT (e.g. 64-byte smart meter reading)
    let sdt_cg = SdtPacket {
        transaction_id: 1,
        mode: SdtMode::ConfiguredGrant,
        rrc_resume_cause: 0, // mo-Signalling
        payload: vec![0xEE; 64],
    };

    engine.transmit_sdt_packet(sdt_cg).expect("Transmit CG-SDT");
    assert_eq!(engine.sdt_queue.len(), 1);
    assert_eq!(engine.metrics.total_sdt_transmissions, 1);
    assert!(engine.metrics.cumulative_energy_consumed_joules > 0.0);

    // Transmit RACH-based SDT
    let sdt_rach = SdtPacket {
        transaction_id: 2,
        mode: SdtMode::RachBased,
        rrc_resume_cause: 1, // mo-Data
        payload: vec![0xFF; 128],
    };

    engine
        .transmit_sdt_packet(sdt_rach)
        .expect("Transmit RACH-SDT");
    assert_eq!(engine.sdt_queue.len(), 2);
    assert_eq!(engine.metrics.total_sdt_transmissions, 2);
}

#[test]
fn test_eredcap_energy_efficiency_and_power_savings() {
    let target_wus = LpWusSequence::generate(7, 50, LpWusModulation::OnOfKeying, 63, 20).unwrap();
    let edrx = EDrxConfig::new(500, 4, 5.12, 1280).unwrap();
    let mut engine = ERedCapEngine::new(
        2002,
        ERedCapBandwidth::Bw5Mhz,
        AntennaConfiguration::OneTxOneRx,
        target_wus.clone(),
        edrx,
    );

    // Simulate 2 hours (7,200,000 ms) of deep sleep operation with 10 WUS monitoring occasions
    let occasion_interval_ms = 720_000; // once every 12 minutes
    for _ in 0..10 {
        engine.step_time_ms(occasion_interval_ms);

        // 9 occasions have no wake-up signal (noise only) -> remain in sleep
        let noise_signal = vec![0.05; 63];
        let dec = engine.evaluate_wus_occasion(&noise_signal, 0.1);
        assert!(matches!(dec, LpWurDecision::RemainAsleep { .. }));
    }

    // 1 occasion receives valid wake-up signal -> wakes main baseband
    let dec_wake = engine.evaluate_wus_occasion(&target_wus.chip_values, 0.05);
    assert!(matches!(dec_wake, LpWurDecision::WakeUpMainBaseband { .. }));

    // Verify statistics
    assert_eq!(engine.metrics.total_wus_monitored, 11);
    assert_eq!(engine.metrics.total_wus_detections, 1);
    assert!(engine.metrics.total_deep_sleep_hours >= 1.99);

    // Energy comparison:
    // With LP-WUS + eDRX, device avoids waking full 95 mW receiver every 1.28 s
    let savings_pct = engine.energy_savings_percentage();
    assert!(
        savings_pct > 90.0,
        "LP-WUS + eDRX must achieve > 90% energy savings over legacy DRX, got {:.2}%",
        savings_pct
    );
}
