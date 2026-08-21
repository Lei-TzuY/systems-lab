use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_multihoming::{
    EvpnDfElectionEngine, EvpnEthernetSegmentRoute, EVPN_ROUTE_TYPE_ETHERNET_SEGMENT,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_all_active_df_election() {
    let pe1 = Ipv4Address::new(10, 0, 0, 1);
    let pe2 = Ipv4Address::new(10, 0, 0, 2);
    let pe3 = Ipv4Address::new(10, 0, 0, 3);
    let esi = [0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22];

    let mut engine = EvpnDfElectionEngine::new(pe2);
    engine.add_segment_peer(esi, pe1);
    engine.add_segment_peer(esi, pe2);
    engine.add_segment_peer(esi, pe3);

    // Sorted PE list: [10.0.0.1 (0), 10.0.0.2 (1), 10.0.0.3 (2)]
    // VID 10 -> 10 % 3 = 1 (PE2) -> Local PE2 is DF
    assert!(engine.is_designated_forwarder(&esi, 10));

    // VID 11 -> 11 % 3 = 2 (PE3) -> Local PE2 is Non-DF
    assert!(!engine.is_designated_forwarder(&esi, 11));

    // VID 12 -> 12 % 3 = 0 (PE1) -> Local PE2 is Non-DF
    assert!(!engine.is_designated_forwarder(&esi, 12));
}

#[test]
fn test_evpn_es_route_and_constant() {
    let es_route = EvpnEthernetSegmentRoute::new(
        RouteDistinguisher::new(Ipv4Address::new(192, 168, 1, 1), 1),
        [1; 10],
        Ipv4Address::new(192, 168, 1, 1),
    );
    assert_eq!(es_route.esi, [1; 10]);
    assert_eq!(EVPN_ROUTE_TYPE_ETHERNET_SEGMENT, 4);
}
