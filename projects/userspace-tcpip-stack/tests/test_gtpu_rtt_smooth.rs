use toy_tcpip::gtpu_rtt_smooth::{GtpuRttSmoothEngine, RttAnomalyVerdict};

#[test]
fn test_gtpu_rtt_smooth_lifecycle() {
    // Session 0x88001122, Fast alpha = 1/4 (0.25), Slow alpha = 1/16 (0.0625), Spike Threshold = 140%
    let mut engine = GtpuRttSmoothEngine::new(0x88001122, (1, 4), (1, 16), 140);

    // 1. Initial sample
    let v0 = engine.feed_sample(10_000); // 10 ms (10,000 µs)
    assert_eq!(v0, RttAnomalyVerdict::Normal);
    assert_eq!(engine.fast_ema_us(), 10_000);
    assert_eq!(engine.slow_ema_us(), 10_000);

    // 2. Feed several consistent baseline samples
    for _ in 0..15 {
        let v = engine.feed_sample(10_000);
        assert_eq!(v, RttAnomalyVerdict::Normal);
    }
    assert_eq!(engine.fast_ema_us(), 10_000);
    assert_eq!(engine.slow_ema_us(), 10_000);

    // 3. Inject sudden transport latency spike: 50 ms (50,000 µs)
    let v_spike = engine.feed_sample(50_000);
    match v_spike {
        RttAnomalyVerdict::LatencySpike {
            fast_ema_us,
            slow_ema_us,
            ratio_percent,
        } => {
            assert!(fast_ema_us > slow_ema_us);
            assert!(ratio_percent >= 140);
        }
        _ => panic!("Expected LatencySpike anomaly"),
    }
    assert_eq!(engine.total_spikes_detected, 1);
}
