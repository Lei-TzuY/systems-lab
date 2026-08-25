use toy_tcpip::evpn_core_isolation::{CoreIsolationState, EvpnCoreIsolationEngine};

#[test]
fn test_evpn_core_isolation_and_split_horizon_filtering() {
    let mut leaf = EvpnCoreIsolationEngine::new(10);

    leaf.add_core_uplink("spine_primary");
    leaf.register_client_ac("client_port_1", Some(0x12345678));
    assert_eq!(leaf.state, CoreIsolationState::Normal);

    // ESI mismatch -> Allowed
    assert!(leaf.should_forward_to_ac("client_port_1", Some(0x87654321)));

    // ESI match -> Split-Horizon drop
    assert!(!leaf.should_forward_to_ac("client_port_1", Some(0x12345678)));

    // Core uplink down -> Core Isolation
    leaf.remove_core_uplink("spine_primary");
    assert_eq!(leaf.state, CoreIsolationState::CoreIsolated);
    assert!(!leaf.should_forward_to_ac("client_port_1", None));
}
