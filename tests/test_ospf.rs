use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ospf::{OspfHelloPacket, OspfLsdb, OSPF_TYPE_HELLO, OSPF_VERSION_2};

#[test]
fn test_ospf_hello_packet_codec() {
    let r_id = Ipv4Address::new(10, 0, 0, 1);
    let mask = Ipv4Address::new(255, 255, 255, 0);
    let dr = Ipv4Address::new(10, 0, 0, 1);
    let neighbors = vec![Ipv4Address::new(10, 0, 0, 2), Ipv4Address::new(10, 0, 0, 3)];

    let hello = OspfHelloPacket::build_hello(r_id, mask, dr, neighbors.clone());
    let raw = hello.serialize();

    let parsed = OspfHelloPacket::parse(&raw, true).unwrap();
    assert_eq!(parsed.header.version, OSPF_VERSION_2);
    assert_eq!(parsed.header.msg_type, OSPF_TYPE_HELLO);
    assert_eq!(parsed.header.router_id, r_id);
    assert_eq!(parsed.network_mask, mask);
    assert_eq!(parsed.designated_router, dr);
    assert_eq!(parsed.neighbors, neighbors);
}

#[test]
fn test_ospf_dijkstra_shortest_path_graph() {
    let mut lsdb = OspfLsdb::new();
    let r1 = Ipv4Address::new(1, 1, 1, 1);
    let r4 = Ipv4Address::new(4, 4, 4, 4);

    // Add R3 <-> R4 with cost 5
    let r3 = Ipv4Address::new(3, 3, 3, 3);
    lsdb.add_link(r3, r4, 5);

    let paths = lsdb.compute_shortest_paths(r1);
    // Path to R4: R1 -> R2 (10) -> R3 (10) -> R4 (5) = total cost 25 via R2
    let (cost, next_hop) = paths.get(&r4).unwrap();
    assert_eq!(*cost, 25);
    assert_eq!(*next_hop, Some(Ipv4Address::new(2, 2, 2, 2)));
}
