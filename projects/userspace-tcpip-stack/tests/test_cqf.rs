use toy_tcpip::cqf::CqfEngine;

#[test]
fn test_cqf_deterministic_queuing_and_bounds() {
    let mut cqf = CqfEngine::new(125);
    assert_eq!(cqf.latency_bounds_us(), (125, 250));

    // Cycle 0: Enqueue 3 packets
    cqf.enqueue(1, 7, vec![1, 2, 3]);
    cqf.enqueue(2, 7, vec![4, 5, 6]);
    cqf.enqueue(3, 6, vec![7, 8, 9]);

    // Tick to Cycle 1 -> Transmits all 3 packets from Cycle 0
    let tx1 = cqf.advance_cycle();
    assert_eq!(tx1.len(), 3);
    assert_eq!(tx1[0].id, 1);
    assert_eq!(tx1[1].id, 2);
    assert_eq!(tx1[2].id, 3);

    // Cycle 1: Enqueue 1 packet
    cqf.enqueue(4, 7, vec![10, 11, 12]);

    // Tick to Cycle 2 -> Transmits packet from Cycle 1
    let tx2 = cqf.advance_cycle();
    assert_eq!(tx2.len(), 1);
    assert_eq!(tx2[0].id, 4);

    assert_eq!(cqf.transmitted_packets_count, 4);
}
