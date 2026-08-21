use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_smet::{
    EvpnSmetEngine, EvpnSmetRoute, EVPN_ROUTE_TYPE_JOIN_SYNCH, EVPN_ROUTE_TYPE_SMET,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_smet_constants_and_roundtrip() {
    assert_eq!(EVPN_ROUTE_TYPE_SMET, 6);
    assert_eq!(EVPN_ROUTE_TYPE_JOIN_SYNCH, 7);

    let rd = RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 10);
    let smet = EvpnSmetRoute::new_any_source(
        rd,
        200,
        Ipv4Address::new(239, 255, 1, 1),
        Ipv4Address::new(10, 0, 0, 1),
    );

    let raw = smet.serialize_nlri();
    let parsed = EvpnSmetRoute::parse_nlri(&raw).unwrap();
    assert_eq!(parsed, smet);
}

#[test]
fn test_evpn_smet_group_withdrawal() {
    let mut engine = EvpnSmetEngine::new();
    let rd = RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 1), 1);

    let group = Ipv4Address::new(239, 10, 10, 10);
    let pe1 = Ipv4Address::new(192, 168, 1, 1);
    let pe2 = Ipv4Address::new(192, 168, 1, 2);

    engine.add_smet_route(EvpnSmetRoute::new_any_source(rd.clone(), 50, group, pe1));
    engine.add_smet_route(EvpnSmetRoute::new_any_source(rd.clone(), 50, group, pe2));

    let pes_before = engine.resolve_replication_pes(50, Ipv4Address::UNSPECIFIED, group);
    assert_eq!(pes_before.len(), 2);

    // Withdraw PE1
    assert!(engine.withdraw_smet_route(50, group, pe1));
    let pes_after = engine.resolve_replication_pes(50, Ipv4Address::UNSPECIFIED, group);
    assert_eq!(pes_after.len(), 1);
    assert_eq!(pes_after[0], pe2);
}
