use toy_tcpip::cfm::{CfmEngine, CfmPacket, CFM_MULTICAST_CLASS1, CFM_OPCODE_CCM, CFM_OPCODE_LBR, ETHERTYPE_CFM};

#[test]
fn test_cfm_ccm_heartbeat_and_mep_tracking() {
    let mut engine = CfmEngine::new(1, 3, "metro.ethernet.vpn100");

    // Peer MEP 50 sends CCM
    let ccm = CfmPacket::build_ccm(3, 50, 42, "metro.ethernet.vpn100", false);
    assert_eq!(ccm.header.opcode, CFM_OPCODE_CCM);
    assert_eq!(ccm.header.md_level, 3);

    let raw = ccm.serialize();
    let resp = engine.process_cfm_frame(&raw);
    assert!(resp.is_none());

    let peer = engine.remote_meps.get(&50).unwrap();
    assert_eq!(peer.last_seq, 42);
    assert_eq!(peer.ccm_count, 1);
    assert_eq!(peer.rdi, false);
}

#[test]
fn test_cfm_loopback_reply_generation() {
    let mut engine = CfmEngine::new(1, 3, "metro.ethernet.vpn100");
    let lbm = CfmPacket::build_lbm(3, 9999, b"IEEE 802.1ag Ping Payload");
    let raw = lbm.serialize();

    let lbr = engine.process_cfm_frame(&raw).unwrap();
    assert_eq!(lbr.header.opcode, CFM_OPCODE_LBR);
    assert_eq!(lbr.header.md_level, 3);
    assert_eq!(&lbr.payload[0..4], &9999u32.to_be_bytes());
}

#[test]
fn test_cfm_constants() {
    assert_eq!(ETHERTYPE_CFM, 0x8902);
    assert_eq!(CFM_MULTICAST_CLASS1.0, [0x01, 0x80, 0xC2, 0x00, 0x00, 0x30]);
}
