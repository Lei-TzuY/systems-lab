use toy_tcpip::tsn_qav_cbs::{CreditBasedShaperQueue, SrClass, TsnQavBridgePort};

#[test]
fn test_tsn_qav_cbs_dual_class_shaper() {
    let mut bridge_port = TsnQavBridgePort::new(100_000_000, 30_000_000, 20_000_000);

    // Class A queue test
    bridge_port.class_a.enqueue_frame(1200);
    let tx_a = bridge_port.class_a.try_transmit(0);
    assert_eq!(tx_a, Some(1200));

    bridge_port.class_a.complete_transmission(12_000);
    assert_eq!(bridge_port.class_a.total_transmitted_frames, 1);
    assert_eq!(bridge_port.class_a.total_transmitted_bytes, 1200);

    // Class B queue test
    let mut class_b = CreditBasedShaperQueue::new(SrClass::ClassB, 20_000_000, 100_000_000, 1500);
    class_b.enqueue_frame(800);
    let tx_b = class_b.try_transmit(0);
    assert_eq!(tx_b, Some(800));
}
