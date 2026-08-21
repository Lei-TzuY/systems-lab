use toy_tcpip::ethernet::{EthernetFrame, MacAddress, ETHERTYPE_IPV4};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::mpls::{MplsHeader, MplsPacket};
use toy_tcpip::vpls::{PwControlWord, VplsInstance, VplsPseudowire, PW_CONTROL_WORD_LEN};

#[test]
fn test_pw_control_word_codec() {
    let cw = PwControlWord::new(12345);
    let bytes = cw.serialize();
    let parsed = PwControlWord::parse(&bytes).unwrap();
    assert_eq!(parsed.seq_num, 12345);
    assert_eq!(PW_CONTROL_WORD_LEN, 4);
}

#[test]
fn test_vpls_mesh_multipoint_bridging() {
    let mut vpls = VplsInstance::new(200);

    let pw1 = VplsPseudowire {
        peer_ip: Ipv4Address::new(10, 1, 1, 1),
        vc_label_tx: 2001,
        vc_label_rx: 3001,
        tunnel_label_tx: 100,
    };
    vpls.add_pseudowire(pw1);

    let site_a_mac = MacAddress([0x00, 0x50, 0x56, 0x01, 0x02, 0x03]);
    let site_b_mac = MacAddress([0x00, 0x50, 0x56, 0xAA, 0xBB, 0xCC]);

    let frame = EthernetFrame::serialize(site_b_mac, site_a_mac, ETHERTYPE_IPV4, b"VPLS Data Traffic");

    // Ingress packet over PW1
    let cw = PwControlWord::new(1);
    let mut payload = Vec::new();
    payload.extend_from_slice(&cw.serialize());
    payload.extend_from_slice(&frame);

    let mpls_frame = MplsPacket::new(
        vec![MplsHeader::new(3001, 0, true, 64)],
        payload,
    ).serialize();

    let (dst_mac, decapped) = vpls.process_ingress_vpls(&mpls_frame).unwrap();
    assert_eq!(dst_mac, site_b_mac);
    assert_eq!(decapped, frame);
    assert_eq!(vpls.mac_table.get(&site_a_mac), Some(&Some(3001)));

    // Egress packet to Site A
    let encapped = vpls.encapsulate_frame(site_a_mac, &frame, 2).unwrap();
    let mpls_parsed = MplsPacket::parse(&encapped).unwrap();
    assert_eq!(mpls_parsed.labels[0].label, 100);
    assert_eq!(mpls_parsed.labels[1].label, 2001);
}
