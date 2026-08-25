use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::srv6_mup_interworking::{MupSessionMapping, Srv6MupInterworkingEngine};

#[test]
fn test_srv6_mup_stateless_interworking_flow() {
    let mut engine = Srv6MupInterworkingEngine::new();

    let gnb_ipv6 = Ipv6Address::new([0x2001, 0x0db8, 0x0010, 0, 0, 0, 0, 1]);
    let sid_edge = Ipv6Address::new([0x2001, 0x0db8, 0xcafe, 0, 0, 0, 0, 9]);

    engine.register_mapping(MupSessionMapping {
        gtp_teid: 0x55443322,
        gnodeb_ip: gnb_ipv6,
        srv6_segments: vec![sid_edge],
        qfi: 5,
    });

    // 1. Uplink GTP-U frame from gNodeB -> End.M.GTP6.D translation
    let payload = b"5G VoNR Voice RTP Stream".to_vec();
    let srv6_pkt = engine.end_m_gtp6_d(gnb_ipv6, 0x55443322, 5, payload.clone());
    assert!(srv6_pkt.is_some());
    let pkt = srv6_pkt.unwrap();
    assert_eq!(pkt.src_ip, gnb_ipv6);
    assert_eq!(pkt.dst_ip, sid_edge);
    assert_eq!(pkt.qfi, 5);
    assert_eq!(pkt.inner_payload, payload);

    // 2. Downlink SRv6 frame towards gNodeB -> End.M.GTP6.E translation
    let local_pe = Ipv6Address::new([0x2001, 0x0db8, 0x0020, 0, 0, 0, 0, 1]);
    let gtpu_pkt = engine.end_m_gtp6_e(local_pe, gnb_ipv6, 0x99887766, 5, payload.clone());
    assert_eq!(gtpu_pkt.src_ip, local_pe);
    assert_eq!(gtpu_pkt.dst_ip, gnb_ipv6);
    assert_eq!(gtpu_pkt.teid, 0x99887766);
    assert_eq!(gtpu_pkt.qfi, 5);
    assert_eq!(gtpu_pkt.payload, payload);

    assert_eq!(engine.translations_to_srv6, 1);
    assert_eq!(engine.translations_to_gtp, 1);
}

#[test]
fn test_srv6_mup_unmapped_teid_returns_none() {
    let mut engine = Srv6MupInterworkingEngine::new();
    let gnb = Ipv6Address::new([0x2001, 0x0db8, 0, 0, 0, 0, 0, 1]);
    let res = engine.end_m_gtp6_d(gnb, 0xDEADBEEF, 1, vec![0; 32]);
    assert!(res.is_none());
}
