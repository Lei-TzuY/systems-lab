//! Integration test suite for 3GPP Rel-17 5G NR Sidelink DRX & Inter-UE Coordination Engine.

use toy_tcpip::nr_sidelink_drx::{
    CoordinationSchemeType, PartialSensingConfig, ResourceSlotBlock, SidelinkDrxEngine,
    SidelinkDrxProfileConfig, SlDrxCastType,
};

#[test]
fn test_sl_drx_cycle_and_on_duration() {
    let mut engine = SidelinkDrxEngine::new(0x112233);

    let config = SidelinkDrxProfileConfig {
        profile_id: 1,
        cast_type: SlDrxCastType::Unicast,
        peer_ue_id: Some(0x445566),
        on_duration_slots: 5,
        inactivity_slots: 10,
        harq_rtt_slots: 4,
        retransmission_slots: 8,
        cycle_slots: 100,
        start_offset_slots: 10,
        pqi_list: vec![21, 22],
    };

    assert!(engine.add_profile(config).is_ok());

    // Slots 0..9: Sleep
    for slot in 0..10 {
        assert_eq!(engine.current_slot(), slot);
        let active = engine.advance_slot();
        assert!(!active, "Slot {} should be sleep", slot);
    }

    // Slots 10..14: Active (on-duration = 5 slots)
    for slot in 10..15 {
        assert_eq!(engine.current_slot(), slot);
        let active = engine.advance_slot();
        assert!(active, "Slot {} should be active during onDuration", slot);
    }

    // Slots 15..109: Sleep
    for slot in 15..110 {
        assert_eq!(engine.current_slot(), slot);
        let active = engine.advance_slot();
        assert!(!active, "Slot {} should be sleep", slot);
    }

    // Slot 110: Active (next cycle start: 110 % 100 == 10)
    assert_eq!(engine.current_slot(), 110);
    let active = engine.advance_slot();
    assert!(active, "Slot 110 should be active at new cycle start");
}

#[test]
fn test_sl_drx_inactivity_timer_and_harq_retransmission() {
    let mut engine = SidelinkDrxEngine::new(0xAAAAAA);

    let config = SidelinkDrxProfileConfig {
        profile_id: 0,
        cast_type: SlDrxCastType::Unicast,
        peer_ue_id: Some(0xBBBBBB),
        on_duration_slots: 4,
        inactivity_slots: 6,
        harq_rtt_slots: 3,
        retransmission_slots: 5,
        cycle_slots: 200,
        start_offset_slots: 0,
        pqi_list: vec![23],
    };

    assert!(engine.add_profile(config).is_ok());

    // Slot 0..3: on-duration active
    for _ in 0..4 {
        assert!(engine.advance_slot());
    }

    // Slot 4: on-duration expired, should be sleep
    assert!(!engine.advance_slot());

    // Receive SCI from peer at slot 5 -> restarts inactivity timer (6 slots)
    engine.notify_sci_received(0xBBBBBB, true);
    assert!(engine.is_in_active_time());

    // Advance 3 slots under inactivity
    for _ in 0..3 {
        assert!(engine.advance_slot());
    }

    // Receive another SCI -> re-arms inactivity timer to 6 slots
    engine.notify_sci_received(0xBBBBBB, true);
    for _ in 0..5 {
        assert!(engine.advance_slot());
    }

    // Trigger HARQ RTT for process ID 2 (duration = 3 slots)
    engine.trigger_harq_rtt(0, 2);

    // Let inactivity expire (1 slot) while RTT counts down
    engine.advance_slot();

    // Advance 2 more slots for RTT completion
    engine.advance_slot();
    engine.advance_slot();

    // RTT timer expired, retransmission timer now running (5 slots) -> Active
    assert!(engine.is_in_active_time());
    assert!(engine.advance_slot());

    // Local UE receives ACK for HARQ process 2 -> retransmission stops
    engine.notify_harq_ack(0, 2);

    // Should transition to sleep immediately
    assert!(!engine.is_in_active_time());
    assert!(!engine.advance_slot());
}

