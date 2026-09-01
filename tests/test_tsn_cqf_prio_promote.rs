use toy_tcpip::tsn_cqf_prio_promote::{
    PrioPromoteProfile, PriorityPromoteVerdict, TsnCqfPrioPromoteEngine,
};

#[test]
fn test_tsn_cqf_prio_promote_lifecycle() {
    let mut engine = TsnCqfPrioPromoteEngine::new(2);

    // Stream 1: Mission-Critical Guidance (Base PCP 4, Promoted 7, Fallback 2, Promote at 30 µs, Drop at 60 µs)
    engine.register_stream(PrioPromoteProfile::new(
        1,
        "Guidance-Stream",
        4,
        7,
        2,
        30_000,
        60_000,
    ));

    // 1. Normal age (15 µs < 30 µs) -> Normal PCP 4
    let v1 = engine.evaluate_frame(1, 15_000);
    assert_eq!(
        v1,
        PriorityPromoteVerdict::Normal {
            pcp: 4,
            age_ns: 15_000
        }
    );

    // 2. High age (35 µs >= 30 µs) -> Promoted to PCP 7
    let v2 = engine.evaluate_frame(1, 35_000);
    assert_eq!(
        v2,
        PriorityPromoteVerdict::Promoted {
            original_pcp: 4,
            promoted_pcp: 7,
            age_ns: 35_000
        }
    );

    // 3. Second promotion -> high-priority buffer reaches capacity 2
    let v3 = engine.evaluate_frame(1, 40_000);
    assert_eq!(
        v3,
        PriorityPromoteVerdict::Promoted {
            original_pcp: 4,
            promoted_pcp: 7,
            age_ns: 40_000
        }
    );

    // 4. Third promotion exceeds capacity -> PreemptionFallback to PCP 2
    let v4 = engine.evaluate_frame(1, 45_000);
    assert_eq!(
        v4,
        PriorityPromoteVerdict::PreemptionFallback { fallback_pcp: 2 }
    );

    // 5. Exceeded deadline (70 µs > 60 µs) -> DeadlineMissDrop
    let v5 = engine.evaluate_frame(1, 70_000);
    assert_eq!(
        v5,
        PriorityPromoteVerdict::DeadlineMissDrop {
            age_ns: 70_000,
            max_allowed_ns: 60_000
        }
    );

    // 6. Reset cycle clears active high prio count
    engine.reset_cycle();
    assert_eq!(engine.high_prio_current_count, 0);
}
