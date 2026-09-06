use toy_tcpip::gtp_ext::{
    GTP_EXT_HDR_PDU_SESSION_CONTAINER, PDU_SESSION_TYPE_DL, PDU_SESSION_TYPE_UL,
    PduSessionContainer, build_gtpu_with_pdu_container, parse_gtpu_with_pdu_container,
};

#[test]
fn test_gtp_ext_constants() {
    assert_eq!(GTP_EXT_HDR_PDU_SESSION_CONTAINER, 0x85);
    assert_eq!(PDU_SESSION_TYPE_DL, 0);
    assert_eq!(PDU_SESSION_TYPE_UL, 1);
}

#[test]
fn test_gtp_ext_uplink_downlink_roundtrip() {
    // Test UL
    let ul = PduSessionContainer::new_ul(5);
    assert_eq!(ul.pdu_type, PDU_SESSION_TYPE_UL);
    assert_eq!(ul.qfi, 5);
    assert!(!ul.rqi);

    let bytes_ul = ul.serialize();
    let parsed_ul = PduSessionContainer::parse(&bytes_ul).unwrap();
    assert_eq!(parsed_ul, ul);

    // Test DL with RQI
    let dl = PduSessionContainer::new_dl(8, true);
    assert_eq!(dl.pdu_type, PDU_SESSION_TYPE_DL);
    assert_eq!(dl.qfi, 8);
    assert!(dl.rqi);

    let bytes_dl = dl.serialize();
    let parsed_dl = PduSessionContainer::parse(&bytes_dl).unwrap();
    assert_eq!(parsed_dl, dl);
}

#[test]
fn test_gtp_ext_full_packet_encap_decap() {
    let container = PduSessionContainer::new_dl(7, true);
    let payload = b"Hello 5G User Plane Core";
    let raw = build_gtpu_with_pdu_container(0x55AA1122, &container, payload);

    let (teid, c, p) = parse_gtpu_with_pdu_container(&raw).unwrap();
    assert_eq!(teid, 0x55AA1122);
    assert_eq!(c.qfi, 7);
    assert!(c.rqi);
    assert_eq!(p, payload);
}
