use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_bum_policer::{BumPolicerVerdict, BumType, EvpnBumPolicerEngine};

#[test]
fn test_evpn_bum_policer_multicast_burst_and_quarantine() {
    let mut engine = EvpnBumPolicerEngine::new(2); // 2 drops -> quarantine

    // Set Multicast limit on VNI 200: 2000 B/s, 2000 B burst
    engine.set_rate_limit(200, BumType::Multicast, 2_000, 2_000);

    let rogue_mac = MacAddress([0x52, 0x54, 0x00, 0x99, 0x88, 0x77]);

    // 1500B -> Pass
    assert_eq!(
        engine.police_frame(200, rogue_mac, BumType::Multicast, 1500, 0),
        BumPolicerVerdict::Pass
    );

    // 1000B -> Drop #1
    assert_eq!(
        engine.police_frame(200, rogue_mac, BumType::Multicast, 1000, 0),
        BumPolicerVerdict::RateLimitedDrop
    );

    // 1000B -> Drop #2 -> Storm Quarantined!
    assert_eq!(
        engine.police_frame(200, rogue_mac, BumType::Multicast, 1000, 0),
        BumPolicerVerdict::StormQuarantined
    );

    // Verify unquarantine
    assert!(engine.unquarantine_mac(200, &rogue_mac));
    assert_eq!(engine.quarantined_macs.len(), 0);
}
