use toy_tcpip::glbp::{GlbpEngine, GlbpPacket, GlbpRole, GLBP_MULTICAST_IP, GLBP_UDP_PORT};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_glbp_packet_framing_and_constants() {
    let vip = Ipv4Address::new(10, 1, 1, 254);
    let pkt = GlbpPacket::build_hello(5, 110, 1, 100, vip);
    let raw = pkt.serialize();

    let parsed = GlbpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.group, 5);
    assert_eq!(parsed.priority, 110);
    assert_eq!(parsed.forwarder_num, 1);
    assert_eq!(parsed.virtual_ip, vip);
    assert_eq!(GLBP_UDP_PORT, 3222);
    assert_eq!(GLBP_MULTICAST_IP, Ipv4Address::new(224, 0, 0, 102));

    let vmac = GlbpPacket::virtual_mac(5, 1);
    assert_eq!(vmac.0, [0x00, 0x07, 0xB4, 0x00, 5, 1]);
}

#[test]
fn test_glbp_engine_active_forwarder_round_robin() {
    let vip = Ipv4Address::new(192, 168, 100, 1);
    let mut engine = GlbpEngine::new(2, 120, vip);
    assert_eq!(engine.role, GlbpRole::ActiveVirtualGateway);
    engine.active_forwarders = vec![1, 2];

    let mac_f1 = engine.resolve_arp_reply_mac();
    let mac_f2 = engine.resolve_arp_reply_mac();
    let mac_f1_again = engine.resolve_arp_reply_mac();

    assert_eq!(mac_f1.0, [0x00, 0x07, 0xB4, 0x00, 2, 1]);
    assert_eq!(mac_f2.0, [0x00, 0x07, 0xB4, 0x00, 2, 2]);
    assert_eq!(mac_f1_again.0, [0x00, 0x07, 0xB4, 0x00, 2, 1]);
}
