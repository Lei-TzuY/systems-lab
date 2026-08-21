use toy_tcpip::dhcp::{DhcpMessageType, DhcpPacket};
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_dhcp_dora_handshake_packets() {
    let client_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let xid = 0xabcdef12;

    // 1. Discover
    let discover = DhcpPacket::build_discover(client_mac, xid);
    let disc_bytes = discover.serialize();
    let disc_parsed = DhcpPacket::parse(&disc_bytes).unwrap();
    assert_eq!(disc_parsed.msg_type, DhcpMessageType::Discover);
    assert_eq!(disc_parsed.xid, xid);

    // 2. Offer
    let offer = DhcpPacket::build_offer(
        client_mac,
        xid,
        Ipv4Address::new(10, 0, 0, 50),
        Ipv4Address::new(10, 0, 0, 1),
        Ipv4Address::new(255, 255, 255, 0),
        Ipv4Address::new(10, 0, 0, 1),
        Ipv4Address::new(1, 1, 1, 1),
        3600,
    );
    let offer_bytes = offer.serialize();
    let offer_parsed = DhcpPacket::parse(&offer_bytes).unwrap();
    assert_eq!(offer_parsed.msg_type, DhcpMessageType::Offer);
    assert_eq!(offer_parsed.yiaddr, Ipv4Address::new(10, 0, 0, 50));
    assert_eq!(offer_parsed.server_id, Some(Ipv4Address::new(10, 0, 0, 1)));
    assert_eq!(offer_parsed.lease_time, Some(3600));
}
