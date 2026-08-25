use toy_tcpip::gtpu_redundant_paths::GtpuRedundantEngine;
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gtpu_redundant_path_deduplication() {
    let mut engine = GtpuRedundantEngine::new(
        202,
        Ipv4Address::new(10, 10, 1, 1),
        0x1000,
        Ipv4Address::new(10, 20, 2, 2),
        0x2000,
    );

    let (p1, p2) = engine.replicate_outgoing(b"Critical URLLC Control Msg");

    // First arrival
    let r1 = engine.ingest_incoming(p1.sequence_number, p1.payload);
    assert_eq!(r1, Some(b"Critical URLLC Control Msg".to_vec()));

    // Redundant arrival
    let r2 = engine.ingest_incoming(p2.sequence_number, p2.payload);
    assert_eq!(r2, None);
    assert_eq!(engine.total_duplicates_dropped, 1);
}
