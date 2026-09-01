use toy_tcpip::tsn_cqf_deficit_meter::{
    DeficitMeterVerdict, DeficitStreamProfile, TsnCqfDeficitMeterEngine,
};

#[test]
fn test_tsn_cqf_deficit_meter_lifecycle() {
    let mut engine = TsnCqfDeficitMeterEngine::new(50_000); // 50 µs cycle

    // Stream 1: Flight Control (Quantum = 2000 B, Max Cap = 3000 B)
    engine.register_stream(DeficitStreamProfile::new(1, "Flight-Ctrl", 2000, 3000));

    // 1. Ingest frame within quantum
    let v1 = engine.meter_frame(1, 1200);
    assert_eq!(
        v1,
        DeficitMeterVerdict::Admitted {
            remaining_credit_bytes: 800
        }
    );

    // 2. Ingest frame exceeding remaining credit (900 > 800)
    let v2 = engine.meter_frame(1, 900);
    assert_eq!(
        v2,
        DeficitMeterVerdict::DeficitExceeded {
            required_bytes: 900,
            available_credit_bytes: 800
        }
    );

    // 3. Ingest exact remaining fit (800 == 800)
    let v3 = engine.meter_frame(1, 800);
    assert_eq!(
        v3,
        DeficitMeterVerdict::Admitted {
            remaining_credit_bytes: 0
        }
    );

    // 4. Rotate cycle -> replenished with 2000 B
    engine.rotate_cycle();
    assert_eq!(engine.current_cycle_index, 1);

    let v4 = engine.meter_frame(1, 500);
    assert_eq!(
        v4,
        DeficitMeterVerdict::Admitted {
            remaining_credit_bytes: 1500
        }
    );

    // 5. Carryover into next cycle: 1500 + 2000 = 3500 -> capped at 3000
    engine.rotate_cycle();
    let s = engine.streams.iter().find(|s| s.stream_id == 1).unwrap();
    assert_eq!(s.current_credit_bytes, 3000);
    assert_eq!(s.total_admitted_frames, 3);
    assert_eq!(s.total_dropped_frames, 1);
}
