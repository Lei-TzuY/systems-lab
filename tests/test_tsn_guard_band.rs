use toy_tcpip::tsn_guard_band::{MacMergeState, PriorityType, TsnPreemptionGuardBandEngine};

#[test]
fn test_tsn_guard_band_and_preemption_admission() {
    let mut engine = TsnPreemptionGuardBandEngine::new(10_000_000_000, true); // 10 Gbps

    // Guard band for 64B on 10G is (64 * 8 * 1e9) / 1e10 = 51.2 ns -> 51 ns
    assert_eq!(engine.calculate_guard_band_duration_ns(), 51);

    // Express frame always allowed
    assert!(engine.can_transmit_frame(PriorityType::Express, 1500, 10));

    // Preemptable frame fits
    assert!(engine.can_transmit_frame(PriorityType::Preemptable, 1000, 10_000));

    // Hold state with preemption disabled
    let mut no_preempt = TsnPreemptionGuardBandEngine::new(1_000_000_000, false);
    no_preempt.set_merge_primitive(MacMergeState::Hold);
    assert!(!no_preempt.can_transmit_frame(PriorityType::Preemptable, 100, 50_000));
}
