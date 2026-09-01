use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_dhcp_snooping::{
    DhcpOption82, DhcpSnoopMsgType, DhcpSnoopPacket, DhcpSnoopVerdict, EvpnDhcpSnoopingEngine,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_dhcp_snooping_lifecycle() {
    let leaf_mac = MacAddress([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
    let mut engine = EvpnDhcpSnoopingEngine::new(leaf_mac);

    let untrusted_port = 2;
    let trusted_server_port = 8;
    let vni = 500;

    engine.set_port_trusted(trusted_server_port, true);
    assert!(engine.is_port_trusted(trusted_server_port));
    assert!(!engine.is_port_trusted(untrusted_port));

    let client_mac = MacAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    let offered_ip = Ipv4Address::new(192, 168, 50, 101);

    // 1. Client sends DHCP Discover -> Option 82 inserted
    let discover = DhcpSnoopPacket {
        msg_type: DhcpSnoopMsgType::Discover,
        xid: 0x99887766,
        client_mac,
        assigned_ip: Ipv4Address::new(0, 0, 0, 0),
        lease_time_secs: 0,
        option_82: None,
    };
    let v1 = engine.process_dhcp_packet(vni, untrusted_port, discover, 5000);
    if let DhcpSnoopVerdict::Forward(fwd) = v1 {
        let opt82 = fwd.option_82.expect("Option 82 injected");
        assert_eq!(opt82.circuit_id, "vni:500/port:2");
        assert_eq!(opt82.remote_id, leaf_mac);
    } else {
        panic!("Expected Forward");
    }

    // 2. Rogue DHCP Offer on untrusted port -> Dropped
    let rogue_offer = DhcpSnoopPacket {
        msg_type: DhcpSnoopMsgType::Offer,
        xid: 0x99887766,
        client_mac,
        assigned_ip: Ipv4Address::new(10, 0, 0, 99),
        lease_time_secs: 3600,
        option_82: None,
    };
    assert_eq!(
        engine.process_dhcp_packet(vni, untrusted_port, rogue_offer, 5001),
        DhcpSnoopVerdict::DropRogueServerResponse
    );

    // 3. Legitimate DHCP ACK on trusted port -> Option 82 stripped & binding registered
    let legit_ack = DhcpSnoopPacket {
        msg_type: DhcpSnoopMsgType::Ack,
        xid: 0x99887766,
        client_mac,
        assigned_ip: offered_ip,
        lease_time_secs: 86400,
        option_82: Some(DhcpOption82 {
            circuit_id: "vni:500/port:2".to_string(),
            remote_id: leaf_mac,
        }),
    };
    let v3 = engine.process_dhcp_packet(vni, trusted_server_port, legit_ack, 5002);
    if let DhcpSnoopVerdict::Forward(fwd) = v3 {
        assert!(fwd.option_82.is_none());
    } else {
        panic!("Expected Forward");
    }

    assert_eq!(engine.bindings.len(), 1);
    assert_eq!(engine.bindings[0].mac, client_mac);
    assert_eq!(engine.bindings[0].ip, offered_ip);
    assert_eq!(engine.bindings[0].lease_expiry_secs, 5002 + 86400);
}
