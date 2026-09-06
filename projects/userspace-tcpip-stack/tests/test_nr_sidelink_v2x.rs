//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR Sidelink / C-V2X PC5 Engine.

use toy_tcpip::nr_sidelink_v2x::*;

#[test]
fn test_nr_sidelink_sci_1a_and_2a_serialization() {
    // 1. 1st-Stage SCI Format 1-A (PSCCH)
    let sci1 = SciFormat1A {
        priority: 2,
        freq_resource_assign: 0x0234,
        time_resource_assign: 0x18,
        reservation_period_ms: 100,
        mcs: 19,
        stage2_format: 0, // SCI format 2-A
    };

    let bytes1 = NrSidelinkEngine::encode_sci_1a(&sci1);
    assert_eq!(bytes1.len(), 6);

    let decoded1 =
        NrSidelinkEngine::decode_sci_1a(&bytes1).expect("SCI format 1-A decoding should succeed");
    assert_eq!(decoded1.priority, 2);
    assert_eq!(decoded1.freq_resource_assign, 0x0234);
    assert_eq!(decoded1.time_resource_assign, 0x18);
    assert_eq!(decoded1.reservation_period_ms, 100);
    assert_eq!(decoded1.mcs, 19);
    assert_eq!(decoded1.stage2_format, 0);

    // 2. 2nd-Stage SCI Format 2-A (PSSCH)
    let sci2 = SciFormat2A {
        source_l2_id: 0x123456,
        dest_l2_id: 0x654321,
        harq_process_id: 7,
        ndi: true,
        rv: 1,
        cast_type: SidelinkCastType::GroupcastOption1Distance,
        harq_feedback_enabled: true,
        comm_range_requirement_m: Some(250),
    };

    let bytes2 = NrSidelinkEngine::encode_sci_2a(&sci2);
    assert_eq!(bytes2.len(), 8);

    let decoded2 =
        NrSidelinkEngine::decode_sci_2a(&bytes2).expect("SCI format 2-A decoding should succeed");
    assert_eq!(decoded2.source_l2_id, 0x123456);
    assert_eq!(decoded2.dest_l2_id, 0x654321);
    assert_eq!(decoded2.harq_process_id, 7);
    assert!(decoded2.ndi);
    assert_eq!(decoded2.rv, 1);
    assert_eq!(
        decoded2.cast_type,
        SidelinkCastType::GroupcastOption1Distance
    );
    assert!(decoded2.harq_feedback_enabled);
    assert_eq!(decoded2.comm_range_requirement_m, Some(250));
}

#[test]
fn test_nr_sidelink_mode2_sensing_and_collision_exclusion() {
    let bwp = SidelinkBandwidthPart {
        num_subchannels: 4,
        subchannel_size_prb: 15,
        start_rb: 0,
        psfch_period_slots: 2,
        min_proc_time_tproc0: 2,
    };

    let mut engine = NrSidelinkEngine::new(0xAAAAAA, bwp);

    // Record high-power periodic reservation from peer UE (SL-RSRP -85 dBm > -110 dBm)
    // Peer reserved subchannel 2 on slot 100 with reservation period 10 slots
    let peer_sci = SciFormat1A {
        priority: 1,
        freq_resource_assign: 2,
        time_resource_assign: 0,
        reservation_period_ms: 10,
        mcs: 15,
        stage2_format: 0,
    };
    engine.record_sensing_reservation(100, 2, peer_sci, -85);

    // Record weak interference peer (SL-RSRP -125 dBm < -110 dBm): should NOT exclude
    let weak_sci = SciFormat1A {
        priority: 1,
        freq_resource_assign: 1,
        time_resource_assign: 0,
        reservation_period_ms: 10,
        mcs: 15,
        stage2_format: 0,
    };
    engine.record_sensing_reservation(100, 1, weak_sci, -125);

    // Current slot is 100. Selection window: T1 = 2, T2 = 10 -> slots 102..110
    // Slot 110 on subchannel 2 collides with periodic reservation (110 - 100 = 10 % 10 == 0)
    for trial_slot in 100..120 {
        let picked = engine
            .select_mode2_resource(trial_slot, 2, 10, 2)
            .expect("Should pick valid candidate resource");

        // The colliding resource (slot 110, subchannel 2) must never be picked when current_slot is 100
        if trial_slot == 100 {
            assert!(!(picked.slot == 110 && picked.subchannel == 2));
        }
    }
}

