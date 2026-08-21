use std::str::FromStr;
use toy_tcpip::gre_v6::{
    GreIpv6Packet, ETHERTYPE_ETHERNET_IN_GRE, ETHERTYPE_IPV4_IN_GRE, ETHERTYPE_IPV6_IN_GRE, ETHERTYPE_MPLS_IN_GRE,
};
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_gre_ipv6_constants_and_encapsulation() {
    assert_eq!(ETHERTYPE_IPV4_IN_GRE, 0x0800);
    assert_eq!(ETHERTYPE_IPV6_IN_GRE, 0x86DD);
    assert_eq!(ETHERTYPE_MPLS_IN_GRE, 0x8847);
    assert_eq!(ETHERTYPE_ETHERNET_IN_GRE, 0x6558);

    let src6 = Ipv6Address::from_str("2001:db8:cafe::1").unwrap();
    let dst6 = Ipv6Address::from_str("2001:db8:beef::2").unwrap();

    let inner_data = b"Encapsulated IPv4 Packet traversing GRE-over-IPv6 backbone tunnel";
    let gre_pkt = GreIpv6Packet::new(
        src6,
        dst6,
        ETHERTYPE_IPV4_IN_GRE,
        Some(0x12345678),
        Some(42),
        inner_data,
    );

    let raw = gre_pkt.serialize();
    let parsed = GreIpv6Packet::parse(&raw).unwrap();

    assert_eq!(parsed.src_ip6, src6);
    assert_eq!(parsed.dst_ip6, dst6);
    assert_eq!(parsed.protocol_type, ETHERTYPE_IPV4_IN_GRE);
    assert_eq!(parsed.key, Some(0x12345678));
    assert_eq!(parsed.sequence, Some(42));
    assert_eq!(&parsed.payload, inner_data);
}

#[test]
fn test_gre_ipv6_multiprotocol_payloads() {
    let src6 = Ipv6Address::from_str("fe80::1").unwrap();
    let dst6 = Ipv6Address::from_str("fe80::2").unwrap();

    let mpls_bytes = vec![0x00, 0x06, 0x41, 0x40, 0xDE, 0xAD, 0xBE, 0xEF];
    let gre_mpls = GreIpv6Packet::new(src6, dst6, ETHERTYPE_MPLS_IN_GRE, None, None, &mpls_bytes);

    let raw_mpls = gre_mpls.serialize();
    let parsed_mpls = GreIpv6Packet::parse(&raw_mpls).unwrap();

    assert_eq!(parsed_mpls.protocol_type, ETHERTYPE_MPLS_IN_GRE);
    assert_eq!(parsed_mpls.key, None);
    assert_eq!(parsed_mpls.sequence, None);
    assert_eq!(parsed_mpls.payload, mpls_bytes);
}
