use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::RouteDistinguisher;
use toy_tcpip::evpn_mass_withdraw::{EvpnMassWithdrawEngine, EvpnPerEsAdRoute};
use toy_tcpip::evpn_synch::EthernetSegmentId;
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_per_es_ad_route_codec() {
    let rd = RouteDistinguisher::new(Ipv4Address::new(192, 168, 1, 1), 100);
    let esi = EthernetSegmentId::from_u32(42);
    let next_hop = Ipv4Address::new(192, 168, 1, 1);

    let route = EvpnPerEsAdRoute::new(rd.clone(), esi, next_hop);
    assert_eq!(route.ethernet_tag_id, 0xFFFF_FFFF);
    assert_eq!(route.mpls_label, 0);

    let wire = route.serialize();
    assert_eq!(wire.len(), 25);

    let parsed = EvpnPerEsAdRoute::parse(&wire, next_hop).expect("parse per-ES route");
    assert_eq!(parsed.rd, rd);
    assert_eq!(parsed.esi, esi);
    assert_eq!(parsed.ethernet_tag_id, 0xFFFF_FFFF);
    assert_eq!(parsed.next_hop, next_hop);
}

#[test]
fn test_evpn_fast_convergence_mass_withdraw_failover() {
    let mut engine = EvpnMassWithdrawEngine::new();
    let es1 = EthernetSegmentId::from_u32(101);
    let vni = 500;

    let pe1_primary = Ipv4Address::new(192, 168, 1, 1);
    let pe2_backup = Ipv4Address::new(192, 168, 1, 2);

    let mac_a = MacAddress([0x52, 0x54, 0x00, 0x01, 0x01, 0x01]);
    let mac_b = MacAddress([0x52, 0x54, 0x00, 0x01, 0x01, 0x02]);

    engine.register_mac(es1, vni, mac_a, None, pe1_primary, pe2_backup);
    engine.register_mac(es1, vni, mac_b, None, pe1_primary, pe2_backup);

    assert_eq!(engine.lookup_active_pe(vni, mac_a), Some(pe1_primary));
    assert_eq!(engine.lookup_active_pe(vni, mac_b), Some(pe1_primary));

    // Simulate link failure on ES1 (Mass Withdrawal of Route Type 1)
    let flipped = engine.process_es_failure_mass_withdraw(&es1);
    assert_eq!(flipped, 2);

    // Fast reroute to backup PE instantly in O(1)
    assert_eq!(engine.lookup_active_pe(vni, mac_a), Some(pe2_backup));
    assert_eq!(engine.lookup_active_pe(vni, mac_b), Some(pe2_backup));

    // Recovery
    let restored = engine.process_es_recovery(&es1);
    assert_eq!(restored, 2);
    assert_eq!(engine.lookup_active_pe(vni, mac_a), Some(pe1_primary));
    assert_eq!(engine.lookup_active_pe(vni, mac_b), Some(pe1_primary));
}
