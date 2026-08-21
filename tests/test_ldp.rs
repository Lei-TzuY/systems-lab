use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ldp::{LdpPdu, LDP_MSG_HELLO, LDP_MSG_LABEL_MAPPING, LDP_PORT};

#[test]
fn test_ldp_hello_discovery_packet() {
    let lsr_id = Ipv4Address::new(172, 16, 0, 1);
    let hello = LdpPdu::build_hello(lsr_id, 30);
    let raw = hello.serialize();

    let parsed = LdpPdu::parse(&raw).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.lsr_id, lsr_id);
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.messages[0].msg_type, LDP_MSG_HELLO);
    assert_eq!(LDP_PORT, 646);
}

#[test]
fn test_ldp_label_mapping_and_bindings() {
    let lsr_id = Ipv4Address::new(10, 1, 1, 1);
    let prefix = Ipv4Address::new(10, 200, 0, 0);
    let pdu = LdpPdu::build_label_mapping(lsr_id, 55, prefix, 16, 500);
    let raw = pdu.serialize();

    let parsed = LdpPdu::parse(&raw).unwrap();
    assert_eq!(parsed.messages[0].msg_type, LDP_MSG_LABEL_MAPPING);
    assert_eq!(parsed.messages[0].msg_id, 55);

    let bindings = parsed.extract_bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].prefix, prefix);
    assert_eq!(bindings[0].prefix_len, 16);
    assert_eq!(bindings[0].label, 500);
}
