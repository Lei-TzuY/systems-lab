use toy_tcpip::bgp_epe::{
    BGP_EPE_PEER_ADJ_SID, BGP_EPE_PEER_NODE_SID, BGP_EPE_PEER_SET_SID, BgpEpeDatabase, PeerSid,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_bgp_epe_sid_types_and_constants() {
    assert_eq!(BGP_EPE_PEER_NODE_SID, 1);
    assert_eq!(BGP_EPE_PEER_ADJ_SID, 2);
    assert_eq!(BGP_EPE_PEER_SET_SID, 3);

    let sid = PeerSid {
        sid_type: BGP_EPE_PEER_NODE_SID,
        label: 24001,
        peer_asn: 64512,
        peer_ip: Ipv4Address::new(203, 0, 113, 10),
        egress_interface_id: None,
        weight: 100,
    };
    assert_eq!(sid.label, 24001);
}

#[test]
fn test_bgp_epe_database_lookup_and_multipath() {
    let mut epe = BgpEpeDatabase::new();
    let peer1 = Ipv4Address::new(203, 0, 113, 10);
    let peer2 = Ipv4Address::new(203, 0, 113, 20);

    epe.add_peer_node_sid(24001, 64512, peer1);
    epe.add_peer_adj_sid(24002, 64512, peer1, 4);
    epe.add_peer_set_member(24003, 64512, peer1, Some(1), 60);
    epe.add_peer_set_member(24003, 64513, peer2, Some(2), 40);

    // Resolve PeerNode-SID
    let res1 = epe.resolve_egress_path(24001);
    assert_eq!(res1.len(), 1);
    assert_eq!(res1[0].peer_asn, 64512);

    // Resolve PeerAdj-SID
    let res2 = epe.resolve_egress_path(24002);
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].egress_interface_id, Some(4));

    // Resolve PeerSet-SID
    let res3 = epe.resolve_egress_path(24003);
    assert_eq!(res3.len(), 2);
    assert_eq!(res3[0].weight + res3[1].weight, 100);
}
