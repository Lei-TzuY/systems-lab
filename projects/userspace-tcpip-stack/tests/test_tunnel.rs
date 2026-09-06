use toy_tcpip::ethernet::ETHERTYPE_IPV4;
use toy_tcpip::ipv4::{Ipv4Address, Ipv4Packet};
use toy_tcpip::tunnel::{GrePacket, IP_PROTO_GRE, IP_PROTO_IP_IN_IP};

#[test]
fn test_gre_packet_options_and_checksum() {
    let payload = b"Encapsulated IPv4 Packet over GRE Tunnel";
    let raw = GrePacket::serialize(ETHERTYPE_IPV4, Some(0x9988), Some(1), true, payload);
    let parsed = GrePacket::parse(&raw).unwrap();

    assert_eq!(parsed.header.protocol_type, ETHERTYPE_IPV4);
    assert_eq!(parsed.header.key, Some(0x9988));
    assert_eq!(parsed.header.sequence_number, Some(1));
    assert!(parsed.header.checksum.is_some());
    assert_eq!(parsed.payload, payload);
}

#[test]
fn test_gre_and_ip_in_ip_site_to_site_tunnel() {
    let outer_src = Ipv4Address::new(100, 64, 0, 1);
    let outer_dst = Ipv4Address::new(100, 64, 0, 2);
    let inner_payload = b"Top secret corporate LAN payload";

    // 1. GRE IPv4 Tunneling
    let gre_pkt = GrePacket::encapsulate_gre_ipv4(outer_src, outer_dst, inner_payload, Some(0x500));
    let parsed_outer_ip = Ipv4Packet::parse(&gre_pkt, true).unwrap();
    assert_eq!(parsed_outer_ip.header.protocol.to_u8(), IP_PROTO_GRE);

    let parsed_gre = GrePacket::parse(parsed_outer_ip.payload).unwrap();
    assert_eq!(parsed_gre.header.protocol_type, ETHERTYPE_IPV4);
    assert_eq!(parsed_gre.payload, inner_payload);

    // 2. IP-in-IP Direct Tunneling
    let ipip_pkt = GrePacket::encapsulate_ip_in_ip(outer_src, outer_dst, inner_payload);
    let parsed_ipip = Ipv4Packet::parse(&ipip_pkt, true).unwrap();
    assert_eq!(parsed_ipip.header.protocol.to_u8(), IP_PROTO_IP_IN_IP);
    assert_eq!(parsed_ipip.payload, inner_payload);
}
