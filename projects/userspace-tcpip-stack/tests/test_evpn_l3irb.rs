use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_l3irb::{
    BGP_EXT_COMMUNITY_ROUTER_MAC, EVPN_ROUTE_TYPE_IP_PREFIX, EvpnIpPrefixRoute, EvpnL3VrfTable,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_l3_irb_symmetric_routing_table() {
    let mut vrf = EvpnL3VrfTable::new(
        "TENANT-BLUE",
        50002,
        MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x01]),
    );

    let rmac_leaf2 = MacAddress([0xAA, 0xBB, 0xCC, 0x00, 0x00, 0x02]);
    let vtep_leaf2 = Ipv4Address::new(192, 168, 10, 2);

    let route = EvpnIpPrefixRoute::new(
        RouteDistinguisher::new(Ipv4Address::new(10, 0, 0, 2), 200),
        Ipv4Address::new(172, 16, 20, 0),
        24,
        50002,
        rmac_leaf2,
        vtep_leaf2,
    );

    vrf.add_prefix_route(route);

    let resolved = vrf.lookup(Ipv4Address::new(172, 16, 20, 101)).unwrap();
    assert_eq!(resolved.l3_vni, 50002);
    assert_eq!(resolved.router_mac, rmac_leaf2);
    assert_eq!(resolved.vtep_ip, vtep_leaf2);
}

#[test]
fn test_evpn_l3_constants() {
    assert_eq!(EVPN_ROUTE_TYPE_IP_PREFIX, 5);
    assert_eq!(BGP_EXT_COMMUNITY_ROUTER_MAC, 0x0603);
}
