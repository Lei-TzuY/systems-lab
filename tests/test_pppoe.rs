use toy_tcpip::pppoe::{PppoePacket, PPPOE_CODE_PADI, PPPOE_CODE_SESSION_DATA, PPP_PROTO_IPV4};

#[test]
fn test_pppoe_padi_discovery_stage() {
    let padi = PppoePacket::build_padi();
    let raw = padi.serialize();

    let parsed = PppoePacket::parse(&raw).unwrap();
    assert_eq!(parsed.code, PPPOE_CODE_PADI);
    assert_eq!(parsed.session_id, 0);
    assert_eq!(parsed.version, 1);
}

#[test]
fn test_pppoe_session_stage_encapsulation() {
    let inner_ip = vec![0x45, 0x00, 0x00, 0x28, 0x00, 0x01, 0x00, 0x00, 0x40, 0x06];
    let session_pkt = PppoePacket::build_session_ipv4(0x1234, &inner_ip);
    let raw = session_pkt.serialize();

    let parsed = PppoePacket::parse(&raw).unwrap();
    assert_eq!(parsed.code, PPPOE_CODE_SESSION_DATA);
    assert_eq!(parsed.session_id, 0x1234);

    let proto = u16::from_be_bytes([parsed.payload[0], parsed.payload[1]]);
    assert_eq!(proto, PPP_PROTO_IPV4);
    assert_eq!(&parsed.payload[2..], &inner_ip);
}
