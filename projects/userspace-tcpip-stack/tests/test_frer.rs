use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::frer::{ETHERTYPE_RTAG, FrerEngine, RTagFrame};

#[test]
fn test_frer_rtag_frame_codec() {
    let dst = MacAddress([0x01, 0x80, 0xC2, 0x00, 0x00, 0x01]);
    let src = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);

    let frame = RTagFrame::new(dst, src, 42, 0x0800, vec![10, 20, 30, 40]);
    let raw = frame.serialize();

    let parsed = RTagFrame::parse(&raw).unwrap();
    assert_eq!(parsed.dst_mac, dst);
    assert_eq!(parsed.src_mac, src);
    assert_eq!(parsed.rtag.sequence_number, 42);
    assert_eq!(parsed.rtag.inner_ethertype, 0x0800);
    assert_eq!(parsed.payload, vec![10, 20, 30, 40]);
}

#[test]
fn test_frer_engine_hitless_failover() {
    let mut engine = FrerEngine::new();
    let dst = MacAddress([0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    let src = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);

    let (frame_a1, frame_b1) = engine.replicate(dst, src, 0x0800, b"Packet 1");
    let (frame_a2, frame_b2) = engine.replicate(dst, src, 0x0800, b"Packet 2");

    // Packet 1 arrives on Path A first
    assert!(engine.process_ingress_frame(&frame_a1).is_some());
    // Packet 1 duplicate on Path B arrives second -> eliminated
    assert!(engine.process_ingress_frame(&frame_b1).is_none());

    // Path A dropped Packet 2, but Path B delivers Packet 2 -> Accepted! (Hitless protection)
    assert!(engine.process_ingress_frame(&frame_b2).is_some());
    // Delayed Packet 2 arrives on Path A later -> eliminated
    assert!(engine.process_ingress_frame(&frame_a2).is_none());

    assert_eq!(engine.packets_forwarded, 2);
    assert_eq!(engine.packets_eliminated_duplicates, 2);
}

#[test]
fn test_frer_rtag_ethertype_constant() {
    assert_eq!(ETHERTYPE_RTAG, 0xF1C1);
}
