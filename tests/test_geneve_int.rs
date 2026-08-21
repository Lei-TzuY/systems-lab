use toy_tcpip::geneve_int::{GeneveIntPacket, IntHopTelemetry, GENEVE_OPT_CLASS_INT, GENEVE_OPT_TYPE_INT_HOP};

#[test]
fn test_geneve_int_hop_collection_and_decoding() {
    let mut pkt = GeneveIntPacket::build(8888, 0x0800, Vec::new(), b"Payload Packet Data");

    // Add 2 telemetry hops
    pkt.add_hop_telemetry(IntHopTelemetry {
        switch_id: 1,
        ingress_port: 10,
        egress_port: 20,
        hop_latency_ns: 250,
        queue_depth_bytes: 512,
    });
    pkt.add_hop_telemetry(IntHopTelemetry {
        switch_id: 2,
        ingress_port: 20,
        egress_port: 30,
        hop_latency_ns: 300,
        queue_depth_bytes: 4096,
    });

    assert_eq!(pkt.calculate_total_latency_ns(), 550);
    assert_eq!(pkt.max_queue_depth_bytes(), 4096);

    let raw = pkt.serialize();
    let parsed = GeneveIntPacket::parse(&raw).unwrap();

    assert_eq!(parsed.vni, 8888);
    assert_eq!(parsed.telemetry_hops.len(), 2);
    assert_eq!(parsed.telemetry_hops[0].hop_latency_ns, 250);
    assert_eq!(parsed.telemetry_hops[1].hop_latency_ns, 300);
}

#[test]
fn test_geneve_int_constants() {
    assert_eq!(GENEVE_OPT_CLASS_INT, 0x0105);
    assert_eq!(GENEVE_OPT_TYPE_INT_HOP, 0x01);
}
