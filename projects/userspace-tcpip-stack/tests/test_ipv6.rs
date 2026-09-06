use std::str::FromStr;
use toy_tcpip::ipv6::{Ipv6Address, Ipv6Packet, NEXT_HEADER_UDP, compute_ipv6_transport_checksum};

#[test]
fn test_ipv6_address_parsing_and_rfc5952_display() {
    let unspec = Ipv6Address::from_str("::").unwrap();
    assert_eq!(unspec, Ipv6Address::UNSPECIFIED);
    assert_eq!(unspec.to_string(), "::");

    let loopback = Ipv6Address::from_str("::1").unwrap();
    assert!(loopback.is_loopback());
    assert_eq!(loopback.to_string(), "::1");

    let global = Ipv6Address::from_str("2001:0db8:0000:0000:0000:ff00:0042:8329").unwrap();
    assert_eq!(global.to_string(), "2001:db8::ff00:42:8329");
}

#[test]
fn test_ipv6_packet_serialization_and_pseudo_header_checksum() {
    let src = Ipv6Address::from_str("fe80::1").unwrap();
    let dst = Ipv6Address::from_str("fe80::2").unwrap();
    let payload = b"IPv6 Payload Data";

    let raw = Ipv6Packet::serialize(src, dst, NEXT_HEADER_UDP, 128, payload);
    let parsed = Ipv6Packet::parse(&raw).unwrap();

    assert_eq!(parsed.header.version, 6);
    assert_eq!(parsed.header.src_ip, src);
    assert_eq!(parsed.header.dst_ip, dst);
    assert_eq!(parsed.header.payload_length, payload.len() as u16);
    assert_eq!(parsed.payload, payload);

    let csum = compute_ipv6_transport_checksum(src, dst, NEXT_HEADER_UDP, payload);
    assert_ne!(csum, 0);
}
