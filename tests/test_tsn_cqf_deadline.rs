use toy_tcpip::tsn_cqf_deadline::{CqfAdmissionResult, CqfScheduledFrame, TsnCqfDeadlineEngine};

#[test]
fn test_tsn_cqf_deadline_cycle_swap() {
    let mut engine = TsnCqfDeadlineEngine::new(20_000, 15_000, 4000, 4);

    let frame = CqfScheduledFrame {
        stream_id: 10,
        payload_bytes: 1500,
        arrival_time_ns: 12_000,
        max_allowable_delay_ns: 50_000,
    };

    assert_eq!(
        engine.ingest_frame(&frame),
        CqfAdmissionResult::AdmittedCurrentCycle { cycle_id: 0 }
    );
    assert_eq!(engine.total_admitted, 1);

    engine.advance_cycle();
    assert_eq!(engine.current_cycle_bytes, 0);
}
