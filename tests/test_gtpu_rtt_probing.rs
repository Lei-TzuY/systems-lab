use toy_tcpip::gtpu_rtt_probing::{GtpuRttProbingEngine, ProbeAccessLeg};

#[test]
fn test_gtpu_rtt_active_probing_steering() {
    let mut engine = GtpuRttProbingEngine::new(42);

    let probe = engine.create_probe(ProbeAccessLeg::ThreeGpp, 1000);
    assert_eq!(probe.probe_id, 1);
    assert_eq!(probe.leg, ProbeAccessLeg::ThreeGpp);

    let new_srtt = engine.handle_probe_reply(&probe, 4000); // 3000us RTT
    assert!(new_srtt < 20_000.0);
    assert_eq!(engine.total_probes_sent, 1);
    assert_eq!(engine.total_probes_received, 1);
}
