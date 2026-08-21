use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_type3::{
    EvpnBumFloodingTree, EvpnType3Route, PmsiTunnelAttribute, EVPN_ROUTE_TYPE_IMET,
    PMSI_TUNNEL_TYPE_INGRESS_REPLICATION,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_type3_constants_and_codec() {
    assert_eq!(EVPN_ROUTE_TYPE_IMET, 3);
    assert_eq!(PMSI_TUNNEL_TYPE_INGRESS_REPLICATION, 6);

    let pmsi = PmsiTunnelAttribute::new_ingress_replication(30001, Ipv4Address::new(10, 1, 1, 1));
    let pmsi_bytes = pmsi.serialize();
    let pmsi_parsed = PmsiTunnelAttribute::parse(&pmsi_bytes).unwrap();
    assert_eq!(pmsi_parsed, pmsi);

    let rd = RouteDistinguisher::new(Ipv4Address::new(10, 1, 1, 1), 200);
    let route = EvpnType3Route::new_ipv4(
        rd,
        0,
        Ipv4Address::new(10, 1, 1, 1),
        30001,
    );

    let bytes = route.serialize();
    let parsed = EvpnType3Route::parse(&bytes).unwrap();
    assert_eq!(parsed, route);
}

#[test]
fn test_evpn_type3_bum_replication_endpoints() {
    let mut bum_tree = EvpnBumFloodingTree::new();
    let pe1 = Ipv4Address::new(192, 0, 2, 1);
    let pe2 = Ipv4Address::new(192, 0, 2, 2);
    let pe3 = Ipv4Address::new(192, 0, 2, 3);

    bum_tree.add_route(EvpnType3Route::new_ipv4(
        RouteDistinguisher::new(pe1, 1),
        0,
        pe1,
        50001,
    ));
    bum_tree.add_route(EvpnType3Route::new_ipv4(
        RouteDistinguisher::new(pe2, 1),
        0,
        pe2,
        50001,
    ));
    bum_tree.add_route(EvpnType3Route::new_ipv4(
        RouteDistinguisher::new(pe3, 1),
        0,
        pe3,
        50001,
    ));

    // Exclude PE1 from flood list (split-horizon)
    let flood_targets = bum_tree.get_flood_endpoints(50001, pe1);
    assert_eq!(flood_targets.len(), 2);
    assert!(flood_targets.contains(&pe2));
    assert!(flood_targets.contains(&pe3));
    assert!(!flood_targets.contains(&pe1));
}
