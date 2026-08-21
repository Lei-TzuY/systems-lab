use std::str::FromStr;
use toy_tcpip::ipv6::{Ipv6Address, Ipv6Header};
use toy_tcpip::srv6::{IPV6_EXT_ROUTING, SRV6_ROUTING_TYPE, Srv6Header};

#[test]
fn test_srv6_header_codec_and_srh_attributes() {
    let s1 = Ipv6Address::from_str("fd00:1::1").unwrap();
    let s2 = Ipv6Address::from_str("fd00:2::1").unwrap();
    let srh = Srv6Header::build(6, &[s1, s2]);

    let raw = srh.serialize();
    let (parsed, len) = Srv6Header::parse(&raw).unwrap();

    assert_eq!(len, 8 + 16 * 2);
    assert_eq!(parsed.next_header, 6);
    assert_eq!(parsed.segments_left, 1);
    assert_eq!(parsed.last_entry, 1);
    assert_eq!(parsed.segment_list.len(), 2);
    assert_eq!(IPV6_EXT_ROUTING, 43);
    assert_eq!(SRV6_ROUTING_TYPE, 4);
}

#[test]
fn test_srv6_multi_segment_forwarding() {
    let s1 = Ipv6Address::from_str("2001:db8:a::1").unwrap();
    let s2 = Ipv6Address::from_str("2001:db8:b::1").unwrap();
    let mut srh = Srv6Header::build(17, &[s1, s2]);

    let mut ip_hdr = Ipv6Header {
        version: 6,
        traffic_class: 0,
        flow_label: 0,
        payload_length: 0,
        next_header: IPV6_EXT_ROUTING,
        hop_limit: 64,
        src_ip: Ipv6Address::from_str("2001:db8::1").unwrap(),
        dst_ip: s2,
    };

    assert_eq!(srh.segments_left, 1);
    assert!(srh.advance_hop(&mut ip_hdr));
    assert_eq!(ip_hdr.dst_ip, s1);
    assert_eq!(srh.segments_left, 0);
    assert!(!srh.advance_hop(&mut ip_hdr));
}
