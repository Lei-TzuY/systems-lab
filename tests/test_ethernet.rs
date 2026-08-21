use std::str::FromStr;
use toy_tcpip::ethernet::{EtherType, EthernetFrame, MacAddress, ETHERTYPE_ARP, ETHERTYPE_IPV4, ETHERTYPE_IPV6};

#[test]
fn test_mac_address_display_and_parsing() {
    let mac = MacAddress([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc]);
    assert_eq!(format!("{}", mac), "12:34:56:78:9a:bc");

    let parsed = MacAddress::from_str("12:34:56:78:9a:bc").unwrap();
    assert_eq!(parsed, mac);

    assert!(MacAddress::BROADCAST.is_broadcast());
    assert!(!MacAddress::BROADCAST.is_unicast());

    let unicast = MacAddress([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e]);
    assert!(unicast.is_unicast());
    assert!(!unicast.is_multicast());

    let multicast = MacAddress([0x01, 0x00, 0x5e, 0x00, 0x00, 0x01]);
    assert!(multicast.is_multicast());
}

#[test]
fn test_ethernet_frame_ethertypes() {
    let src = MacAddress([0xaa, 0xbb, 0xcc, 0x11, 0x22, 0x33]);
    let dst = MacAddress([0xdd, 0xee, 0xff, 0x44, 0x55, 0x66]);

    let f_ip = EthernetFrame::serialize(dst, src, ETHERTYPE_IPV4, b"ip-payload");
    let parsed_ip = EthernetFrame::parse(&f_ip).unwrap();
    assert_eq!(parsed_ip.ethertype, EtherType::IPv4);

    let f_arp = EthernetFrame::serialize(dst, src, ETHERTYPE_ARP, b"arp-payload");
    let parsed_arp = EthernetFrame::parse(&f_arp).unwrap();
    assert_eq!(parsed_arp.ethertype, EtherType::Arp);

    let f_ipv6 = EthernetFrame::serialize(dst, src, ETHERTYPE_IPV6, b"ipv6-payload");
    let parsed_ipv6 = EthernetFrame::parse(&f_ipv6).unwrap();
    assert_eq!(parsed_ipv6.ethertype, EtherType::IPv6);
}
