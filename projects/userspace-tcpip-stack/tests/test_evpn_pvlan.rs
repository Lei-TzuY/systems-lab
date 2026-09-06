use toy_tcpip::evpn_pvlan::{EvpnPvlanEngine, PvlanPortType};

#[test]
fn test_evpn_pvlan_port_isolation_rules() {
    let mut engine = EvpnPvlanEngine::new(200);

    engine.register_port("core_gw", PvlanPortType::Promiscuous);
    engine.register_port("isolated_host_1", PvlanPortType::Isolated);
    engine.register_port("isolated_host_2", PvlanPortType::Isolated);

    // Isolated -> Gateway allowed
    assert!(engine.can_forward("isolated_host_1", "core_gw"));

    // Isolated -> Isolated blocked
    assert!(!engine.can_forward("isolated_host_1", "isolated_host_2"));
    assert_eq!(engine.total_allowed_frames, 1);
    assert_eq!(engine.total_blocked_frames, 1);
}
