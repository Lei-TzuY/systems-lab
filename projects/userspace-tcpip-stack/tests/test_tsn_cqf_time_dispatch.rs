use toy_tcpip::tsn_cqf_time_dispatch::TsnCqfTimeDispatchEngine;

#[test]
fn test_tsn_cqf_time_dispatch_multi_cycle() {
    let mut cqf = TsnCqfTimeDispatchEngine::new(50_000); // 50us cycle

    cqf.enqueue_frame(1, 100);
    cqf.enqueue_frame(2, 200);

    // Within cycle 0 (30us) -> nothing drained
    let r1 = cqf.advance_time(30_000);
    assert!(r1.is_empty());

    // Reach 50us (Cycle 1) -> 2 frames from Cycle 0 drained
    let r2 = cqf.advance_time(20_000);
    assert_eq!(r2.len(), 2);
    assert_eq!(cqf.total_dispatched, 2);
}
