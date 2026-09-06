use toy_tcpip::cbs::CreditBasedShaper;

#[test]
fn test_cbs_credit_limits_and_reset() {
    let mut cbs = CreditBasedShaper::new("AVB-Class-B", 50_000_000, 1_000_000_000, 1500);
    assert_eq!(cbs.idle_slope_bps, 50_000_000);
    assert_eq!(cbs.send_slope_bps, -950_000_000);

    // If empty queue and credit accumulated somehow, reset to 0
    cbs.advance_time(100);
    assert_eq!(cbs.current_credit_bits, 0);

    // Queue has frames, wait 200µs -> credit = 50 Mbps * 200µs = 10,000 bits
    cbs.has_queued_frames = true;
    cbs.advance_time(300);
    assert!(cbs.current_credit_bits > 0);
    assert!(cbs.can_transmit());

    // Queue emptied when credit is positive -> immediately resets to 0
    cbs.has_queued_frames = false;
    cbs.advance_time(350);
    assert_eq!(cbs.current_credit_bits, 0);
}

#[test]
fn test_cbs_transmission_burst_depletion() {
    let mut cbs = CreditBasedShaper::new("AVB-Class-A", 150_000_000, 1_000_000_000, 1500);
    cbs.has_queued_frames = true;
    cbs.advance_time(100); // 150 * 100 = 15,000 bits
    assert!(cbs.can_transmit());

    cbs.start_transmitting(100);
    // Transmit for 100µs at sendSlope (-850 Mbps)
    // delta = -850 * 100 = -85,000 bits -> credit = 15,000 - 85,000 = -70,000 bits
    cbs.finish_transmitting(200, true);
    assert!(cbs.current_credit_bits < 0);
    assert!(!cbs.can_transmit());

    // Wait until credit recovers back to >= 0
    // Needed recovery = 70,000 / 150 = ~467µs
    cbs.advance_time(700); // 500µs elapsed * 150 Mbps = 75,000 bits -> credit >= 0
    assert!(cbs.current_credit_bits >= 0);
    assert!(cbs.can_transmit());
}
