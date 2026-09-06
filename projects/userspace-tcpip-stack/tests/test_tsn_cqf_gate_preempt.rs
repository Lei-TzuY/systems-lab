use toy_tcpip::tsn_cqf_gate_preempt::{
    CqfPreemptVerdict, TsnCqfGatePreemptEngine, TsnTrafficClass,
};

#[test]
fn test_tsn_cqf_gate_preempt_lifecycle() {
    let mut engine = TsnCqfGatePreemptEngine::new(200_000, 1); // 200 µs cycle, 1 Gbps

    // 1. Express frame passes unconditionally
    let v1 = engine.evaluate_transmission(TsnTrafficClass::Express, 1500, 190_000, 1);
    assert_eq!(v1, CqfPreemptVerdict::PassExpress { frame_bytes: 1500 });

    // 2. Preemptible frame that fits completely
    let v2 = engine.evaluate_transmission(TsnTrafficClass::Preemptible, 1000, 10_000, 1);
    assert_eq!(
        v2,
        CqfPreemptVerdict::TransmitFullPreemptible {
            frame_bytes: 1000,
            remaining_cycle_ns: 182_000,
        }
    );

    // 3. Preemptible frame at t=195,000 ns (5000 ns remaining = 625 bytes)
    // 1500 bytes frame preempted: 625 bytes first, 875 bytes left
    let v3 = engine.evaluate_transmission(TsnTrafficClass::Preemptible, 1500, 195_000, 1);
    assert_eq!(
        v3,
        CqfPreemptVerdict::PreemptAndFragment {
            first_fragment_bytes: 625,
            remaining_bytes: 875,
            mpacket_seq: 1,
        }
    );

    // 4. Preemptible frame with < 64 bytes window remaining (e.g. 200 ns = 25 bytes)
    let v4 = engine.evaluate_transmission(TsnTrafficClass::Preemptible, 1000, 199_800, 1);
    assert_eq!(
        v4,
        CqfPreemptVerdict::HoldPreemptible {
            hold_duration_ns: 200,
            next_cycle_index: 2,
        }
    );
}
