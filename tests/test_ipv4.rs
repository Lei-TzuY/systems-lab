use std::str::FromStr;
use toy_tcpip::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet, IP_PROTO_UDP};

#[test]
fn test_ipv4_address_basics() {
    let ip = Ipv4Address::new(172, 16, 0, 1);
    assert_eq!(format!("{}", ip), "172.16.0.1");

    let parsed = Ipv4Address::from_str("172.16.0.1").unwrap();
    assert_eq!(parsed, ip);
    assert!(!ip.is_loopback());

    let localhost = Ipv4Address::LOCALHOST;
    assert!(localhost.is_loopback());

    let bcast = Ipv4Address::BROADCAST;
    assert!(bcast.is_broadcast());
}

#[test]
fn test_ipv4_checksum_validation() {
    let src = Ipv4Address::new(192, 168, 1, 5);
    let dst = Ipv4Address::new(192, 168, 1, 1);
    let payload = b"Network Protocol Verification";

    let mut raw = Ipv4Packet::serialize(src, dst, IP_PROTO_UDP, 0x4321, 128, payload);

    // Parse valid packet with checksum check
    let pkt = Ipv4Packet::parse(&raw, true).expect("valid ipv4");
    assert_eq!(pkt.header.src_ip, src);
    assert_eq!(pkt.header.dst_ip, dst);
    assert_eq!(pkt.header.ttl, 128);
    assert_eq!(pkt.header.protocol, IpProtocol::Udp);
    assert_eq!(pkt.payload, payload);

    // Corrupt one byte in header
    raw[0] = 0x46; // Change IHL
    let err = Ipv4Packet::parse(&raw, true);
    assert!(err.is_err());
}
