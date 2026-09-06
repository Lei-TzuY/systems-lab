//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR DRX & Power Saving Engine.

use toy_tcpip::nr_drx_engine::*;

#[test]
fn test_nr_drx_long_cycle_on_duration_timing() {
    let config = DrxConfig {
        drx_on_duration_slots: 10,
        drx_inactivity_slots: 0,
        drx_harq_rtt_timer_dl: 8,
        drx_harq_rtt_timer_ul: 8,
        drx_retransmission_timer_dl: 16,
        drx_retransmission_timer_ul: 16,
        drx_long_cycle_slots: 100,
        drx_start_offset_slots: 0,
        short_drx: None,
        dci_2_6_wus_enabled: false,
    };

    let mut engine = NrDrxEngine::new(config, 10); // 10 slots per frame (15 kHz SCS)

    // Run for 2 full Long DRX cycles (200 slots)
    let mut on_duration_count = 0;
    let mut sleep_count = 0;

    for slot_idx in 0..200 {
        let activity = engine.step_slot();
        match activity {
            DrxActivity::ActiveTime(reason) => {
                assert_eq!(reason, ActiveReason::OnDurationTimer);
                on_duration_count += 1;
                // Verify active slots occur at [0..10] and [100..110]
                let cycle_slot = slot_idx % 100;
                assert!(
                    cycle_slot < 10,
                    "Active time must only occur during onDuration (first 10 slots of cycle)"
                );
            }
            DrxActivity::Sleep => {
                sleep_count += 1;
                let cycle_slot = slot_idx % 100;
                assert!(cycle_slot >= 10, "Sleep must occur outside onDuration");
            }
        }
    }

    assert_eq!(
        on_duration_count, 20,
        "Must have 20 active slots (10 per cycle)"
    );
    assert_eq!(sleep_count, 180, "Must have 180 sleep slots (90 per cycle)");

    // Duty cycle = 20 / 200 = 10%
    let duty_cycle = engine.active_duty_cycle();
    assert!((duty_cycle - 0.10).abs() < 1e-6);

    // Energy savings = 90%
    let savings = engine.energy_savings_percentage();
    assert!((savings - 90.0).abs() < 1e-6);
}

#[test]
fn test_nr_drx_inactivity_timer_and_traffic_burst() {
    let config = DrxConfig {
        drx_on_duration_slots: 5,
        drx_inactivity_slots: 20,
        drx_harq_rtt_timer_dl: 8,
        drx_harq_rtt_timer_ul: 8,
        drx_retransmission_timer_dl: 16,
        drx_retransmission_timer_ul: 16,
        drx_long_cycle_slots: 100,
        drx_start_offset_slots: 0,
        short_drx: None,
        dci_2_6_wus_enabled: false,
    };

    let mut engine = NrDrxEngine::new(config, 10);

    // Slot 0, 1, 2: Active under onDuration
    for _ in 0..3 {
        let act = engine.step_slot();
        assert_eq!(act, DrxActivity::ActiveTime(ActiveReason::OnDurationTimer));
    }

    // At slot 3: Data packet arrives, starts inactivity timer (20 slots)
    engine.notify_new_transmission(true, 0);
    assert_eq!(engine.inactivity_timer, 20);

    // Slots 3, 4: Active (both onDuration and inactivity running)
    let act3 = engine.step_slot();
    assert!(act3.is_active());
    let act4 = engine.step_slot();
    assert!(act4.is_active());

    // Slots 5..15: onDuration has expired (was 5 slots), but InactivityTimer keeps UE in ActiveTime!
    for _ in 5..15 {
        let act = engine.step_slot();
        assert_eq!(act, DrxActivity::ActiveTime(ActiveReason::InactivityTimer));
    }

    // At slot 15: Second packet arrives, restarting InactivityTimer back to 20!
    engine.notify_new_transmission(false, 1);
    assert_eq!(engine.inactivity_timer, 20);

    // Let the 20 inactivity slots expire without further traffic
    for _ in 0..20 {
        let act = engine.step_slot();
        assert_eq!(act, DrxActivity::ActiveTime(ActiveReason::InactivityTimer));
    }

    // InactivityTimer is now 0 -> UE immediately enters Sleep!
    let sleep_act = engine.step_slot();
    assert_eq!(sleep_act, DrxActivity::Sleep);
}

