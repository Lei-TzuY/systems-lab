use std::str::FromStr;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::icmpv6::{ICMPV6_TYPE_ECHO_REPLY, ICMPV6_TYPE_ECHO_REQUEST, Icmpv6Packet, NdpTable};
use toy_tcpip::ipv6::Ipv6Address;

#[test]
fn test_icmpv6_ping_roundtrip() {
    let host_a = Ipv6Address::from_str("2001:db8::a").unwrap();
    let host_b = Ipv6Address::from_str("2001:db8::b").unwrap();

    let echo_req = Icmpv6Packet::build_echo_request(host_a, host_b, 0x5555, 1, b"Ping6 test");
    let parsed_req = Icmpv6Packet::parse(host_a, host_b, &echo_req, true).unwrap();
    assert_eq!(parsed_req.msg_type, ICMPV6_TYPE_ECHO_REQUEST);

    let echo_reply = Icmpv6Packet::build_echo_reply(host_b, host_a, 0x5555, 1, b"Ping6 test");
    let parsed_reply = Icmpv6Packet::parse(host_b, host_a, &echo_reply, true).unwrap();
    assert_eq!(parsed_reply.msg_type, ICMPV6_TYPE_ECHO_REPLY);
}

#[test]
fn test_ndp_neighbor_solicitation_and_cache() {
    let client_ip = Ipv6Address::from_str("fe80::100").unwrap();
    let client_mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let target_ip = Ipv6Address::from_str("fe80::1").unwrap();
    let target_mac = MacAddress([0x52, 0x54, 0x00, 0x99, 0x88, 0x77]);

    let ns = Icmpv6Packet::build_neighbor_solicitation(client_ip, target_ip, target_ip, client_mac);
    let parsed_ns = Icmpv6Packet::parse(client_ip, target_ip, &ns, true).unwrap();
    assert_eq!(parsed_ns.code, 0);

    let na = Icmpv6Packet::build_neighbor_advertisement(
        target_ip, client_ip, target_ip, target_mac, true, true, true,
    );
    let parsed_na = Icmpv6Packet::parse(target_ip, client_ip, &na, true).unwrap();
    assert_eq!(parsed_na.code, 0);

    let mut cache = NdpTable::new();
    cache.insert(target_ip, target_mac);
    assert_eq!(cache.lookup(&target_ip), Some(target_mac));
}
