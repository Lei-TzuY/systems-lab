use toy_tcpip::evpn_umt_ir::EvpnUmtEngine;
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_umt_replication_tree() {
    let mut umt = EvpnUmtEngine::new(Ipv4Address::new(10, 0, 0, 1));

    let remote1 = Ipv4Address::new(10, 0, 0, 2);
    let remote2 = Ipv4Address::new(10, 0, 0, 3);
    let remote3 = Ipv4Address::new(10, 0, 0, 4);

    umt.add_inclusive_vtep(100, remote1);
    umt.add_inclusive_vtep(100, remote2);
    umt.add_inclusive_vtep(100, remote3);

    let g1 = Ipv4Address::new(239, 10, 10, 10);
    umt.add_selective_receiver(100, g1, remote1);

    // Replicate to g1
    let t1 = umt.resolve_replication_targets(100, g1);
    assert_eq!(t1.len(), 1);
    assert_eq!(t1[0], remote1);
    assert_eq!(umt.total_leaves_pruned, 2);

    // Fallback to inclusive for unjoined group
    let g2 = Ipv4Address::new(239, 20, 20, 20);
    let t2 = umt.resolve_replication_targets(100, g2);
    assert_eq!(t2.len(), 3);
}
