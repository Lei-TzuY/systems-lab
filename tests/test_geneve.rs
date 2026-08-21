use toy_tcpip::geneve::{GeneveOption, GenevePacket, ETHERTYPE_TRANSPARENT_ETH, GENEVE_BASE_HEADER_LEN, GENEVE_UDP_PORT};

#[test]
fn test_geneve_encapsulation_and_vni() {
    let inner_frame = b"Layer 2 Ethernet Frame for Tenant Subnet";
    let vni = 0x0ABCDE; // 24-bit VNI
    let raw = GenevePacket::encapsulate_eth(vni, inner_frame);

    assert_eq!(raw.len(), GENEVE_BASE_HEADER_LEN + inner_frame.len());
    let parsed = GenevePacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 0);
    assert_eq!(parsed.vni, vni);
    assert_eq!(parsed.protocol_type, ETHERTYPE_TRANSPARENT_ETH);
    assert_eq!(parsed.payload, inner_frame);
    assert_eq!(GENEVE_UDP_PORT, 6081);
}

#[test]
fn test_geneve_tlv_option_parsing() {
    let opt = GeneveOption {
        class: 0x0102,
        opt_type: 0x05,
        critical: true,
        data: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };

    let pkt = GenevePacket {
        version: 0,
        oam: false,
        critical: true,
        protocol_type: ETHERTYPE_TRANSPARENT_ETH,
        vni: 5001,
        options: vec![opt],
        payload: b"Data".to_vec(),
    };

    let serialized = pkt.serialize();
    let parsed = GenevePacket::parse(&serialized).unwrap();
    assert_eq!(parsed.options.len(), 1);
    assert_eq!(parsed.options[0].class, 0x0102);
    assert_eq!(parsed.options[0].critical, true);
    assert_eq!(parsed.options[0].data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
}
