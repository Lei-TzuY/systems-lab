use toy_tcpip::snmp::{
    SNMP_PDU_GET_REQUEST, SNMP_PDU_RESPONSE, SNMP_VERSION_2C, SnmpMessage, SnmpMib, SnmpValue,
    SnmpVarbind, encode_ber_integer, encode_ber_string,
};

#[test]
fn test_snmp_ber_primitives() {
    let int_bytes = encode_ber_integer(12345);
    assert_eq!(int_bytes[0], 0x02); // INTEGER tag

    let str_bytes = encode_ber_string("toy_snmp");
    assert_eq!(str_bytes[0], 0x04); // OCTET STRING tag
    assert_eq!(str_bytes[1], 8); // length
}

#[test]
fn test_snmp_end_to_end_query_and_response() {
    let req =
        SnmpMessage::build_get_request("public", 999, &["1.3.6.1.2.1.1.1.0", "1.3.6.1.2.1.1.3.0"]);
    let raw_req = req.serialize();

    let parsed_req = SnmpMessage::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.version, SNMP_VERSION_2C);
    assert_eq!(parsed_req.pdu.pdu_type, SNMP_PDU_GET_REQUEST);
    assert_eq!(parsed_req.pdu.request_id, 999);
    assert_eq!(parsed_req.pdu.varbinds.len(), 2);

    let mib = SnmpMib::new();
    let mut results = Vec::new();
    for vb in &parsed_req.pdu.varbinds {
        let val = mib.get(&vb.oid).cloned().unwrap_or(SnmpValue::Null);
        results.push(SnmpVarbind {
            oid: vb.oid.clone(),
            value: val,
        });
    }

    let resp = SnmpMessage::build_response(&parsed_req, results);
    let raw_resp = resp.serialize();
    let parsed_resp = SnmpMessage::parse(&raw_resp).unwrap();

    assert_eq!(parsed_resp.pdu.pdu_type, SNMP_PDU_RESPONSE);
    assert_eq!(parsed_resp.pdu.request_id, 999);
    assert_eq!(parsed_resp.pdu.varbinds.len(), 2);
}
