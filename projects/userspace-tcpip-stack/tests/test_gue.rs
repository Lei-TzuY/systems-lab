use toy_tcpip::gue::{
    FOU_UDP_PORT, FouPacket, GUE_PROTO_GRE, GUE_PROTO_IPV4, GUE_PROTO_IPV6, GUE_UDP_PORT, GuePacket,
};

#[test]
fn test_gue_encapsulation_protocols() {
    // 1. IPv4 over GUE
    let ip4_payload = b"Generic UDP Encapsulated IPv4 Packet";
    let gue_ip4 = GuePacket::build_ipv4(ip4_payload);
    let raw4 = gue_ip4.serialize();
    let parsed4 = GuePacket::parse(&raw4).unwrap();
    assert_eq!(parsed4.header.next_proto, GUE_PROTO_IPV4);
    assert_eq!(parsed4.payload, ip4_payload);

    // 2. IPv6 over GUE
    let ip6_payload = b"Generic UDP Encapsulated IPv6 Packet";
    let gue_ip6 = GuePacket::build_ipv6(ip6_payload);
    let raw6 = gue_ip6.serialize();
    let parsed6 = GuePacket::parse(&raw6).unwrap();
    assert_eq!(parsed6.header.next_proto, GUE_PROTO_IPV6);

    // 3. GRE over GUE
    let gre_payload = b"Generic UDP Encapsulated GRE Packet";
    let gue_gre = GuePacket::build_gre(gre_payload);
    let raw_gre = gue_gre.serialize();
    let parsed_gre = GuePacket::parse(&raw_gre).unwrap();
    assert_eq!(parsed_gre.header.next_proto, GUE_PROTO_GRE);

    assert_eq!(GUE_UDP_PORT, 6080);
}

#[test]
fn test_fou_tunnel_handling() {
    let payload = b"Direct Foo-over-UDP Packet";
    let fou = FouPacket::build_ip(payload);
    let raw = fou.serialize();

    let parsed = FouPacket::parse(GUE_PROTO_IPV4, &raw);
    assert_eq!(parsed.proto, GUE_PROTO_IPV4);
    assert_eq!(parsed.payload, payload);
    assert_eq!(FOU_UDP_PORT, 5555);
}
