use toy_tcpip::evpn_ssm_snooping::{EvpnSsmEngine, SmetRouteAction};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_ssm_snooping_lifecycle() {
    let mut engine = EvpnSsmEngine::new(Ipv4Address::new(10, 0, 0, 1), 60);

    let group = Ipv4Address::new(232, 5, 5, 5);
    let source = Ipv4Address::new(192, 168, 50, 10);

    // 1. Port 10 joins (S, G) channel -> triggers SMET advertisement
    let a1 = engine.handle_local_join(300, 10, group, source, 100);
    assert_eq!(
        a1,
        Some(SmetRouteAction::AdvertiseSmet {
            vni: 300,
            group_ip: group,
            source_ip: source,
        })
    );

    // 2. Remote Leaf 10.0.0.3 advertises SMET for same (S, G)
    engine.handle_remote_smet_add(300, Ipv4Address::new(10, 0, 0, 3), group, source);

    // 3. Evaluate forwarding: local port 10 and remote VTEP 10.0.0.3
    let fwd = engine.evaluate_forwarding(300, source, group);
    assert!(!fwd.should_drop);
    assert_eq!(fwd.local_ports, vec![10]);
    assert_eq!(fwd.remote_vteps, vec![Ipv4Address::new(10, 0, 0, 3)]);

    // 4. Non-subscribed source/group -> dropped
    let fwd_unsub = engine.evaluate_forwarding(300, Ipv4Address::new(1, 1, 1, 1), group);
    assert!(fwd_unsub.should_drop);

    // 5. Port 10 leaves -> triggers SMET withdrawal
    let a_leave = engine.handle_local_leave(300, 10, group, source);
    assert_eq!(
        a_leave,
        Some(SmetRouteAction::WithdrawSmet {
            vni: 300,
            group_ip: group,
            source_ip: source,
        })
    );
}
