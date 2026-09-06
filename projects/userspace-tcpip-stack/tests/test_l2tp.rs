use toy_tcpip::l2tp::{IP_PROTO_L2TPV3, L2tpv3Packet};

#[test]
fn test_l2tpv3_pseudowire_header_and_payload() {
    let eth_frame = vec![
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, 0x02, 0x00, 0x00, 0x00, 0x00, 0x10, 0x08, 0x00, 0x45,
        0x00, 0x00, 0x14,
    ];

    let session_id = 0x12345678;
    let encap = L2tpv3Packet::encapsulate(session_id, &eth_frame, None);
    assert_eq!(encap.len(), 4 + eth_frame.len());

    let (parsed_sid, inner) = L2tpv3Packet::decapsulate(&encap, false).unwrap();
    assert_eq!(parsed_sid, session_id);
    assert_eq!(inner, eth_frame);
    assert_eq!(IP_PROTO_L2TPV3, 115);
}

#[test]
fn test_l2tpv3_cookie_protection() {
    let payload = b"Sensitive VLAN Tagged Traffic";
    let cookie = 0x1122334455667788;
    let encap = L2tpv3Packet::encapsulate(42, payload, Some(cookie));

    let parsed = L2tpv3Packet::parse(&encap, true).unwrap();
    assert_eq!(parsed.session_id, 42);
    assert_eq!(parsed.cookie, Some(cookie));
    assert_eq!(parsed.inner_frame, payload);
}
