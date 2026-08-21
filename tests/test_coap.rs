use toy_tcpip::coap::{CoapPacket, COAP_CODE_205_CONTENT, COAP_CODE_GET, COAP_OPT_URI_PATH, COAP_TYPE_ACK, COAP_TYPE_CON, COAP_UDP_PORT};

#[test]
fn test_coap_get_and_content_roundtrip() {
    let get = CoapPacket::build_get(0x8899, "status", &[0x01, 0x02]);
    let raw = get.serialize();

    let parsed = CoapPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.msg_type, COAP_TYPE_CON);
    assert_eq!(parsed.code, COAP_CODE_GET);
    assert_eq!(parsed.message_id, 0x8899);
    assert_eq!(parsed.options[0].number, COAP_OPT_URI_PATH);
    assert_eq!(parsed.options[0].value, b"status");
    assert_eq!(COAP_UDP_PORT, 5683);

    let resp = CoapPacket::build_response(&parsed, COAP_CODE_205_CONTENT, b"Running");
    let resp_raw = resp.serialize();
    let parsed_resp = CoapPacket::parse(&resp_raw).unwrap();
    assert_eq!(parsed_resp.msg_type, COAP_TYPE_ACK);
    assert_eq!(parsed_resp.code, COAP_CODE_205_CONTENT);
    assert_eq!(parsed_resp.payload, b"Running");
}
