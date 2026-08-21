use toy_tcpip::bgp_ls::{
    BgpLsLinkDescriptor, BgpLsNlri, BgpLsNodeDescriptor, BgpLsTopologyDatabase,
    BGP_AFI_BGP_LS, BGP_SAFI_BGP_LS,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_bgp_ls_constants_and_nlri_encoding() {
    assert_eq!(BGP_AFI_BGP_LS, 16388);
    assert_eq!(BGP_SAFI_BGP_LS, 71);

    let node = BgpLsNodeDescriptor {
        asn: 64512,
        igp_router_id: Ipv4Address::new(10, 255, 0, 1),
        node_name: Some("Core-Router-A".to_string()),
    };

    let nlri_node = BgpLsNlri::Node(node.clone());
    let raw = nlri_node.serialize();
    assert!(raw.len() >= 16);

    let parsed = BgpLsNlri::parse(&raw).unwrap();
    if let BgpLsNlri::Node(n) = parsed {
        assert_eq!(n.asn, 64512);
        assert_eq!(n.igp_router_id, Ipv4Address::new(10, 255, 0, 1));
        assert_eq!(n.node_name, Some("Core-Router-A".to_string()));
    } else {
        panic!("Expected Node NLRI");
    }
}

#[test]
fn test_bgp_ls_topology_database_link_ingest() {
    let mut db = BgpLsTopologyDatabase::new();

    let node1 = BgpLsNodeDescriptor {
        asn: 65000,
        igp_router_id: Ipv4Address::new(172, 16, 1, 1),
        node_name: Some("Spine-1".to_string()),
    };
    let node2 = BgpLsNodeDescriptor {
        asn: 65000,
        igp_router_id: Ipv4Address::new(172, 16, 1, 2),
        node_name: Some("Leaf-1".to_string()),
    };

    db.ingest_nlri(BgpLsNlri::Node(node1.clone()));
    db.ingest_nlri(BgpLsNlri::Node(node2.clone()));

    let link = BgpLsLinkDescriptor {
        local_node: node1,
        remote_node: node2,
        local_interface_ip: Ipv4Address::new(10, 0, 0, 1),
        remote_neighbor_ip: Ipv4Address::new(10, 0, 0, 2),
        te_metric: 50,
        max_bandwidth_bps: 400_000_000_000.0,
        max_reservable_bandwidth_bps: 320_000_000_000.0,
        admin_group_color: 0xFF,
    };
    db.ingest_nlri(BgpLsNlri::Link(link));

    assert_eq!(db.nodes.len(), 2);
    assert_eq!(db.links.len(), 1);
    assert_eq!(db.links[0].te_metric, 50);
    assert_eq!(db.links[0].local_interface_ip, Ipv4Address::new(10, 0, 0, 1));
}
