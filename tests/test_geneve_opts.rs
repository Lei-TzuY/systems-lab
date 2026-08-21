use toy_tcpip::geneve_opts::{
    GeneveOptionTlv, GENEVE_CLASS_CISCO, GENEVE_CLASS_OVS_LINUX, GENEVE_CLASS_STANDARD, GENEVE_CLASS_VMWARE,
    GENEVE_TYPE_INBAND_TELEMETRY, GENEVE_TYPE_SECURITY_GROUP, GENEVE_TYPE_SERVICE_CHAIN,
};

#[test]
fn test_geneve_opts_constants_and_padding() {
    assert_eq!(GENEVE_CLASS_STANDARD, 0x0100);
    assert_eq!(GENEVE_CLASS_CISCO, 0x0101);
    assert_eq!(GENEVE_CLASS_VMWARE, 0x0104);
    assert_eq!(GENEVE_CLASS_OVS_LINUX, 0x0108);

    assert_eq!(GENEVE_TYPE_SECURITY_GROUP, 0x01);
    assert_eq!(GENEVE_TYPE_INBAND_TELEMETRY, 0x02);
    assert_eq!(GENEVE_TYPE_SERVICE_CHAIN, 0x03);

    // Test automatic 4-byte padding of 3-byte slice
    let opt = GeneveOptionTlv::new(GENEVE_CLASS_CISCO, 0x10, false, &[1, 2, 3]);
    assert_eq!(opt.data.len(), 4);
    assert_eq!(opt.data, vec![1, 2, 3, 0]);

    let raw = opt.serialize();
    assert_eq!(raw.len(), 8); // 4 bytes header + 4 bytes data
}

#[test]
fn test_geneve_opts_parse_multiple_options() {
    let opt1 = GeneveOptionTlv::new(GENEVE_CLASS_OVS_LINUX, GENEVE_TYPE_SECURITY_GROUP, false, &[0x00, 0x00, 0x01, 0x00]);
    let opt2 = GeneveOptionTlv::new(GENEVE_CLASS_STANDARD, GENEVE_TYPE_SERVICE_CHAIN, true, &[0xAA, 0xBB, 0xCC, 0xDD]);

    let mut buf = Vec::new();
    buf.extend_from_slice(&opt1.serialize());
    buf.extend_from_slice(&opt2.serialize());

    let parsed = GeneveOptionTlv::parse_all(&buf);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].class, GENEVE_CLASS_OVS_LINUX);
    assert_eq!(parsed[0].critical, false);
    assert_eq!(parsed[1].class, GENEVE_CLASS_STANDARD);
    assert_eq!(parsed[1].critical, true);
    assert_eq!(parsed[1].data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
}
