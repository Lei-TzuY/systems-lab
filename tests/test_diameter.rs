use toy_tcpip::diameter::{
    DIAMETER_AVP_ORIGIN_HOST, DIAMETER_AVP_RESULT_CODE, DIAMETER_CMD_CAPABILITIES_EXCHANGE,
    DIAMETER_CMD_DEVICE_WATCHDOG, DIAMETER_FLAG_REQUEST, DIAMETER_PORT, DIAMETER_SUCCESS,
    DIAMETER_VERSION_1, DiameterAvp, DiameterMessage, DiameterServer,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_diameter_avp_and_cer_cea_flow() {
    let host = "pgw01.epc.mnc001.mcc001.3gppnetwork.org";
    let realm = "epc.mnc001.mcc001.3gppnetwork.org";
    let ip = Ipv4Address::new(172, 16, 1, 10);

    let cer = DiameterMessage::build_cer(host, realm, ip, 10415, "ToyStack-Core", 1, 1);
    let raw = cer.serialize();

    let parsed_cer = DiameterMessage::parse(&raw).unwrap();
    assert_eq!(parsed_cer.header.version, DIAMETER_VERSION_1);
    assert_eq!(
        parsed_cer.header.command_code,
        DIAMETER_CMD_CAPABILITIES_EXCHANGE
    );
    assert!(parsed_cer.header.flags & DIAMETER_FLAG_REQUEST != 0);

    let server = DiameterServer::new("hss01.epc.mnc001.mcc001.3gppnetwork.org", realm);
    let cea = server.handle_request(&parsed_cer);
    let raw_cea = cea.serialize();

    let parsed_cea = DiameterMessage::parse(&raw_cea).unwrap();
    assert!(parsed_cea.header.flags & DIAMETER_FLAG_REQUEST == 0); // Answer
    assert_eq!(parsed_cea.avps[0].code, DIAMETER_AVP_RESULT_CODE);

    assert_eq!(DIAMETER_PORT, 3868);
    assert_eq!(DIAMETER_SUCCESS, 2001);
}

#[test]
fn test_diameter_vendor_avp_and_watchdog() {
    let mut avp = DiameterAvp::new(264, b"test-host");
    avp.vendor_id = Some(10415);
    let raw_avp = avp.serialize();

    let (parsed_avp, consumed) = DiameterAvp::parse(&raw_avp).unwrap();
    assert_eq!(parsed_avp.code, 264);
    assert_eq!(parsed_avp.vendor_id, Some(10415));
    assert_eq!(parsed_avp.data, b"test-host");
    assert_eq!(consumed, raw_avp.len());

    assert_eq!(DIAMETER_AVP_ORIGIN_HOST, 264);
    assert_eq!(DIAMETER_CMD_DEVICE_WATCHDOG, 280);
}
