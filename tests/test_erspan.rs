use toy_tcpip::erspan::{ErspanPacket, NvgrePacket, ERSPAN_TYPE2_HEADER_LEN, ETHERTYPE_ERSPAN_TYPE2, ETHERTYPE_NVGRE_ETHERNET};

#[test]
fn test_erspan_type2_port_mirroring() {
    let original_payload = b"Payload mirrored from Switch Port 1 to Monitoring Server";
    let encap = ErspanPacket::encapsulate(202, 100, 1, original_payload);

    assert_eq!(encap.len(), ERSPAN_TYPE2_HEADER_LEN + original_payload.len());
    let parsed = ErspanPacket::parse(&encap).unwrap();

    assert_eq!(parsed.header.session_id, 202);
    assert_eq!(parsed.header.vlan, 100);
    assert_eq!(parsed.header.index, 1);
    assert_eq!(parsed.mirrored_frame, original_payload);
    assert_eq!(ETHERTYPE_ERSPAN_TYPE2, 0x88BE);
    assert_eq!(ETHERTYPE_NVGRE_ETHERNET, 0x6558);
}

#[test]
fn test_nvgre_virtual_subnet_encap() {
    let frame = b"NVGRE Inner Ethernet Frame";
    let encap = NvgrePacket::encapsulate(50001, 1, frame);
    assert_eq!(encap, frame);
}
