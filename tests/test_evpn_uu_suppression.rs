use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_uu_suppression::{EvpnUuSuppressionEngine, UuSuppressionDecision};

#[test]
fn test_evpn_unknown_unicast_flood_suppression() {
    let mut engine = EvpnUuSuppressionEngine::new();
    let vni = 200;
    let known_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let unknown_mac = MacAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

    engine.set_vni_suppression(vni, true);
    engine.add_known_mac(vni, known_mac);

    // 1. Known MAC is forwarded
    let dec1 = engine.evaluate_frame(vni, known_mac);
    assert_eq!(dec1, UuSuppressionDecision::ForwardKnownUnicast);

    // 2. Unknown MAC is suppressed when policy is active
    let dec2 = engine.evaluate_frame(vni, unknown_mac);
    assert_eq!(dec2, UuSuppressionDecision::SuppressedUnknownUnicast);

    // 3. Disable suppression on VNI: unknown MAC is allowed to flood
    engine.set_vni_suppression(vni, false);
    let dec3 = engine.evaluate_frame(vni, unknown_mac);
    assert_eq!(dec3, UuSuppressionDecision::ForwardFloodingAllowed);

    assert_eq!(engine.allowed_packets_count, 2);
    assert_eq!(engine.suppressed_packets_count, 1);
}
