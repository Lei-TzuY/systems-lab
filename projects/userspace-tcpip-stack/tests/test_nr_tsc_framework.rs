//! Integration Tests for 3GPP Release 18 Smart Grid & Industrial Automation TSN Deterministic QoS Framework.

use toy_tcpip::nr_tsc_framework::*;

#[test]
fn test_ieee8021q_pcp_to_delay_critical_5qi_mapping() {
    let mut mapper = TsnQosMapper::new();

    // Verify standardized default mappings
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp7NetworkControlOrPtp, None),
        DelayCritical5Qi::Qi85
    );
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp6InternetworkControlOrGoose, None),
        DelayCritical5Qi::Qi85
    );
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp5VoiceOrSampledValues, None),
        DelayCritical5Qi::Qi82
    );
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp4Video, None),
        DelayCritical5Qi::Qi83
    );
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp3CriticalApplications, None),
        DelayCritical5Qi::Qi84
    );
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp0BestEffort, None),
        DelayCritical5Qi::Qi80
    );

    // Custom mapping override: VLAN 100 with PCP 5 mapped to 5QI 86 (Motion Control Robotics)
    mapper.set_custom_mapping(5, Some(100), DelayCritical5Qi::Qi86);
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp5VoiceOrSampledValues, Some(100)),
        DelayCritical5Qi::Qi86
    );
    // Other VLANs retain default
    assert_eq!(
        mapper.map_qos(EthernetPcp::Pcp5VoiceOrSampledValues, Some(200)),
        DelayCritical5Qi::Qi82
    );

    // Verify 5QI parameter metrics
    let qi85 = DelayCritical5Qi::Qi85;
    assert_eq!(qi85.five_qi_value(), 85);
    assert_eq!(qi85.packet_delay_budget_us(), 5_000); // 5 ms
    assert_eq!(qi85.max_data_burst_volume(), 255);
    assert!(qi85.packet_error_rate() <= 1e-5);
    assert_eq!(qi85.default_survival_time_us(), 4_000);

    // Calculate bitrate for IEC 61850 SV (4000 Hz = 250 µs periodicity, 256 bytes)
    // GFBR = (256 * 8 * 1_000_000) / 250 = 8,192,000 bps = 8.192 Mbps
    let (gfbr, mfbr) =
        TsnQosMapper::calculate_flow_bitrates(256, 250, 1.25).expect("Bitrate calculation failed");
    assert_eq!(gfbr, 8_192_000);
    assert_eq!(mfbr, 10_240_000); // 1.25 * 8.192 Mbps = 10.24 Mbps
}

#[test]
fn test_tscai_profile_bat_and_5g_nr_slot_timing() {
    let tscai = TscaiProfile::new(
        101,
        TscFlowDirection::Downlink,
        1_000,      // 1 ms = 1000 µs periodicity
        10_000_000, // BAT reference: 10 ms mark
        256,
        4_000, // 4 ms survival time
        50,    // 50 µs arrival window
    )
    .expect("Failed to initialize TSCAI profile");

    // Scheduled BAT calculation across cycles
    assert_eq!(tscai.scheduled_bat_ns(0), 10_000_000);
    assert_eq!(tscai.scheduled_bat_ns(1), 11_000_000); // + 1 ms (1_000_000 ns)
    assert_eq!(tscai.scheduled_bat_ns(5), 15_000_000); // + 5 ms

    // Arrival within 50 µs window (50,000 ns)
    assert!(tscai.is_within_window(11_030_000, 1)); // +30 µs -> OK
    assert!(tscai.is_within_window(10_970_000, 1)); // -30 µs -> OK
    assert!(!tscai.is_within_window(11_060_000, 1)); // +60 µs -> Outside window

    // 5G NR Slot Timing:
    // 30 kHz SCS: 1 subframe (1 ms) has 2 slots (0.5 ms each).
    // Timestamp: 12_750_000 ns = 12.75 ms
    // Radio frame 1 (10 ms), Subframe 2 (2 ms into frame = 12 ms),
    // 750 µs into subframe -> Slot 1 (since slot 0 is 0..500 µs, slot 1 is 500..1000 µs).
    let timing_30k = TscaiProfile::calculate_nr_slot_timing(30, 12_750_000);
    assert_eq!(timing_30k.radio_frame, 1);
    assert_eq!(timing_30k.subframe, 2);
    assert_eq!(timing_30k.slot, 1);
    assert_eq!(timing_30k.scs_khz, 30);
    assert!(timing_30k.symbol <= 13);

    // 60 kHz SCS: 1 subframe (1 ms) has 4 slots (0.25 ms each).
    // 750 µs into subframe -> Slot 3 (0: 0..250, 1: 250..500, 2: 500..750, 3: 750..1000).
    let timing_60k = TscaiProfile::calculate_nr_slot_timing(60, 12_750_000);
    assert_eq!(timing_60k.slot, 3);
}

