use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::rip::{
    RIP_AFI_IPV4, RIP_CMD_RESPONSE, RIP_VERSION_2, RipEngine, RipEntry, RipPacket,
};

#[test]
fn test_rip_packet_parse_and_serialize() {
    let entry = RipEntry {
        address_family: RIP_AFI_IPV4,
        route_tag: 0,
        ip: Ipv4Address::new(172, 16, 0, 0),
        subnet_mask: Ipv4Address::new(255, 255, 0, 0),
        next_hop: Ipv4Address::UNSPECIFIED,
        metric: 3,
    };

    let packet = RipPacket {
        command: RIP_CMD_RESPONSE,
        version: RIP_VERSION_2,
        routing_domain: 0,
        entries: vec![entry],
    };

    let raw = packet.serialize();
    let parsed = RipPacket::parse(&raw).unwrap();

    assert_eq!(parsed.command, RIP_CMD_RESPONSE);
    assert_eq!(parsed.version, RIP_VERSION_2);
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].ip, Ipv4Address::new(172, 16, 0, 0));
    assert_eq!(parsed.entries[0].metric, 3);
}

#[test]
fn test_rip_distance_vector_convergence() {
    let mut router_a = RipEngine::new();
    router_a.add_local_network(Ipv4Address::new(192, 168, 10, 0), 24, "eth0");

    let mut router_b = RipEngine::new();
    router_b.add_local_network(Ipv4Address::new(192, 168, 20, 0), 24, "eth1");

    // Router A advertises to Router B
    let adv_a = router_a.build_advertisement();
    let updated = router_b.process_advertisement(Ipv4Address::new(192, 168, 10, 1), &adv_a, "eth0");
    assert_eq!(updated, 1);

    // Verify Router B learned route to 192.168.10.0/24
    let route = router_b
        .routes
        .lookup(Ipv4Address::new(192, 168, 10, 55))
        .unwrap();
    assert_eq!(route.gateway, Some(Ipv4Address::new(192, 168, 10, 1)));
}
