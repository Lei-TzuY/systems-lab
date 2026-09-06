use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::vtp::{VTP_MULTICAST_MAC, VtpEngine, VtpMode, VtpPacket, VtpVlanInfo};

#[test]
fn test_vtp_packet_framing_and_constants() {
    let updater = Ipv4Address::new(10, 0, 0, 1);
    let summary = VtpPacket::build_summary("CampusHQ", 8, updater);
    let raw = summary.serialize();

    let parsed = VtpPacket::parse(&raw).unwrap();
    if let VtpPacket::Summary(s) = parsed {
        assert_eq!(s.domain, "CampusHQ");
        assert_eq!(s.revision, 8);
        assert_eq!(s.updater_ip, updater);
    } else {
        panic!("Expected Summary Adv");
    }

    assert_eq!(VTP_MULTICAST_MAC.0, [0x01, 0x00, 0x0C, 0xCC, 0xCC, 0xCC]);
}

#[test]
fn test_vtp_engine_vlan_database_sync() {
    let mut server = VtpEngine::new("CampusHQ", VtpMode::Server);
    let mut client = VtpEngine::new("CampusHQ", VtpMode::Client);

    server.add_vlan(50, "WirelessUsers");
    assert_eq!(server.revision, 6);

    let vlan_list: Vec<VtpVlanInfo> = server
        .vlans
        .iter()
        .map(|(&id, name)| VtpVlanInfo {
            vlan_id: id,
            vlan_name: name.clone(),
            status: 0,
        })
        .collect();

    let subset_adv = VtpPacket::build_subset("CampusHQ", server.revision, &vlan_list);
    let raw = subset_adv.serialize();
    let parsed = VtpPacket::parse(&raw).unwrap();

    if let VtpPacket::Subset(sub) = parsed {
        let updated = client.sync_subset(&sub);
        assert!(updated);
        assert_eq!(client.revision, 6);
        assert_eq!(client.vlans.get(&50).unwrap(), "WirelessUsers");
    } else {
        panic!("Expected Subset Adv");
    }
}
