use toy_tcpip::qos::{PacketPriority, PriorityScheduler, TokenBucket};

#[test]
fn test_token_bucket_rate_limiter() {
    let mut tb = TokenBucket::new(2000, 1000); // 2000B burst, 1000B/s rate

    assert!(tb.try_consume(1500, 0));
    assert!(!tb.try_consume(600, 0)); // Only 500 left

    // 1 second later -> 1000 tokens added (total 1500)
    assert!(tb.try_consume(1000, 1000));
}

#[test]
fn test_priority_scheduler_queue_order() {
    let mut scheduler = PriorityScheduler::new();

    scheduler.enqueue(PacketPriority::Normal, b"HTTP data".to_vec());
    scheduler.enqueue(PacketPriority::Low, b"Backup data".to_vec());
    scheduler.enqueue(PacketPriority::High, b"DNS/ICMP packet".to_vec());

    assert_eq!(scheduler.dequeue(), Some(b"DNS/ICMP packet".to_vec()));
    assert_eq!(scheduler.dequeue(), Some(b"HTTP data".to_vec()));
    assert_eq!(scheduler.dequeue(), Some(b"Backup data".to_vec()));
    assert_eq!(scheduler.dequeue(), None);
}
