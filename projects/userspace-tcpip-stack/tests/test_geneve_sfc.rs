use toy_tcpip::geneve_sfc::{GENEVE_OPT_CLASS_SFC, GeneveSfcHop, GeneveSfcPacket};

#[test]
fn test_geneve_sfc_multi_hop_chain_and_options() {
    let hop = GeneveSfcHop {
        vni: 8080,
        service_path_id: 0x00AABB,
        service_index: 4,
        tenant_id: 1001,
        security_group: 42,
    };

    let payload = b"GET /v1/telemetry HTTP/1.1\r\nHost: internal.edge\r\n\r\n";
    let mut pkt = GeneveSfcPacket::build(8080, 0x0800, hop, payload);

    let raw = pkt.serialize();
    let parsed = GeneveSfcPacket::parse(&raw).unwrap();

    assert_eq!(parsed.vni, 8080);
    assert_eq!(parsed.protocol_type, 0x0800);
    assert_eq!(parsed.sfc_metadata.service_path_id, 0x00AABB);
    assert_eq!(parsed.sfc_metadata.service_index, 4);
    assert_eq!(parsed.sfc_metadata.tenant_id, 1001);
    assert_eq!(parsed.sfc_metadata.security_group, 42);
    assert_eq!(parsed.payload, payload);

    // Hop 1: Firewall
    assert!(pkt.advance_service_hop());
    assert_eq!(pkt.sfc_metadata.service_index, 3);

    // Hop 2: Deep Packet Inspection (DPI)
    assert!(pkt.advance_service_hop());
    assert_eq!(pkt.sfc_metadata.service_index, 2);

    // Hop 3: Web Application Firewall (WAF)
    assert!(pkt.advance_service_hop());
    assert_eq!(pkt.sfc_metadata.service_index, 1);

    // Hop 4: Egress / NAT Terminus
    assert!(pkt.advance_service_hop());
    assert_eq!(pkt.sfc_metadata.service_index, 0);

    // Cannot advance past 0
    assert!(!pkt.advance_service_hop());
}

#[test]
fn test_geneve_sfc_class_constant() {
    assert_eq!(GENEVE_OPT_CLASS_SFC, 0x0104);
}
