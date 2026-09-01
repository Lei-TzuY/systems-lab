use toy_tcpip::gtpu_gap_retransmit::{GapAction, GtpuGapRetransmitEngine};

#[test]
fn test_gtpu_gap_retransmit_lifecycle() {
    let mut engine = GtpuGapRetransmitEngine::new(0x7788, 100, 3);

    // 1. Packets 100, 101 arrive in order
    assert_eq!(engine.inspect_sequence(100), GapAction::Contiguous);
    assert_eq!(engine.inspect_sequence(101), GapAction::Contiguous);

    // 2. Packet 104 arrives (Packets 102, 103 missing)
    let _a104 = engine.inspect_sequence(104);
    assert_eq!(engine.holes.len(), 2);
    assert_eq!(engine.holes[0].missing_seq, 102);
    assert_eq!(engine.holes[1].missing_seq, 103);

    // 3. Packet 105 arrives (OOO count = 2)
    assert_eq!(engine.inspect_sequence(105), GapAction::Contiguous);

    // 4. Packet 106 arrives (OOO count = 3 >= threshold) -> Fast Retransmit Trigger
    let a106 = engine.inspect_sequence(106);
    assert_eq!(
        a106,
        GapAction::TriggerFastRetransmit {
            missing_seq: 102,
            ooo_count: 3,
        }
    );

    // 5. Retransmit of packet 102 arrives -> Hole Repaired
    assert_eq!(
        engine.inspect_sequence(102),
        GapAction::HoleRepaired { repaired_seq: 102 }
    );
    assert_eq!(engine.holes.len(), 1);
    assert_eq!(engine.holes[0].missing_seq, 103);
}
