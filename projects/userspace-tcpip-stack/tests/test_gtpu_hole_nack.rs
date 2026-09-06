//! Integration tests for 3GPP TS 29.281 / TS 23.501 5G GTP-U Sequence Hole Filling & Proactive NACK Engine.

use toy_tcpip::gtpu_hole_nack::{GtpuHoleNackEngine, HoleNackVerdict};

#[test]
fn test_gtpu_hole_nack_integration() {
    let mut engine = GtpuHoleNackEngine::new(0x20002, 32, 100_000);

    // Continuous packets 1..5
    for s in 1..=5 {
        let v = engine.ingest_packet(s, (s as u64) * 1000);
        match v {
            HoleNackVerdict::InOrderPacket { teid, seq_number } => {
                assert_eq!(teid, 0x20002);
                assert_eq!(seq_number, s);
            }
            _ => panic!("Expected InOrderPacket"),
        }
    }
    assert_eq!(engine.holes.len(), 0);

    // Jump from 5 to 10 -> missing 6, 7, 8, 9
    let v_gap = engine.ingest_packet(10, 10_000);
    match v_gap {
        HoleNackVerdict::HoleDetectedAndNackGenerated {
            missing_start,
            missing_end,
            nack_report,
            ..
        } => {
            assert_eq!(missing_start, 6);
            assert_eq!(missing_end, 9);
            assert_eq!(nack_report.count, 4);
            assert_eq!(nack_report.bitmask, 0b1111);
        }
        _ => panic!("Expected HoleDetectedAndNackGenerated"),
    }
    assert_eq!(engine.holes.len(), 1);

    // Retransmit NACKs check before timeout -> 0
    assert_eq!(engine.check_retransmit_nacks(50_000).len(), 0);

    // Retransmit NACKs check after timeout (10000 + 100000 = 110000)
    let retrans_nacks = engine.check_retransmit_nacks(120_000);
    assert_eq!(retrans_nacks.len(), 1);
    assert_eq!(retrans_nacks[0].base_missing_seq, 6);
    assert_eq!(retrans_nacks[0].count, 4);

    // Arrive out of order to fill hole: 6, 8, 7, 9
    engine.ingest_packet(6, 130_000);
    assert_eq!(engine.holes[0].start_seq, 7);

    engine.ingest_packet(8, 140_000);
    assert_eq!(engine.holes.len(), 2);

    engine.ingest_packet(7, 150_000);
    assert_eq!(engine.holes.len(), 1);
    assert_eq!(engine.holes[0].start_seq, 9);

    engine.ingest_packet(9, 160_000);
    assert_eq!(engine.holes.len(), 0);
}
