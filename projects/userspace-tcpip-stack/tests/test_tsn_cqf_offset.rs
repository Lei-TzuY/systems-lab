use toy_tcpip::tsn_cqf_offset::TsnCqfOffsetEngine;

#[test]
fn test_tsn_cqf_multihop_offset_forwarding() {
    let mut engine = TsnCqfOffsetEngine::new();
    engine.add_hop(20_000, 2000, 1000); // 20us cycle, 3us transit
    engine.add_hop(20_000, 2000, 1000); // 20us cycle, 3us transit

    let frame = engine.forward_frame_multihop(1, 1024);
    assert_eq!(frame.current_hop, 2);
    assert_eq!(frame.accumulated_latency_ns, 46_000); // (3000+20000) * 2 = 46000
    assert_eq!(engine.total_frames_forwarded, 1);
}
