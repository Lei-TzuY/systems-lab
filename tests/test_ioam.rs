use toy_tcpip::ioam::{IoamPacket, IOAM_TRACE_BIT_INGRESS_EGRESS, IOAM_TRACE_BIT_NODE_ID, IOAM_TRACE_BIT_TIMESTAMP_NS, IOAM_TRACE_BIT_TRANSIT_DELAY, IOAM_TYPE_PREALLOC_TRACE};

#[test]
fn test_ioam_hop_telemetry_trace() {
    let mut pkt = IoamPacket::new(1, b"User Payload Traveling Across Data Center Fabric");

    // Leaf Switch 1
    pkt.trace_header.add_hop(101, 1, 2, 1700000000100000, 35);
    // Spine Switch 1
    pkt.trace_header.add_hop(201, 3, 4, 1700000000100040, 25);
    // Leaf Switch 2
    pkt.trace_header.add_hop(102, 2, 1, 1700000000100075, 40);

    let raw = pkt.serialize();
    assert_eq!(raw.len() >= 68, true);

    let parsed = IoamPacket::parse(&raw).unwrap();
    assert_eq!(parsed.trace_header.namespace_id, 1);
    assert_eq!(parsed.trace_header.node_records.len(), 3);

    assert_eq!(parsed.trace_header.node_records[0].node_id, 101);
    assert_eq!(parsed.trace_header.node_records[0].transit_delay_ns, 35);

    assert_eq!(parsed.trace_header.node_records[1].node_id, 201);
    assert_eq!(parsed.trace_header.node_records[1].transit_delay_ns, 25);

    assert_eq!(parsed.trace_header.node_records[2].node_id, 102);
    assert_eq!(parsed.trace_header.node_records[2].transit_delay_ns, 40);

    assert_eq!(&parsed.inner_payload, b"User Payload Traveling Across Data Center Fabric");
    assert_eq!(IOAM_TYPE_PREALLOC_TRACE, 0);
    assert_eq!(
        parsed.trace_header.trace_type,
        IOAM_TRACE_BIT_NODE_ID | IOAM_TRACE_BIT_INGRESS_EGRESS | IOAM_TRACE_BIT_TIMESTAMP_NS | IOAM_TRACE_BIT_TRANSIT_DELAY
    );
}