#[test]
fn test_nr_drx_short_to_long_cycle_transition() {
    let short_config = ShortDrxConfig {
        drx_short_cycle_slots: 20,
        drx_short_cycle_timer_count: 3, // 3 short cycles before long DRX
    };

    let config = DrxConfig {
        drx_on_duration_slots: 4,
        drx_inactivity_slots: 0,
        drx_harq_rtt_timer_dl: 8,
        drx_harq_rtt_timer_ul: 8,
        drx_retransmission_timer_dl: 16,
        drx_retransmission_timer_ul: 16,
        drx_long_cycle_slots: 80,
        drx_start_offset_slots: 0,
        short_drx: Some(short_config),
        dci_2_6_wus_enabled: false,
    };

    let mut engine = NrDrxEngine::new(config, 10);
    // Explicitly start in Short DRX mode with 3 cycles
    engine.current_cycle_mode = DrxCycleMode::ShortDrx;
    engine.short_cycle_cycles_left = 3;

    // Cycle 1: Slots 0..20 (On-duration: 0..4)
    for slot in 0..20 {
        let act = engine.step_slot();
        if slot < 4 {
            assert!(act.is_active());
        } else {
            assert_eq!(act, DrxActivity::Sleep);
        }
    }
    assert_eq!(engine.short_cycle_cycles_left, 2);
    assert_eq!(engine.current_cycle_mode, DrxCycleMode::ShortDrx);

    // Cycle 2: Slots 20..40 (On-duration: 20..24)
    for slot in 20..40 {
        let act = engine.step_slot();
        if slot < 24 {
            assert!(act.is_active());
        } else {
            assert_eq!(act, DrxActivity::Sleep);
        }
    }
    assert_eq!(engine.short_cycle_cycles_left, 1);
    assert_eq!(engine.current_cycle_mode, DrxCycleMode::ShortDrx);

    // Cycle 3: Slots 40..60 (On-duration: 40..44)
    for slot in 40..60 {
        let act = engine.step_slot();
        if slot < 44 {
            assert!(act.is_active());
        } else {
            assert_eq!(act, DrxActivity::Sleep);
        }
    }
    // Short DRX cycles exhausted -> transitioned to Long DRX!
    assert_eq!(engine.short_cycle_cycles_left, 0);
    assert_eq!(engine.current_cycle_mode, DrxCycleMode::LongDrx);

    // Slot 60 would have been a short cycle start, but now we are in Long DRX (cycle = 80 slots)!
    // Therefore slot 60 must SLEEP!
    let act60 = engine.step_slot();
    assert_eq!(
        act60,
        DrxActivity::Sleep,
        "Slot 60 must sleep because UE transitioned to Long DRX"
    );

    // Advance to next Long DRX cycle boundary at slot 80
    for _ in 61..80 {
        let act = engine.step_slot();
        assert_eq!(act, DrxActivity::Sleep);
    }

    // At slot 80: Long DRX On-Duration begins!
    let act80 = engine.step_slot();
    assert!(
        act80.is_active(),
        "Slot 80 must be active as Long DRX cycle start"
    );
}

#[test]
fn test_nr_drx_mac_ce_commands() {
    let short_config = ShortDrxConfig {
        drx_short_cycle_slots: 20,
        drx_short_cycle_timer_count: 4,
    };

    let config = DrxConfig {
        drx_on_duration_slots: 15,
        drx_inactivity_slots: 30,
        drx_harq_rtt_timer_dl: 8,
        drx_harq_rtt_timer_ul: 8,
        drx_retransmission_timer_dl: 16,
        drx_retransmission_timer_ul: 16,
        drx_long_cycle_slots: 160,
        drx_start_offset_slots: 0,
        short_drx: Some(short_config),
        dci_2_6_wus_enabled: false,
    };

    let mut engine = NrDrxEngine::new(config, 10);

    // Step into active time
    engine.step_slot();
    engine.notify_new_transmission(true, 0);
    assert!(engine.on_duration_timer > 0);
    assert!(engine.inactivity_timer > 0);

    // 1. gNodeB transmits DRX Command MAC CE (LCID 60)
    engine.process_mac_ce(DrxMacCe::DrxCommand);

    // Verification: onDuration and inactivity stopped immediately
    assert_eq!(engine.on_duration_timer, 0);
    assert_eq!(engine.inactivity_timer, 0);
    // Short DRX initiated
    assert_eq!(engine.current_cycle_mode, DrxCycleMode::ShortDrx);
    assert_eq!(engine.short_cycle_cycles_left, 4);

    // 2. gNodeB transmits Long DRX Command MAC CE (LCID 61)
    engine.process_mac_ce(DrxMacCe::LongDrxCommand);

    // Verification: short cycle aborted, immediate Long DRX mode forced
    assert_eq!(engine.short_cycle_cycles_left, 0);
    assert_eq!(engine.current_cycle_mode, DrxCycleMode::LongDrx);
}

#[test]
fn test_nr_drx_harq_retransmission_and_wus() {
    let config = DrxConfig {
        drx_on_duration_slots: 4,
        drx_inactivity_slots: 0,
        drx_harq_rtt_timer_dl: 5,
        drx_harq_rtt_timer_ul: 5,
        drx_retransmission_timer_dl: 8,
        drx_retransmission_timer_ul: 8,
        drx_long_cycle_slots: 100,
        drx_start_offset_slots: 0,
        short_drx: None,
        dci_2_6_wus_enabled: true,
    };

    let mut engine = NrDrxEngine::new(config, 10);

    // 1. DL HARQ NACK and Retransmission Timer in Active Time
    engine.step_slot(); // onDuration begins
    engine.notify_new_transmission(true, 1);
    engine.notify_harq_nack(true, 1);

    // Progress 4 slots: onDuration ends
    for _ in 0..4 {
        engine.step_slot();
    }
    assert_eq!(engine.on_duration_timer, 0);

    // Step 2 more slots for HARQ RTT timer to expire (5 slots total)
    engine.step_slot();
    engine.step_slot();

    // RTT expired on NACK: drx-RetransmissionTimerDL is now active!
    let act_retrans = engine.step_slot();
    assert_eq!(
        act_retrans,
        DrxActivity::ActiveTime(ActiveReason::HarqRetransmissionDL),
        "Must be in Active Time due to DL HARQ retransmission timer"
    );

    // 2. Rel-17 DCI format 2_6 Wake-Up Signal (WUS) skipping On-Duration
    // Fast forward to slot 95 (approaching next Long DRX cycle at slot 100)
    while engine.current_slot != 9 || engine.current_sfn != 9 {
        engine.step_slot();
    }

    // WUS received indicating wake_up = false (no traffic scheduled)
    engine.set_wus_indication(false);

    // Step across cycle start (slot 100)
    let act100 = engine.step_slot();
    assert_eq!(
        act100,
        DrxActivity::Sleep,
        "WUS indication false must cause UE to skip on-duration and remain in Sleep!"
    );
    assert_eq!(engine.on_duration_timer, 0);
}
