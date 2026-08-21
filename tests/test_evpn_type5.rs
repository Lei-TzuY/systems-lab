use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_type5::{EvpnType5Rib, EvpnType5Route, EVPN_ROUTE_TYPE_IP_PREFIX};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_type5_constants_and_codec() {
    assert_eq!(EVPN_ROUTE_TYPE_IP_PREFIX, 5);

    let rd = RouteDistinguisher::new(Ipv4Address::new(192, 0, 2, 1), 10);
    let route = EvpnType5Route::new_ipv4(
        rd,
        Ipv4Address::new(172, 16, 0, 0),
        16,
        Ipv4Address::new(192, 0, 2, 254),
        10005,
    );

    let serialized = route.serialize();
    let parsed = EvpnType5Route::parse(&serialized).unwrap();
    assert_eq!(parsed, route);
}

#[test]
fn test_evpn_type5_rib_longest_prefix_match() {
    let mut rib = EvpnType5Rib::new();
    let rd = RouteDistinguisher::new(Ipv4Address::new(192, 0, 2, 1), 10);

    rib.add_route(EvpnType5Route::new_ipv4(
        rd.clone(),
        Ipv4Address::new(10, 0, 0, 0),
        8,
        Ipv4Address::new(192, 0, 2, 1),
        10001,
    ));
    rib.add_route(EvpnType5Route::new_ipv4(
        rd.clone(),
        Ipv4Address::new(10, 100, 0, 0),
        16,
        Ipv4Address::new(192, 0, 2, 2),
        10002,
    ));
    rib.add_route(EvpnType5Route::new_ipv4(
        rd,
        Ipv4Address::new(10, 100, 50, 0),
        24,
        Ipv4Address::new(192, 0, 2, 3),
        10003,
    ));

    // Match /24
    let res24 = rib.lookup_lpm(Ipv4Address::new(10, 100, 50, 123)).unwrap();
    assert_eq!(res24.prefix_len, 24);
    assert_eq!(res24.label_or_vni, 10003);

    // Match /16
    let res16 = rib.lookup_lpm(Ipv4Address::new(10, 100, 88, 1)).unwrap();
    assert_eq!(res16.prefix_len, 16);
    assert_eq!(res16.label_or_vni, 10002);

    // Match /8
    let res8 = rib.lookup_lpm(Ipv4Address::new(10, 1, 2, 3)).unwrap();
    assert_eq!(res8.prefix_len, 8);
    assert_eq!(res8.label_or_vni, 10001);

    // No match
    assert!(rib.lookup_lpm(Ipv4Address::new(192, 168, 1, 1)).is_none());
}
