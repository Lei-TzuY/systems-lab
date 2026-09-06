use toy_tcpip::lldp::{LLDP_TLV_CHASSIS_ID, LldpNeighbor, LldpNeighborTable, LldpPacket, LldpTlv};

#[test]
fn test_lldp_tlv_codec() {
    let tlv = LldpTlv {
        tlv_type: LLDP_TLV_CHASSIS_ID,
        value: b"switch-core-01".to_vec(),
    };
    let raw = tlv.serialize();
    let (parsed, consumed) = LldpTlv::parse(&raw).unwrap();

    assert_eq!(consumed, raw.len());
    assert_eq!(parsed.tlv_type, LLDP_TLV_CHASSIS_ID);
    assert_eq!(parsed.value, b"switch-core-01");
}

#[test]
fn test_lldp_packet_and_neighbor_discovery() {
    let pkt = LldpPacket {
        chassis_id: "00:50:56:c0:00:08".to_string(),
        port_id: "TenGigabitEthernet1/1".to_string(),
        ttl: 120,
        system_name: Some("Datacenter-ToR-01".to_string()),
    };

    let raw = pkt.serialize();
    let parsed = LldpPacket::parse(&raw).unwrap();

    assert_eq!(parsed.chassis_id, "00:50:56:c0:00:08");
    assert_eq!(parsed.port_id, "TenGigabitEthernet1/1");
    assert_eq!(parsed.ttl, 120);
    assert_eq!(parsed.system_name, Some("Datacenter-ToR-01".to_string()));

    let mut table = LldpNeighborTable::new();
    table.insert(LldpNeighbor {
        chassis_id: parsed.chassis_id.clone(),
        port_id: parsed.port_id.clone(),
        ttl: parsed.ttl,
        system_name: parsed.system_name.clone(),
    });

    assert!(table.all_neighbors().contains_key("00:50:56:c0:00:08"));
}
