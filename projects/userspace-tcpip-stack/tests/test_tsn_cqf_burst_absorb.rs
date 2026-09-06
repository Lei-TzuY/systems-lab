// tests/test_tsn_cqf_burst_absorb.rs

use toy_tcpip::tsn_cqf_burst_absorb::{BurstAbsorbVerdict, TsnCqfBurstAbsorbEngine};

#[test]
fn test_tsn_cqf_burst_absorb_integration() {
    let mut engine = TsnCqfBurstAbsorbEngine::new(100_000, 5000);
    engine.register_stream(101, 100_000_000, 1500, 3000, 2500);

    // 1. Initial burst within CBS
    let v1 = engine.ingest_frame(101, 1200, 5_000);
    assert_eq!(
        v1,
        BurstAbsorbVerdict::ConformingIngress {
            stream_id: 101,
            frame_bytes: 1200,
            target_cycle: 1,
            queue_depth_bytes: 1200,
        }
    );

    // 2. Second frame exceeds remaining tokens (300 B left < 800 B) -> absorbed in buffer
    let v2 = engine.ingest_frame(101, 800, 10_000);
    assert_eq!(
        v2,
        BurstAbsorbVerdict::BurstAbsorbedBuffered {
            stream_id: 101,
            frame_bytes: 800,
            scheduled_cycle: 2,
            buffer_occupancy_bytes: 800,
        }
    );

    // 3. Third frame exceeding remaining burst buffer capacity (2500 - 800 = 1700 B < 2000 B) -> drop
    let v3 = engine.ingest_frame(101, 2000, 15_000);
    assert_eq!(
        v3,
        BurstAbsorbVerdict::NonConformingBurstDrop {
            stream_id: 101,
            frame_bytes: 2000,
            reason: "Burst absorption buffer overflow",
        }
    );

    // 4. Tick cycle transition
    let drained = engine.tick_cycle_drain(1);
    assert_eq!(drained, 1200);

    assert_eq!(engine.total_conforming_frames, 1);
    assert_eq!(engine.total_absorbed_frames, 1);
    assert_eq!(engine.total_dropped_frames, 1);
    assert_eq!(engine.total_drained_frames, 1);
}

#[test]
fn test_unregistered_stream_drop() {
    let mut engine = TsnCqfBurstAbsorbEngine::new(100_000, 5000);
    let v = engine.ingest_frame(999, 1000, 1000);
    assert_eq!(
        v,
        BurstAbsorbVerdict::NonConformingBurstDrop {
            stream_id: 999,
            frame_bytes: 1000,
            reason: "Unregistered TSN stream",
        }
    );
    assert_eq!(engine.total_dropped_frames, 1);
}