#[test]
fn test_sl_drx_multi_session_arbitration() {
    let mut engine = SidelinkDrxEngine::new(0x010203);

    // Profile 1: Unicast with Peer A (offset 0, duration 5, cycle 50)
    let p1 = SidelinkDrxProfileConfig {
        profile_id: 1,
        cast_type: SlDrxCastType::Unicast,
        peer_ue_id: Some(0x0A0B0C),
        on_duration_slots: 5,
        inactivity_slots: 5,
        harq_rtt_slots: 2,
        retransmission_slots: 4,
        cycle_slots: 50,
        start_offset_slots: 0,
        pqi_list: vec![21],
    };

    // Profile 2: Groupcast for Platoon (offset 20, duration 5, cycle 50)
    let p2 = SidelinkDrxProfileConfig {
        profile_id: 2,
        cast_type: SlDrxCastType::Groupcast,
        peer_ue_id: None,
        on_duration_slots: 5,
        inactivity_slots: 5,
        harq_rtt_slots: 2,
        retransmission_slots: 4,
        cycle_slots: 50,
        start_offset_slots: 20,
        pqi_list: vec![25],
    };

    assert!(engine.add_profile(p1).is_ok());
    assert!(engine.add_profile(p2).is_ok());

    // Slots 0..4: Profile 1 is active
    for _ in 0..5 {
        assert!(engine.advance_slot());
    }

    // Slots 5..19: Both profiles sleep
    for _ in 5..20 {
        assert!(!engine.advance_slot());
    }

    // Slots 20..24: Profile 2 is active
    for _ in 20..25 {
        assert!(engine.advance_slot());
    }

    // Slot 25: Both sleep
    assert!(!engine.advance_slot());

    // Test Pending Grants override
    engine.set_pending_grants(1);
    assert!(engine.is_in_active_time());
    assert!(engine.advance_slot());

    engine.set_pending_grants(0);
    assert!(!engine.is_in_active_time());
}

#[test]
fn test_inter_ue_coordination_scheme_1_filtering() {
    let mut engine = SidelinkDrxEngine::new(0x100001);

    let pref_block = ResourceSlotBlock {
        slot: 50,
        subchannel_index: 2,
        num_subchannels: 2,
        rsrp_dbm: -105,
        priority: 1,
    };

    let non_pref_block = ResourceSlotBlock {
        slot: 50,
        subchannel_index: 6,
        num_subchannels: 2,
        rsrp_dbm: -60,
        priority: 0,
    };

    // Receive Scheme 1 assistance from peer UE 0x200002
    let msg_pref = engine.generate_iuc_scheme1(
        0x100001,
        CoordinationSchemeType::Scheme1Preferred,
        vec![pref_block],
    );
    let msg_non_pref = engine.generate_iuc_scheme1(
        0x100001,
        CoordinationSchemeType::Scheme1NonPreferred,
        vec![non_pref_block],
    );

    engine.process_iuc_message(msg_pref);
    engine.process_iuc_message(msg_non_pref);

    let candidate_a = ResourceSlotBlock {
        slot: 50,
        subchannel_index: 6,
        num_subchannels: 2,
        rsrp_dbm: -55,
        priority: 2,
    };
    let candidate_b = ResourceSlotBlock {
        slot: 50,
        subchannel_index: 2,
        num_subchannels: 2,
        rsrp_dbm: -102,
        priority: 2,
    };
    let candidate_c = ResourceSlotBlock {
        slot: 50,
        subchannel_index: 10,
        num_subchannels: 2,
        rsrp_dbm: -80,
        priority: 2,
    };

    let candidates = vec![candidate_a, candidate_b, candidate_c];
    let filtered = engine.filter_candidate_resources(&candidates, -90);

    // candidate_a overlaps with non-preferred block and RSRP > -90 dBm -> Excluded!
    assert_eq!(filtered.len(), 2);
    // candidate_b is preferred -> Should be ordered first!
    assert_eq!(filtered[0].subchannel_index, 2);
    // candidate_c is neutral -> Ordered second
    assert_eq!(filtered[1].subchannel_index, 10);
}

