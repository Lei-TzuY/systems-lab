use toy_tcpip::tsn_ats_multihop::AtsMultiHopPipeline;

#[test]
fn test_tsn_ats_multihop_latency_bounds() {
    let mut pipeline = AtsMultiHopPipeline::new(4, 50_000); // 4 hops, 50us switching latency per hop

    // Stream 10: CIR = 50 MB/s (50,000,000 B/s), CBS = 1500 B
    pipeline.configure_stream_across_hops(10, 50_000_000, 1500);

    // Ingest 500-byte frame at ingress
    pipeline.ingest_ingress(10, 6, 500, 0);

    // Simulate clock ticks
    for t in (50_000..=500_000).step_by(50_000) {
        pipeline.step_simulation(t);
    }

    assert_eq!(pipeline.delivered_frames.len(), 1);
    let frame = &pipeline.delivered_frames[0];
    assert_eq!(frame.stream_id, 10);
    assert_eq!(frame.hops_traversed, 4);
    assert_eq!(frame.payload_bytes, 500);
}