#[test]
fn test_nr_sidelink_mode2_dynamic_3db_backoff() {
    let bwp = SidelinkBandwidthPart {
        num_subchannels: 2,
        subchannel_size_prb: 20,
        start_rb: 0,
        psfch_period_slots: 1,
        min_proc_time_tproc0: 1,
    };

    let mut engine = NrSidelinkEngine::new(0xBBBBBB, bwp);

    // Inject heavy reservation across subchannels at -105 dBm (above default -110 dBm)
    for subch in 0..2 {
        let heavy_sci = SciFormat1A {
            priority: 0,
            freq_resource_assign: subch as u16,
            time_resource_assign: 0,
            reservation_period_ms: 1, // Every single slot
            mcs: 16,
            stage2_format: 0,
        };
        engine.record_sensing_reservation(50, subch, heavy_sci, -105);
    }

    // With default threshold -110 dBm, 0% resources available (< 20%).
    // The 3 dB dynamic backoff will iterate until threshold reaches -104 dBm,
    // where -105 dBm is below threshold, liberating the resources.
    let selected = engine.select_mode2_resource(50, 1, 5, 1);
    assert!(selected.is_some());
}

#[test]
fn test_nr_sidelink_cbr_cr_congestion_control() {
    let bwp = SidelinkBandwidthPart {
        num_subchannels: 10,
        subchannel_size_prb: 20,
        start_rb: 0,
        psfch_period_slots: 2,
        min_proc_time_tproc0: 2,
    };

    let mut engine = NrSidelinkEngine::new(0xCCCCCC, bwp);

    // 1. CBR Evaluation: 70 busy slots out of 100
    for _ in 0..70 {
        engine.record_s_rssi(-75); // Busy >= -85 dBm
    }
    for _ in 0..30 {
        engine.record_s_rssi(-95); // Idle < -85 dBm
    }

    let cbr = engine.evaluate_cbr(-85);
    assert_eq!(cbr.busy_subchannels, 70);
    assert_eq!(cbr.total_subchannels, 100);
    assert!((cbr.cbr_ratio - 0.70).abs() < 1e-4);

    // 2. CR Evaluation: 1200 subchannels used in 1000-slot window
    // Total possible = 1000 * 10 = 10,000. CR = 1200 / 10000 = 0.12 (12%)
    for slot in 0..120 {
        engine.record_transmission(slot * 4, 10);
    }

    // Priority 7 (lowest priority, CR_limit = 0.10)
    let cr7 = engine.evaluate_cr(300, 7, 0);
    assert!((cr7.cr_ratio - 0.12).abs() < 1e-4);
    assert_eq!(cr7.cr_limit, 0.10);
    assert!(cr7.congestion_mitigated); // Congestion throttle active!

    // Priority 0 (highest emergency priority, CR_limit = 0.80)
    let cr0 = engine.evaluate_cr(300, 0, 0);
    assert!((cr0.cr_ratio - 0.12).abs() < 1e-4);
    assert_eq!(cr0.cr_limit, 0.80);
    assert!(!cr0.congestion_mitigated); // Allowed!
}

#[test]
fn test_nr_sidelink_psfch_distance_based_groupcast() {
    let bwp = SidelinkBandwidthPart {
        num_subchannels: 8,
        subchannel_size_prb: 20,
        start_rb: 0,
        psfch_period_slots: 2,
        min_proc_time_tproc0: 2,
    };

    let engine = NrSidelinkEngine::new(0xDDDDDD, bwp);

    // 1. Broadcast: never sends feedback
    assert_eq!(
        engine.evaluate_psfch_feedback(SidelinkCastType::Broadcast, true, None, None),
        None
    );
    assert_eq!(
        engine.evaluate_psfch_feedback(SidelinkCastType::Broadcast, false, None, None),
        None
    );

    // 2. Unicast: sends ACK on success, NACK on failure
    assert_eq!(
        engine.evaluate_psfch_feedback(SidelinkCastType::Unicast, true, None, None),
        Some(PsfchFeedback::Ack)
    );
    assert_eq!(
        engine.evaluate_psfch_feedback(SidelinkCastType::Unicast, false, None, None),
        Some(PsfchFeedback::Nack)
    );

    // 3. Groupcast Option 1 (Distance-based negative feedback):
    let range_req = Some(200); // 200 meters requirement

    // a. Decode failed, distance 150m (within range): sends NACK!
    assert_eq!(
        engine.evaluate_psfch_feedback(
            SidelinkCastType::GroupcastOption1Distance,
            false,
            Some(150),
            range_req
        ),
        Some(PsfchFeedback::Nack)
    );

    // b. Decode succeeded, distance 150m: no ACK in Option 1!
    assert_eq!(
        engine.evaluate_psfch_feedback(
            SidelinkCastType::GroupcastOption1Distance,
            true,
            Some(150),
            range_req
        ),
        None
    );

    // c. Decode failed, distance 350m (beyond range): suppresses NACK to save channel capacity!
    assert_eq!(
        engine.evaluate_psfch_feedback(
            SidelinkCastType::GroupcastOption1Distance,
            false,
            Some(350),
            range_req
        ),
        None
    );
}
