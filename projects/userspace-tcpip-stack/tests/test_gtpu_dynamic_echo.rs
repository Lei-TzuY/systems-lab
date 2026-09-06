use toy_tcpip::gtpu_dynamic_echo::{GtpuDynamicEchoEngine, GtpuPathHealth};

#[test]
fn test_gtpu_adaptive_heartbeat_lifecycle() {
    let mut engine = GtpuDynamicEchoEngine::new(3000, 300);

    assert_eq!(engine.state, GtpuPathHealth::Healthy);
    assert_eq!(engine.current_interval_ms(), 3000);

    // Record 1 probe loss -> triggers fast probing
    engine.record_probe_result(false);
    assert_eq!(engine.state, GtpuPathHealth::DegradedFastProbing);
    assert_eq!(engine.current_interval_ms(), 300);

    // Recovery
    engine.record_probe_result(true);
    engine.record_probe_result(true);
    engine.record_probe_result(true);
    assert_eq!(engine.state, GtpuPathHealth::Healthy);
}
