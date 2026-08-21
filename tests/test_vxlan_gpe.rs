use toy_tcpip::vxlan_gpe::{VxlanGpePacket, VXLAN_GPE_NP_ETHERNET, VXLAN_GPE_NP_IPV4, VXLAN_GPE_NP_IPV6, VXLAN_GPE_NP_MPLS, VXLAN_GPE_UDP_PORT};

#[test]
fn test_vxlan_gpe_multiprotocol_encapsulation() {
    // 1. IPv4 Next Protocol
    let ip4_pkt = VxlanGpePacket::build_ipv4(1001, b"Raw IPv4 Packet");
    let raw4 = ip4_pkt.serialize();
    let parsed4 = VxlanGpePacket::parse(&raw4).unwrap();
    assert_eq!(parsed4.header.vni, 1001);
    assert_eq!(parsed4.header.next_protocol, VXLAN_GPE_NP_IPV4);
    assert_eq!(parsed4.payload, b"Raw IPv4 Packet");

    // 2. IPv6 Next Protocol
    let ip6_pkt = VxlanGpePacket::build_ipv6(1002, b"Raw IPv6 Packet");
    let raw6 = ip6_pkt.serialize();
    let parsed6 = VxlanGpePacket::parse(&raw6).unwrap();
    assert_eq!(parsed6.header.vni, 1002);
    assert_eq!(parsed6.header.next_protocol, VXLAN_GPE_NP_IPV6);

    // 3. Ethernet & MPLS Next Protocols
    let eth_pkt = VxlanGpePacket::build_ethernet(1003, b"Raw Ethernet Frame");
    assert_eq!(eth_pkt.header.next_protocol, VXLAN_GPE_NP_ETHERNET);

    let mpls_pkt = VxlanGpePacket::build_mpls(1004, b"Raw MPLS Packet");
    assert_eq!(mpls_pkt.header.next_protocol, VXLAN_GPE_NP_MPLS);
    assert_eq!(VXLAN_GPE_UDP_PORT, 4790);
}
