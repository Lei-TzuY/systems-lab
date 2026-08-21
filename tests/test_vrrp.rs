use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::vrrp::{VrrpEngine, VrrpPacket, VrrpState, VRRP_VERSION_3};

#[test]
fn test_vrrp_packet_structure_and_checksum() {
    let vip = Ipv4Address::new(10, 0, 0, 1);
    let pkt = VrrpPacket {
        version: VRRP_VERSION_3,
        msg_type: 1,
        vrid: 42,
        priority: 255, // Master Owner
        count_ip: 1,
        max_adver_int: 100,
        checksum: 0,
        ip_addresses: vec![vip],
    };

    let raw = pkt.serialize();
    let parsed = VrrpPacket::parse(&raw, true).unwrap();

    assert_eq!(parsed.vrid, 42);
    assert_eq!(parsed.priority, 255);
    assert_eq!(parsed.ip_addresses[0], vip);
    assert_eq!(VrrpPacket::virtual_mac(42), MacAddress([0x00, 0x00, 0x5e, 0x00, 0x01, 42]));
}

#[test]
fn test_vrrp_preemption_and_failover() {
    let mut backup_router = VrrpEngine::new(5, 150, Ipv4Address::new(192, 168, 1, 1));
    assert_eq!(backup_router.state, VrrpState::Backup);

    // Advertisement with lower priority (100) received -> Backup preempts to become Master!
    let lower_adv = VrrpPacket {
        version: VRRP_VERSION_3,
        msg_type: 1,
        vrid: 5,
        priority: 100,
        count_ip: 1,
        max_adver_int: 100,
        checksum: 0,
        ip_addresses: vec![Ipv4Address::new(192, 168, 1, 1)],
    };

    let changed = backup_router.process_advertisement(&lower_adv);
    assert!(changed);
    assert_eq!(backup_router.state, VrrpState::Master);
}
