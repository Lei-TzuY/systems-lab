use toy_tcpip::congestion::{CongestionControl, CongestionState, RttEstimator};

#[test]
fn test_congestion_window_slow_start_and_avoidance() {
    let mss = 1460;
    let mut cc = CongestionControl::new(mss);
    cc.ssthresh = 4380; // 3 * MSS

    assert_eq!(cc.cwnd, 2920); // 2 * MSS
    assert_eq!(cc.state, CongestionState::SlowStart);

    // 1st ACK
    cc.on_ack(1460);
    assert_eq!(cc.cwnd, 4380);
    assert_eq!(cc.state, CongestionState::CongestionAvoidance);

    // 2nd ACK in Congestion Avoidance (linear growth)
    let prev_cwnd = cc.cwnd;
    cc.on_ack(1460);
    assert!(cc.cwnd > prev_cwnd);
    assert_eq!(cc.state, CongestionState::CongestionAvoidance);
}

#[test]
fn test_fast_retransmit_and_timeout() {
    let mss = 1460;
    let mut cc = CongestionControl::new(mss);
    cc.cwnd = 14600; // 10 * MSS

    // 1st and 2nd dup ACK do not trigger fast retransmit
    assert!(!cc.on_dup_ack());
    assert!(!cc.on_dup_ack());
    assert_eq!(cc.state, CongestionState::SlowStart);

    // 3rd dup ACK triggers Fast Retransmit
    assert!(cc.on_dup_ack());
    assert_eq!(cc.state, CongestionState::FastRecovery);
    assert_eq!(cc.ssthresh, 7300); // cwnd / 2

    // Timeout collapses cwnd back to 1 MSS
    cc.on_timeout();
    assert_eq!(cc.state, CongestionState::SlowStart);
    assert_eq!(cc.cwnd, 1460);
}

#[test]
fn test_rtt_estimator_convergence() {
    let mut rtt = RttEstimator::new();
    assert_eq!(rtt.rto, 1000.0);

    // Feed steady 50ms samples
    for _ in 0..10 {
        rtt.update_sample(50.0);
    }

    assert!(rtt.srtt.unwrap() >= 49.0 && rtt.srtt.unwrap() <= 51.0);
    assert!(rtt.rto >= 200.0 && rtt.rto <= 300.0); // Clamped at min_rto or low variance
}
