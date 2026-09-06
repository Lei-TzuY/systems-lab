use toy_tcpip::gtpu_jitter_telemetry::GtpuJitterTelemetryEngine;

#[test]
fn test_gtpu_jitter_buffer_metrics() {
    let mut engine = GtpuJitterTelemetryEngine::new(5005);

    engine.record_sample(1, 10_000, 12_000); // delay = 2000 us
    engine.record_sample(2, 20_000, 22_100); // delay = 2100 us (diff = 100)

    assert_eq!(engine.min_delay_us, 2000);
    assert_eq!(engine.max_delay_us, 2100);
    assert_eq!(engine.average_delay_us(), 2050.0);
    assert!(engine.current_jitter_us > 0.0);
}
