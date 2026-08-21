use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_type1::{
    EvpnAliasingEngine, EvpnEthernetAdRoute, ETHERNET_TAG_MAX_PER_ES, EVPN_ROUTE_TYPE_ETHERNET_AD,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_type1_aliasing_and_fast_mass_withdrawal() {
    let mut engine = EvpnAliasingEngine::new();
    let esi = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A];

    let pe1 = Ipv4Address::new(10, 1, 1, 1);
    let pe2 = Ipv4Address::new(10, 1, 1, 2);

    let route1 = EvpnEthernetAdRoute::new_per_es(RouteDistinguisher::new(pe1, 100), esi, pe1);
    let route2 = EvpnEthernetAdRoute::new_per_es(RouteDistinguisher::new(pe2, 100), esi, pe2);

    assert!(route1.is_per_es());
    assert_eq!(route1.ethernet_tag_id, ETHERNET_TAG_MAX_PER_ES);

    engine.add_ad_route(route1);
    engine.add_ad_route(route2);

    // Verify Aliasing ECMP NextHops
    let nhs = engine.get_aliasing_nexthops(&esi);
    assert_eq!(nhs.len(), 2);

    // Fast Mass Withdrawal of PE1
    let removed = engine.mass_withdraw(&esi, pe1);
    assert_eq!(removed, 1);

    let remaining = engine.get_aliasing_nexthops(&esi);
    assert_eq!(remaining, &[pe2]);
}

#[test]
fn test_evpn_type1_constants() {
    assert_eq!(EVPN_ROUTE_TYPE_ETHERNET_AD, 1);
    assert_eq!(ETHERNET_TAG_MAX_PER_ES, 0xFFFFFFFF);
}