#[test]
fn test_hold_and_forward_dejitter_buffer_bounds() {
    let target_delay_ns = 3_000_000; // 3 ms target delay
    let mut buffer = HoldAndForwardBuffer::new(101, target_delay_ns, 64);

    // Three packets arrive over 5G radio with varying jitter:
    // Pkt 1: ingress 0 ns, arrives at 1,500,000 ns (delay 1.5 ms)
    // Pkt 2: ingress 1,000,000 ns, arrives at 3,200,000 ns (delay 2.2 ms)
    // Pkt 3: ingress 2,000,000 ns, arrives at 4,800,000 ns (delay 2.8 ms)
    let p1_sched = buffer
        .enqueue(1, 10, 0, 1_500_000, vec![0xAA])
        .expect("Enqueue p1");
    let p2_sched = buffer
        .enqueue(2, 11, 1_000_000, 3_200_000, vec![0xBB])
        .expect("Enqueue p2");
    let p3_sched = buffer
        .enqueue(3, 12, 2_000_000, 4_800_000, vec![0xCC])
        .expect("Enqueue p3");

    assert_eq!(p1_sched, 3_000_000);
    assert_eq!(p2_sched, 4_000_000);
    assert_eq!(p3_sched, 5_000_000);

    // Poll buffer at t = 2,500,000 ns: No packets released yet (p1 scheduled at 3 ms)
    let rel_early = buffer.release_ready(2_500_000);
    assert_eq!(rel_early.len(), 0);

    // Poll buffer at exact cycle tick t = 3_000_000 ns: P1 released with 0 ns jitter!
    let rel_p1 = buffer.release_ready(3_000_000);
    assert_eq!(rel_p1.len(), 1);
    assert_eq!(rel_p1[0].packet_id, 1);
    assert_eq!(rel_p1[0].payload, vec![0xAA]);

    // Poll buffer at t = 4_000_200 ns: P2 released with minimal 200 ns jitter (< 1 µs)
    let rel_p2 = buffer.release_ready(4_000_200);
    assert_eq!(rel_p2.len(), 1);
    assert_eq!(rel_p2[0].packet_id, 2);

    // Verify late arrival detection: packet arriving after its scheduled release boundary
    // Pkt 4: ingress 3,000,000 ns (sched = 6,000,000 ns), arrives at 6,500,000 ns
    let err_late = buffer.enqueue(4, 13, 3_000_000, 6_500_000, vec![0xDD]);
    assert!(matches!(err_late, Err(TscError::PacketTooLate { .. })));

    let metrics = buffer.metrics();
    assert_eq!(metrics.total_enqueued, 3);
    assert_eq!(metrics.total_released, 2);
    assert_eq!(metrics.total_dropped_late, 1);
    assert_eq!(metrics.current_queue_depth, 1); // Pkt 3 still waiting for t = 5 ms
}

