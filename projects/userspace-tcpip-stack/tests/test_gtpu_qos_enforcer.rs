use toy_tcpip::gtpu_qos_enforcer::{FiveQiResourceType, GtpuQosEnforcer, QosVerdict};

#[test]
fn test_gtpu_qos_enforcer_gbr_and_ambr() {
    let mut enforcer = GtpuQosEnforcer::new(500, 500_000, 1000); // 500 KB/s, 1000B burst

    enforcer.register_qfi(5, 1, FiveQiResourceType::Gbr, 2, 100);
    enforcer.register_qfi(9, 9, FiveQiResourceType::NonGbr, 9, 300);

    // GBR (QFI 5) always passes AMBR token bucket
    let v_gbr = enforcer.enforce_packet(5, 1500, 0);
    assert_eq!(v_gbr, QosVerdict::Pass { qfi: 5 });

    // Non-GBR (QFI 9) consumes tokens: 800B -> Pass
    let v_nongbr1 = enforcer.enforce_packet(9, 800, 0);
    assert_eq!(v_nongbr1, QosVerdict::Pass { qfi: 9 });

    // Non-GBR (QFI 9) exceeds remaining 200B tokens: 500B -> DropAmbrExceeded
    let v_nongbr2 = enforcer.enforce_packet(9, 500, 0);
    assert_eq!(v_nongbr2, QosVerdict::DropAmbrExceeded);
}
