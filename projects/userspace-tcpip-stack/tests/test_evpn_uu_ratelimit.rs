use toy_tcpip::evpn_uu_ratelimit::{EvpnUuRateLimitEngine, UuRateLimitVerdict};

#[test]
fn test_evpn_uu_storm_policer_lifecycle() {
    let mut engine = EvpnUuRateLimitEngine::new();
    engine.configure_vni_limit(200, 5000, 1500);

    // Initial burst 1000B -> Pass
    assert_eq!(
        engine.police_unknown_unicast(200, 1000, 10_000),
        UuRateLimitVerdict::Pass
    );

    // Immediate 800B -> Exceeds remaining 500B -> Drop
    assert_eq!(
        engine.police_unknown_unicast(200, 800, 10_000),
        UuRateLimitVerdict::DropExceeded
    );

    assert_eq!(engine.total_evaluated_frames, 2);
    assert_eq!(engine.total_passed_frames, 1);
    assert_eq!(engine.total_rate_limited_drops, 1);
}