#[test]
fn test_industrial_survival_time_transient_loss_and_recovery() {
    let periodicity_us = 1_000; // 1 ms cycle
    let survival_time_us = 4_000; // 4 ms survival budget (tolerate up to 3-4 lost packets)
    let mut sm = SurvivalTimeStateMachine::new(101, periodicity_us, survival_time_us);

    // Packet 0 arrives at t = 1,000 µs
    let tr0 = sm.on_packet_arrival(1_000);
    assert_eq!(tr0, SurvivalTimeTransition::NoChange);
    assert_eq!(sm.state, SurvivalTimeState::Normal);
    assert!(!sm.is_ran_priority_boost_active());

    // Packet expected at t = 2,000 µs is dropped.
    // Cycle tick at t = 2,200 µs detects drop!
    let tr1 = sm.on_cycle_tick(2_200);
    assert!(matches!(
        tr1,
        SurvivalTimeTransition::EnteredSurvivalTime {
            consecutive_losses: 1,
            ..
        }
    ));
    assert!(
        sm.is_ran_priority_boost_active(),
        "5GS RAN priority boost should be requested"
    );

    // Second packet drop: cycle tick at t = 3,200 µs
    let tr2 = sm.on_cycle_tick(3_200);
    assert!(matches!(
        tr2,
        SurvivalTimeTransition::EnteredSurvivalTime {
            consecutive_losses: 2,
            ..
        }
    ));

    // Packet arrives at t = 3,500 µs BEFORE survival time (4,000 µs) expires!
    let tr_rec = sm.on_packet_arrival(3_500);
    assert!(matches!(
        tr_rec,
        SurvivalTimeTransition::RecoveredToNormal {
            missed_packets: 2,
            ..
        }
    ));
    assert_eq!(sm.total_recoveries, 1);

    // Next regular packet arrives at t = 4,500 µs, completing return to Normal
    let tr_norm = sm.on_packet_arrival(4_500);
    assert_eq!(tr_norm, SurvivalTimeTransition::NoChange);
    assert_eq!(sm.state, SurvivalTimeState::Normal);
    assert!(!sm.is_ran_priority_boost_active());
}

#[test]
fn test_survival_time_expiration_triggers_application_trip() {
    let periodicity_us = 1_000; // 1 ms
    let survival_time_us = 3_000; // 3 ms survival time limit
    let mut sm = SurvivalTimeStateMachine::new(102, periodicity_us, survival_time_us);

    // Initial packet arrives at t = 1,000 µs
    sm.on_packet_arrival(1_000);

    // Consecutive packet drops: t = 2,000 µs missed
    sm.on_cycle_tick(2_100);
    assert!(matches!(
        sm.state,
        SurvivalTimeState::SurvivalTimeActive { .. }
    ));

    // t = 3,000 µs missed
    sm.on_cycle_tick(3_100);

    // t = 4,000 µs missed -> at t = 5,100 µs, elapsed time (5,100 - 2,000 = 3,100 µs) > 3,000 µs!
    let tr_trip = sm.on_cycle_tick(5_100);
    assert!(matches!(tr_trip, SurvivalTimeTransition::Tripped { .. }));
    assert!(matches!(
        sm.state,
        SurvivalTimeState::ApplicationTrip { .. }
    ));
    assert_eq!(sm.total_trips, 1);

    // In an industrial safety circuit, new packets cannot clear the trip automatically
    let tr_post = sm.on_packet_arrival(6_000);
    assert_eq!(tr_post, SurvivalTimeTransition::NoChange);
    assert!(matches!(
        sm.state,
        SurvivalTimeState::ApplicationTrip { .. }
    ));

    // Explicit operator / system reset restores Normal state
    sm.reset_trip();
    assert_eq!(sm.state, SurvivalTimeState::Normal);
}

#[test]
fn test_ieee8021cb_frer_seamless_redundancy_and_deduplication() {
    let mut frer = FrerDeduplicator::new(101, 64);

    // Ingress sequence numbers over dual paths (Path A & Path B):
    // Seq 1 arrives from Path A
    let res1_a = frer.process_sequence(1);
    assert_eq!(res1_a, FrerResult::Accepted { sequence: 1 });

    // Duplicate Seq 1 arrives from Path B
    let res1_b = frer.process_sequence(1);
    assert_eq!(res1_b, FrerResult::DuplicateDiscarded { sequence: 1 });

    // Seq 2 arrives from Path A
    let res2_a = frer.process_sequence(2);
    assert_eq!(res2_a, FrerResult::Accepted { sequence: 2 });

    // Seq 3 arrives from Path A
    let res3_a = frer.process_sequence(3);
    assert_eq!(res3_a, FrerResult::Accepted { sequence: 3 });

    // Delayed Seq 2 arrives from Path B
    let res2_b = frer.process_sequence(2);
    assert_eq!(res2_b, FrerResult::DuplicateDiscarded { sequence: 2 });

    // Out-of-order arrival: Seq 5 arrives before Seq 4
    let res5 = frer.process_sequence(5);
    assert_eq!(res5, FrerResult::Accepted { sequence: 5 });

    // Now Seq 4 arrives: valid out-of-order within sliding history window
    let res4 = frer.process_sequence(4);
    assert!(matches!(
        res4,
        FrerResult::OutOfOrderAccepted { sequence: 4, .. }
    ));

    // Seq 4 arriving again is discarded
    let res4_dup = frer.process_sequence(4);
    assert_eq!(res4_dup, FrerResult::DuplicateDiscarded { sequence: 4 });

    assert_eq!(frer.total_received, 8);
    assert_eq!(frer.total_accepted, 5); // 1, 2, 3, 5, 4 (all 5 accepted once)
    assert_eq!(frer.total_duplicates, 3); // 1_dup, 2_dup, 4_dup
    assert_eq!(frer.total_out_of_order, 1); // 4 was out-of-order
}