#[test]
fn test_inter_ue_coordination_scheme_2_collision_alert() {
    let mut engine = SidelinkDrxEngine::new(0x300001);

    let res_peer_a = ResourceSlotBlock {
        slot: 80,
        subchannel_index: 4,
        num_subchannels: 3, // covers 4, 5, 6
        rsrp_dbm: -75,
        priority: 3,
    };

    let res_peer_b_colliding = ResourceSlotBlock {
        slot: 80,
        subchannel_index: 6,
        num_subchannels: 2, // covers 6, 7 (overlaps at subchannel 6!)
        rsrp_dbm: -72,
        priority: 3,
    };

    let res_peer_b_non_colliding = ResourceSlotBlock {
        slot: 81,
        subchannel_index: 6,
        num_subchannels: 2,
        rsrp_dbm: -72,
        priority: 3,
    };

    // Detect collision
    let alert =
        engine.detect_collision_and_alert(0x300002, &res_peer_a, 0x300003, &res_peer_b_colliding);
    assert!(alert.is_some());
    let msg = alert.unwrap();
    assert_eq!(
        msg.scheme_type,
        CoordinationSchemeType::Scheme2ConflictAlert
    );
    assert_eq!(msg.target_l2_id, 0x300003);
    assert_eq!(msg.resources.len(), 2);
    assert_eq!(engine.telemetry().collisions_avoided_count, 1);

    // Detect non-colliding
    let no_alert = engine.detect_collision_and_alert(
        0x300002,
        &res_peer_a,
        0x300003,
        &res_peer_b_non_colliding,
    );
    assert!(no_alert.is_none());
}

#[test]
fn test_sl_partial_sensing_and_power_saving_metrics() {
    let mut engine = SidelinkDrxEngine::new(0x999999);

    let profile = SidelinkDrxProfileConfig {
        profile_id: 1,
        cast_type: SlDrxCastType::Broadcast,
        peer_ue_id: None,
        on_duration_slots: 5,
        inactivity_slots: 4,
        harq_rtt_slots: 2,
        retransmission_slots: 4,
        cycle_slots: 100,
        start_offset_slots: 50,
        pqi_list: vec![21],
    };
    assert!(engine.add_profile(profile).is_ok());

    // Configure Partial Sensing: 3 contiguous slots before onDuration (slots 47, 48, 49)
    let partial_cfg = PartialSensingConfig {
        periodic_step_slots: 20,
        contiguous_sensing_slots: 3,
        periodic_sensing_depth: 2,
    };
    engine.configure_partial_sensing(partial_cfg);

    // Slots 0..9: Sleep
    for slot in 0..10 {
        assert!(!engine.advance_slot(), "Slot {} should be sleep", slot);
    }
    // Slot 10: Periodic sensing slot (k = 2: 50 - 2*20 = 10)
    assert!(
        engine.is_partial_sensing_slot(),
        "Slot 10 should be periodic sensing"
    );
    assert!(engine.advance_slot(), "Slot 10 should be active");

    // Slots 11..29: Sleep
    for slot in 11..30 {
        assert!(!engine.advance_slot(), "Slot {} should be sleep", slot);
    }
    // Slot 30: Periodic sensing slot (k = 1: 50 - 1*20 = 30)
    assert!(
        engine.is_partial_sensing_slot(),
        "Slot 30 should be periodic sensing"
    );
    assert!(engine.advance_slot(), "Slot 30 should be active");

    // Slots 31..46: Sleep
    for slot in 31..47 {
        assert!(!engine.advance_slot(), "Slot {} should be sleep", slot);
    }

    // Slots 47, 48, 49: Contiguous partial sensing active
    for slot in 47..50 {
        assert!(
            engine.is_partial_sensing_slot(),
            "Slot {} should be contiguous sensing",
            slot
        );
        assert!(engine.advance_slot(), "Slot {} should be active", slot);
    }

    // Slots 50..54: On-duration active
    for slot in 50..55 {
        assert!(
            engine.advance_slot(),
            "Slot {} should be on-duration active",
            slot
        );
    }

    // Run for 10 full cycles (1000 slots total)
    engine.advance_slots(945);

    let telem = engine.telemetry();
    assert_eq!(telem.total_slots, 1000);
    // Overall duty cycle should be significantly under 30%, conserving battery
    assert!(
        telem.duty_cycle_percent < 30.0,
        "Duty cycle was {:.2}%, expected < 30%",
        telem.duty_cycle_percent
    );
    assert!(
        telem.power_saving_percent > 70.0,
        "Power saving was {:.2}%, expected > 70%",
        telem.power_saving_percent
    );
    assert!(telem.num_wakeups >= 10);
}
