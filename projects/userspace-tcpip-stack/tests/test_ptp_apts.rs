//! Integration tests for ITU-T G.8275.2 Assisted Partial Timing Support (APTS) Engine.

use toy_tcpip::ptp_apts::{AptsConfig, AptsEngine, AptsState};
use toy_tcpip::ptp_pdv_filter::{PtpPdvFloorFilter, PtpTimestampSample};

#[test]
fn test_apts_dynamic_asymmetry_learning_under_gnss_lock() {
    let pdv_filter = PtpPdvFloorFilter::new(20, 10.0, 100);
    let mut config = AptsConfig::default();
    config.asymmetry_learning_alpha = 0.5; // Fast convergence for test

    let mut apts = AptsEngine::new(config, pdv_filter);
    assert_eq!(apts.state, AptsState::GnssLocked);
    assert_eq!(apts.current_clock_class(), 6); // PRTC locked

    // True physical delay: forward = 20,600 ns, reverse = 19,400 ns
    // True path asymmetry: 20,600 - 19,400 = +1,200 ns
    // Local clock is locked to GNSS (offset = 0)
    for seq in 0..20 {
        let t1 = (seq as i64) * 1_000_000;
        let t2 = t1 + 20_600;
        let t3 = t2 + 10_000;
        let t4 = t3 + 19_400;
        apts.push_ptp_sample(PtpTimestampSample::new(seq, t1, t2, t3, t4));
    }

    // Update with valid GNSS (0ns offset)
    apts.update_gnss(true, 0);

    // Learned asymmetry must converge towards +1,200 ns
    let metrics = apts.metrics();
    assert_eq!(metrics.state, AptsState::GnssLocked);
    assert_eq!(metrics.calibrated_asymmetry_ns.round() as i64, 1_200);
    assert_eq!(apts.compute_phase_offset(), Some(0));
}

#[test]
fn test_apts_gnss_loss_seamless_failover_to_ptp() {
    let pdv_filter = PtpPdvFloorFilter::new(20, 10.0, 100);
    let mut config = AptsConfig::default();
    config.asymmetry_learning_alpha = 1.0; // Immediate calibration

    let mut apts = AptsEngine::new(config, pdv_filter);

    // Feed PTP stream with +600ns physical network asymmetry:
    // forward = 15,300 ns, reverse = 14,700 ns (diff = +600 ns)
    for seq in 0..20 {
        let t1 = (seq as i64) * 1_000_000;
        let t2 = t1 + 15_300;
        let t3 = t2 + 20_000;
        let t4 = t3 + 14_700;
        apts.push_ptp_sample(PtpTimestampSample::new(seq, t1, t2, t3, t4));
    }

    // Calibrate with GNSS locked
    apts.update_gnss(true, 0);
    assert_eq!(apts.calibrated_asymmetry_ns.round() as i64, 600);

    // GNSS outage occurs (e.g. antenna fault or jamming)
    apts.update_gnss(false, 0);

    // Engine must seamlessly switch to PTP APTS mode
    assert_eq!(apts.state, AptsState::PtpLockedApts);
    assert_eq!(apts.current_clock_class(), 7); // In-spec APTS

    // Offset computed from PTP with calibrated asymmetry compensation must equal 0 ns!
    let ptp_phase_offset = apts.compute_phase_offset().expect("PTP phase offset");
    assert_eq!(ptp_phase_offset, 0);
}

#[test]
fn test_apts_holdover_fallback_and_aging() {
    let pdv_filter = PtpPdvFloorFilter::new(10, 10.0, 100);
    let mut config = AptsConfig::default();
    config.min_floor_packet_rate_percent = 50.0; // High requirement
    config.floor_width_ns = 50;
    config.max_holdover_within_spec_secs = 100;
    config.oscillator_drift_ppb = 5.0; // 5 ns/s

    let mut apts = AptsEngine::new(config, pdv_filter);

    // Initial GNSS locked
    apts.update_gnss(true, 0);

    // PTP samples have heavy jitter so floor packet rate is only 10% (< 50% threshold)
    for seq in 0..10 {
        let queuing = (seq as i64) * 1_000;
        let t1 = (seq as i64) * 1_000_000;
        let t2 = t1 + 10_000 + queuing;
        let t3 = t2 + 10_000;
        let t4 = t3 + 10_000 + queuing;
        apts.push_ptp_sample(PtpTimestampSample::new(seq, t1, t2, t3, t4));
    }

    // GNSS lost
    apts.update_gnss(false, 0);

    // Since PTP floor packet rate is inadequate, must directly fall back into Holdover
    assert_eq!(apts.state, AptsState::Holdover);
    assert_eq!(apts.current_clock_class(), 7); // Holdover in spec (< 100s)

    // Tick holdover by 50 seconds: drift = 50s * 5ns/s = 250 ns
    apts.tick_holdover(50);
    let m1 = apts.metrics();
    assert_eq!(m1.holdover_duration_secs, 50);
    assert_eq!(m1.accumulated_holdover_drift_ns, 250);
    assert_eq!(apts.current_clock_class(), 7);

    // Tick past 100s max holdover budget: clock class degrades to 140 (out of spec)
    apts.tick_holdover(60);
    let m2 = apts.metrics();
    assert_eq!(m2.holdover_duration_secs, 110);
    assert_eq!(m2.accumulated_holdover_drift_ns, 550);
    assert_eq!(apts.current_clock_class(), 140);
}

#[test]
fn test_apts_gnss_qualification_and_restoration() {
    let pdv_filter = PtpPdvFloorFilter::new(10, 10.0, 100);
    let mut config = AptsConfig::default();
    config.gnss_qualification_count = 3;

    let mut apts = AptsEngine::new(config, pdv_filter);
    apts.update_gnss(true, 0);

    // GNSS lost
    apts.update_gnss(false, 0);
    assert_ne!(apts.state, AptsState::GnssLocked);

    // GNSS returns: 1st valid sample enters GnssQualifying
    apts.update_gnss(true, 5);
    assert_eq!(apts.state, AptsState::GnssQualifying);

    // 2nd valid sample: still qualifying
    apts.update_gnss(true, 3);
    assert_eq!(apts.state, AptsState::GnssQualifying);

    // 3rd valid sample meets qualification count (3): declared GnssLocked!
    apts.update_gnss(true, 0);
    assert_eq!(apts.state, AptsState::GnssLocked);
    assert_eq!(apts.current_clock_class(), 6);
    assert_eq!(apts.gnss_restore_events, 1);
}
