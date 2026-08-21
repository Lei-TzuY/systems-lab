use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::pim::{
    ALL_PIM_ROUTERS_MULTICAST, IP_PROTO_PIM, PIM_TYPE_HELLO, PIM_TYPE_JOIN_PRUNE,
    PimMulticastRouter, PimPacket,
};

#[test]
fn test_pim_hello_packet_codec() {
    let hello = PimPacket::build_hello(105, 50);
    let raw = hello.serialize();

    let parsed = PimPacket::parse(&raw, true).unwrap();
    assert_eq!(parsed.header.version, 2);
    assert_eq!(parsed.header.msg_type, PIM_TYPE_HELLO);
    assert_eq!(IP_PROTO_PIM, 103);
    assert_eq!(ALL_PIM_ROUTERS_MULTICAST, Ipv4Address::new(224, 0, 0, 13));
}

#[test]
fn test_pim_join_prune_and_router_shared_tree() {
    let up = Ipv4Address::new(192, 168, 1, 1);
    let grp = Ipv4Address::new(239, 10, 10, 10);
    let rp = Ipv4Address::new(10, 254, 0, 1);

    let join_pkt = PimPacket::build_join_group(up, grp, rp);
    let raw = join_pkt.serialize();

    let parsed = PimPacket::parse(&raw, true).unwrap();
    assert_eq!(parsed.header.msg_type, PIM_TYPE_JOIN_PRUNE);

    let mut router = PimMulticastRouter::new(rp);
    router.join_shared_tree(grp);
    assert_eq!(router.active_groups.get(&grp), Some(&rp));

    router.prune_group(&grp);
    assert_eq!(router.active_groups.get(&grp), None);
}