#[test]
fn test_tsc_engine_end_to_end_orchestration() {
    let bridge_id = [0x80, 0x00, 0x02, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
    let translator = TscTranslatorType::DsTt {
        ds_tt_port_id: 1,
        ue_id: 0x998877665544,
    };

    let mut engine = TscEngine::new(translator, bridge_id);

    // Register IEC 61850 GOOSE teleprotection stream (stream 101, PCP 6)
    let goose_profile = TscaiProfile::new(
        101,
        TscFlowDirection::Downlink,
        1_000,      // 1 ms periodicity
        10_000_000, // BAT reference
        256,
        4_000, // 4 ms survival time
        50,
    )
    .expect("GOOSE profile");

    engine
        .register_stream(
            goose_profile,
            EthernetPcp::Pcp6InternetworkControlOrGoose,
            None,
            2_000_000, // 2 ms target delay
            64,
            64,
        )
        .expect("Register stream 101");

    // Ingress frame processing: verifies QoS mapping to 5QI 85
    let ingress_res = engine
        .process_ingress(101, 1, 10_000_000)
        .expect("Process ingress frame");
    assert_eq!(ingress_res.five_qi, DelayCritical5Qi::Qi85);
    assert_eq!(ingress_res.pdb_us, 5_000);
    assert_eq!(
        ingress_res.frer_result,
        FrerResult::Accepted { sequence: 1 }
    );

    // Egress frame arrival at egress translator (Path A arrives at t = 11.2 ms = 1.2 ms radio delay)
    let egress_res_a = engine
        .process_egress_arrival(101, 1, vec![0x11, 0x22], 10_000_000, 11_200_000)
        .expect("Process egress frame Path A");
    assert_eq!(egress_res_a.scheduled_release_ns, 12_000_000); // 10 ms + 2 ms target delay

    // Egress frame arrival (Path B arrives at t = 11.5 ms): FRER deduplication discards it!
    let egress_res_b = engine
        .process_egress_arrival(101, 1, vec![0x11, 0x22], 10_000_000, 11_500_000)
        .expect("Process egress frame Path B");
    assert!(matches!(
        egress_res_b.frer_result,
        FrerResult::DuplicateDiscarded { sequence: 1 }
    ));

    // Poll egress release at t = 11.8 ms: packet not released yet
    let released_early = engine
        .release_ready_packets(101, 11_800_000)
        .expect("Release early");
    assert_eq!(released_early.len(), 0);

    // Poll egress release at scheduled release boundary t = 12.0 ms: packet released!
    let released_on_time = engine
        .release_ready_packets(101, 12_000_000)
        .expect("Release on time");
    assert_eq!(released_on_time.len(), 1);
    assert_eq!(released_on_time[0].sequence_num, 1);
    assert_eq!(released_on_time[0].payload, vec![0x11, 0x22]);

    // Check bridge delay reporting
    let delay_report = engine
        .report_bridge_delays(101)
        .expect("Bridge delay report");
    assert_eq!(delay_report.ingress_port, 1);
    assert_eq!(delay_report.nominal_bridge_delay_ns, 2_000_000);

    // Check telemetry
    let telem = engine.get_stream_telemetry(101).expect("Stream telemetry");
    assert_eq!(telem.five_qi, DelayCritical5Qi::Qi85);
    assert_eq!(telem.dejitter_metrics.total_released, 1);
    assert_eq!(telem.dejitter_metrics.avg_jitter_abs_ns, 0.0); // exact release!
}
