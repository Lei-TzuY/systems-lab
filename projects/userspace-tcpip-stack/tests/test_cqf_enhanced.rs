use toy_tcpip::cqf_enhanced::{CqfBufferedFrame, CqfDualBufferEngine, CqfPhase};

#[test]
fn test_cqf_enhanced_capacity_overflow_drop() {
    let mut engine = CqfDualBufferEngine::new(500, 1000); // 1000 bytes max capacity
    assert_eq!(engine.phase, CqfPhase::Even);

    // Enqueue 800 bytes -> Success
    assert!(engine.enqueue_frame(1, 10, vec![0x01; 800]));
    assert_eq!(engine.total_enqueued, 1);

    // Enqueue another 300 bytes (800 + 300 = 1100 > 1000) -> Drop!
    assert!(!engine.enqueue_frame(2, 20, vec![0x02; 300]));
    assert_eq!(engine.total_dropped, 1);
    assert_eq!(engine.queue_even.len(), 1);
}

#[test]
fn test_cqf_enhanced_multi_cycle_forwarding() {
    let mut engine = CqfDualBufferEngine::new(1000, 5000);

    // Cycle 0: Enqueue Frame A
    assert!(engine.enqueue_frame(10, 100, vec![0xAA; 100]));
    let frame_ref: &CqfBufferedFrame = &engine.queue_even[0];
    assert_eq!(frame_ref.frame_id, 10);

    // Cycle 1: Enqueue Frame B (odd queue), Drain Cycle 0 Frame A
    assert!(engine.enqueue_frame(20, 1050, vec![0xBB; 200]));
    let drained_c1 = engine.drain_transmitting_queue(1100);
    assert_eq!(drained_c1.len(), 1);
    assert_eq!(drained_c1[0].frame_id, 10);

    // Cycle 2: Drain Cycle 1 Frame B
    let drained_c2 = engine.drain_transmitting_queue(2100);
    assert_eq!(drained_c2.len(), 1);
    assert_eq!(drained_c2[0].frame_id, 20);
    assert_eq!(engine.total_drained, 2);
}
