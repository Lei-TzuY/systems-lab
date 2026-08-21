use toy_tcpip::hsrp::{HSRP_MULTICAST_IP, HSRP_UDP_PORT, HsrpEngine, HsrpPacket, HsrpState};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_hsrp_packet_framing_and_constants() {
    let vip = Ipv4Address::new(172, 16, 1, 1);
    let pkt = HsrpPacket::build_hello(HsrpState::Standby, 10, 105, vip);
    let raw = pkt.serialize();

    let parsed = HsrpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 0);
    assert_eq!(parsed.state, HsrpState::Standby);
    assert_eq!(parsed.group, 10);
    assert_eq!(parsed.priority, 105);
    assert_eq!(parsed.virtual_ip, vip);
    assert_eq!(HSRP_UDP_PORT, 1985);
    assert_eq!(HSRP_MULTICAST_IP, Ipv4Address::new(224, 0, 0, 2));

    let vmac = HsrpPacket::virtual_mac(10);
    assert_eq!(vmac.0, [0x00, 0x00, 0x0C, 0x07, 0xAC, 10]);
}

#[test]
fn test_hsrp_failover_and_preemption() {
    let vip = Ipv4Address::new(192, 168, 50, 1);
    let mut engine = HsrpEngine::new(2, 120, vip, true);

    let peer_active = HsrpPacket::build_hello(HsrpState::Active, 2, 90, vip);
    engine.process_packet(&peer_active, Ipv4Address::new(192, 168, 50, 2));

    // Preempts lower priority 90 -> becomes Active
    assert_eq!(engine.state, HsrpState::Active);

    let superior_active = HsrpPacket::build_hello(HsrpState::Active, 2, 150, vip);
    engine.process_packet(&superior_active, Ipv4Address::new(192, 168, 50, 3));

    // Steps down to Standby
    assert_eq!(engine.state, HsrpState::Standby);
}
