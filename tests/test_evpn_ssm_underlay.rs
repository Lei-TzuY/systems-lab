use toy_tcpip::evpn_ssm_underlay::{
    EvpnUnderlayPmsiEngine, UnderlayEncapsulationPlan, UnderlayTunnelType,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_ssm_underlay_lifecycle() {
    let mut engine = EvpnUnderlayPmsiEngine::new(Ipv4Address::new(10, 0, 0, 1), 2);

    let group = Ipv4Address::new(232, 10, 10, 10);
    let src = Ipv4Address::new(192, 168, 100, 1);

    // 1. Single receiver -> Ingress Replication
    let t1 = engine.add_receiver_vtep(500, group, src, Ipv4Address::new(10, 0, 0, 2));
    assert_eq!(t1, UnderlayTunnelType::IngressReplication);

    let p1 = engine.evaluate_encapsulation(500, src, group);
    assert_eq!(
        p1,
        UnderlayEncapsulationPlan::UnicastReplication {
            destination_vteps: vec![Ipv4Address::new(10, 0, 0, 2)],
        }
    );

    // 2. Second receiver arrives -> Meets threshold 2 -> Promoted to S-PMSI P-Tree!
    let t2 = engine.add_receiver_vtep(500, group, src, Ipv4Address::new(10, 0, 0, 3));
    assert_eq!(t2, UnderlayTunnelType::SelectivePTree);

    let p2 = engine.evaluate_encapsulation(500, src, group);
    match p2 {
        UnderlayEncapsulationPlan::CoreMulticast { underlay_group } => {
            assert_eq!(underlay_group.0[0], 239);
        }
        _ => panic!("Expected CoreMulticast plan"),
    }
}
