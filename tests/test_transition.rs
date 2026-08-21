use std::str::FromStr;
use toy_tcpip::ipv4::{Ipv4Address, Ipv4Packet};
use toy_tcpip::ipv6::{Ipv6Address, Ipv6Packet};
use toy_tcpip::transition::{Tunnel4in6, Tunnel6in4, IP_PROTO_IPV6_IN_IPV4, NEXT_HEADER_IPV4_IN_IPV6};

#[test]
fn test_6in4_tunnel_rfc4213() {
    let local_v4 = Ipv4Address::new(192, 0, 2, 1);
    let remote_v4 = Ipv4Address::new(198, 51, 100, 2);
    let tunnel = Tunnel6in4::new(local_v4, remote_v4);

    let src_v6 = Ipv6Address::from_str("2001:db8:feed::1").unwrap();
    let dst_v6 = Ipv6Address::from_str("2001:db8:cafe::2").unwrap();
    let inner_ip6 = Ipv6Packet::serialize(src_v6, dst_v6, 59, 64, b"Dual-stack 6in4 encapsulation");

    let encap = tunnel.encapsulate(&inner_ip6, 42);
    let outer_ip4 = Ipv4Packet::parse(&encap, true).unwrap();
    assert_eq!(outer_ip4.header.protocol.to_u8(), IP_PROTO_IPV6_IN_IPV4);
    assert_eq!(outer_ip4.header.src_ip, local_v4);
    assert_eq!(outer_ip4.header.dst_ip, remote_v4);

    let decapsulated = tunnel.decapsulate(&outer_ip4).unwrap();
    let parsed_ip6 = Ipv6Packet::parse(decapsulated).unwrap();
    assert_eq!(parsed_ip6.header.src_ip, src_v6);
    assert_eq!(parsed_ip6.header.dst_ip, dst_v6);
    assert_eq!(parsed_ip6.payload, b"Dual-stack 6in4 encapsulation");
    assert_eq!(IP_PROTO_IPV6_IN_IPV4, 41);
}

#[test]
fn test_4in6_tunnel_rfc2473() {
    let local_v6 = Ipv6Address::from_str("2001:db8:1000::1").unwrap();
    let remote_v6 = Ipv6Address::from_str("2001:db8:2000::2").unwrap();
    let tunnel = Tunnel4in6::new(local_v6, remote_v6);

    let src_v4 = Ipv4Address::new(10, 10, 1, 1);
    let dst_v4 = Ipv4Address::new(10, 20, 2, 2);
    let inner_ip4 = Ipv4Packet::serialize(src_v4, dst_v4, 0, 77, 64, b"Dual-stack 4in6 encapsulation");

    let encap = tunnel.encapsulate(&inner_ip4);
    let outer_ip6 = Ipv6Packet::parse(&encap).unwrap();
    assert_eq!(outer_ip6.header.next_header, NEXT_HEADER_IPV4_IN_IPV6);
    assert_eq!(outer_ip6.header.src_ip, local_v6);
    assert_eq!(outer_ip6.header.dst_ip, remote_v6);

    let decapsulated = tunnel.decapsulate(&outer_ip6).unwrap();
    let parsed_ip4 = Ipv4Packet::parse(decapsulated, true).unwrap();
    assert_eq!(parsed_ip4.header.src_ip, src_v4);
    assert_eq!(parsed_ip4.header.dst_ip, dst_v4);
    assert_eq!(parsed_ip4.payload, b"Dual-stack 4in6 encapsulation");
    assert_eq!(NEXT_HEADER_IPV4_IN_IPV6, 4);
}
