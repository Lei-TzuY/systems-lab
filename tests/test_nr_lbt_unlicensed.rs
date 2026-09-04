//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR-U & Listen-Before-Talk (LBT) Engine.

use toy_tcpip::nr_lbt_unlicensed::*;

#[test]
fn test_nr_lbt_energy_detection_threshold() {
    // 1. Standard 20 MHz with 23 dBm EIRP
    let config_20mhz = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw20, 23);
    let thresh_20 = config_20mhz.calculate_threshold_dbm();
    assert_eq!(thresh_20, -72.0);

    // 2. 20 MHz with higher radiated power 26 dBm EIRP -> more sensitive (-75 dBm)
    let config_high_power = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw20, 26);
    let thresh_high_power = config_high_power.calculate_threshold_dbm();
    assert_eq!(thresh_high_power, -75.0);

    // 3. 20 MHz with low radiated power 10 dBm EIRP -> formula gives -59 dBm, but capped at -72 dBm
    let config_low_power = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw20, 10);
    let thresh_low_power = config_low_power.calculate_threshold_dbm();
    assert_eq!(thresh_low_power, -72.0);

    // 4. 40 MHz (+3.01 dB) and 80 MHz (+6.02 dB) scaling
    let config_40mhz = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw40, 26);
    let thresh_40 = config_40mhz.calculate_threshold_dbm();
    // -72 + 3.01 - 3 = -71.99 -> capped at -72.0
    assert_eq!(thresh_40, -72.0);

    let config_40mhz_30dbm = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw40, 30);
    let thresh_40_30dbm = config_40mhz_30dbm.calculate_threshold_dbm();
    // -72 + 3.01 + (23 - 30) = -75.99 dBm
    assert!(thresh_40_30dbm < -75.9 && thresh_40_30dbm > -76.1);
}

#[test]
fn test_nr_lbt_type1_cat4_backoff_and_freeze() {
    let ed_config = EnergyDetectionConfig::new(ChannelBandwidthMhz::Bw20, 23);
    // Fixed seed for deterministic PRNG
    let mut engine = NrLbtEngine::new("gnb-nru-01", ed_config, 123456789);

    // Request Type 1 Cat 4 LBT for CAPC 3 (BestEffort: T_d = 43 us, CW_min = 15)
    let initial_state = engine
        .request_channel_access(ChannelAccessPriorityClass::BestEffort, LbtType::Type1Cat4)
        .unwrap();

    match initial_state {
        LbtState::Deferring {
            capc,
            remaining_defer_us,
            total_defer_us,
        } => {
            assert_eq!(capc, ChannelAccessPriorityClass::BestEffort);
            assert_eq!(remaining_defer_us, 43);
            assert_eq!(total_defer_us, 43);
        }
        other => panic!("Expected Deferring state, got {:?}", other),
    }

    // Step 1: Medium idle (-80 dBm < -72 dBm) for 4 CCA slots (36 us)
    for _ in 0..4 {
        engine.step_cca_slot_9us(-80.0);
    }
    // Remaining defer: 43 - 36 = 7 us
    if let LbtState::Deferring {
        remaining_defer_us, ..
    } = engine.state
    {
        assert_eq!(remaining_defer_us, 7);
    } else {
        panic!("Should still be deferring");
    }

    // Step 2: Medium busy (-60 dBm >= -72 dBm): defer timer must reset to 43 us!
    let reset_state = engine.step_cca_slot_9us(-60.0);
    match reset_state {
        LbtState::Deferring {
            remaining_defer_us,
            total_defer_us,
            ..
        } => {
            assert_eq!(remaining_defer_us, 43);
            assert_eq!(total_defer_us, 43);
        }
        other => panic!("Expected Deferring reset, got {:?}", other),
    }

    // Step 3: Medium continuously idle for 5 slots (45 us > 43 us) -> completes deferral!
    for _ in 0..5 {
        engine.step_cca_slot_9us(-85.0);
    }

    // Now in Backoff state
    let (backoff_n, cw) = match engine.state {
        LbtState::Backoff {
            counter_n,
            current_cw,
            ..
        } => (counter_n, current_cw),
        other => panic!("Expected Backoff state, got {:?}", other),
    };
    assert_eq!(cw, 15);
    assert!(backoff_n <= 15);

    // Step 4: Medium busy causes counter freezing
    let freeze_state = engine.step_cca_slot_9us(-55.0);
    match freeze_state {
        LbtState::Frozen {
            frozen_counter_n, ..
        } => {
            assert_eq!(frozen_counter_n, backoff_n);
        }
        other => panic!("Expected Frozen state, got {:?}", other),
    }
    assert_eq!(engine.metrics.backoff_freeze_events, 1);

    // Step 5: Medium returns idle: resume backoff and count down to 0
    let resumed_state = engine.step_cca_slot_9us(-85.0);
    assert!(matches!(resumed_state, LbtState::Backoff { .. }));

    // Drain remaining counter until channel acquired
    for _ in 0..20 {
        if matches!(engine.state, LbtState::ChannelAcquired { .. }) {
            break;
        }
        engine.step_cca_slot_9us(-85.0);
    }

    match engine.state {
        LbtState::ChannelAcquired {
            capc,
            lbt_type,
            mcot_remaining_us,
            elapsed_tx_us,
        } => {
            assert_eq!(capc, ChannelAccessPriorityClass::BestEffort);
            assert_eq!(lbt_type, LbtType::Type1Cat4);
            assert_eq!(mcot_remaining_us, 8000);
            assert_eq!(elapsed_tx_us, 0);
        }
        other => panic!("Expected ChannelAcquired, got {:?}", other),
    }
    assert_eq!(engine.metrics.successful_acquisitions, 1);
}

#[test]
fn test_nr_lbt_contention_window_adaptation() {
    let ed_config = EnergyDetectionConfig::default();
    let mut engine = NrLbtEngine::new("gnb-nru-cwa", ed_config, 42);

    // Initial CW for CAPC 1 (VoiceSignaling) is CW_min = 3
    assert_eq!(
        *engine
            .current_cws
            .get(&ChannelAccessPriorityClass::VoiceSignaling)
            .unwrap(),
        3
    );

    // Round 1: 10 Transport Blocks with 9 NACKs and 1 ACK (90% NACK >= 80%)
    let mut bad_feedbacks = vec![HarqFeedback::Nack; 9];
    bad_feedbacks.push(HarqFeedback::Ack);

    let cw_doubled_1 = engine.process_harq_reference_subframe(
        ChannelAccessPriorityClass::VoiceSignaling,
        &bad_feedbacks,
    );
    // CW doubles: 2 * (3 + 1) - 1 = 7 (capped at CW_max = 7)
    assert_eq!(cw_doubled_1, 7);
    assert_eq!(engine.metrics.cwa_doubling_events, 1);

    // Round 2 for CAPC 3 (BestEffort, CW_min = 15, CW_max = 63)
    let cw_be_doubled_1 = engine
        .process_harq_reference_subframe(ChannelAccessPriorityClass::BestEffort, &bad_feedbacks);
    // 2 * (15 + 1) - 1 = 31
    assert_eq!(cw_be_doubled_1, 31);

    // Round 3: High collision again for CAPC 3
    let cw_be_doubled_2 = engine
        .process_harq_reference_subframe(ChannelAccessPriorityClass::BestEffort, &bad_feedbacks);
    // 2 * (31 + 1) - 1 = 63 (capped at CW_max = 63)
    assert_eq!(cw_be_doubled_2, 63);

    // Round 4: Clean reception with 8 ACKs, 1 NACK, 1 DTX (20% NACK/DTX < 80%)
    let good_feedbacks = vec![
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Ack,
        HarqFeedback::Nack,
        HarqFeedback::Dtx,
    ];

    let cw_be_reset = engine
        .process_harq_reference_subframe(ChannelAccessPriorityClass::BestEffort, &good_feedbacks);
    // Reset to CW_min = 15
    assert_eq!(cw_be_reset, 15);
    assert_eq!(engine.metrics.cwa_reset_events, 1);
}

#[test]
fn test_nr_lbt_mcot_and_cot_sharing() {
    let ed_config = EnergyDetectionConfig::default();
    let mut engine = NrLbtEngine::new("gnb-nru-cot", ed_config, 999);

    // Acquire channel with CAPC 2 (InteractiveVideo: MCOT = 3 ms / 3000 us)
    engine
        .request_channel_access(
            ChannelAccessPriorityClass::InteractiveVideo,
            LbtType::Type2CCat1,
        )
        .unwrap();

    assert!(matches!(
        engine.state,
        LbtState::ChannelAcquired {
            mcot_remaining_us: 3000,
            ..
        }
    ));

    // Consumes 1000 us of downlink transmission
    let rem1 = engine.consume_transmission_time(1000).unwrap();
    assert_eq!(rem1, 2000);

    // gNodeB creates COT Sharing for UE-007 for 800 us
    let cot_share = engine.create_cot_sharing("ue-007", 800).unwrap();
    assert_eq!(cot_share.target_ue_id, "ue-007");
    assert_eq!(cot_share.shared_duration_us, 800);
    assert_eq!(cot_share.allowed_ul_lbt, LbtType::Type2ACat2);

    // Attempting to share more than remaining 2000 us fails
    let excessive_share = engine.create_cot_sharing("ue-007", 2500);
    assert!(excessive_share.is_err());

    // Consumes another 1000 us
    let rem2 = engine.consume_transmission_time(1000).unwrap();
    assert_eq!(rem2, 1000);

    // Consumes remaining 1000 us -> exhausts MCOT
    let expire_res = engine.consume_transmission_time(1000);
    assert!(expire_res.is_err());
    match engine.state {
        LbtState::CotExpired {
            capc, total_tx_us, ..
        } => {
            assert_eq!(capc, ChannelAccessPriorityClass::InteractiveVideo);
            assert_eq!(total_tx_us, 3000);
        }
        other => panic!("Expected CotExpired, got {:?}", other),
    }

    assert_eq!(engine.metrics.total_tx_microseconds, 3000);

    // Release channel
    engine.release_channel();
    assert_eq!(engine.state, LbtState::Idle);
}

#[test]
fn test_nr_lbt_type2_and_crs_and_wideband_puncturing() {
    let ed_config = EnergyDetectionConfig::default();
    let mut engine = NrLbtEngine::new("gnb-wideband", ed_config, 777);

    // 1. Type 2A Sensing (25 us)
    engine
        .request_channel_access(
            ChannelAccessPriorityClass::VoiceSignaling,
            LbtType::Type2ACat2,
        )
        .unwrap();

    let success = engine.step_type2_sensing(25, -82.0);
    assert!(success);
    assert!(matches!(
        engine.state,
        LbtState::ChannelAcquired {
            lbt_type: LbtType::Type2ACat2,
            ..
        }
    ));

    // 2. Channel Reservation Signal (CRS) generation
    let crs = engine.generate_channel_reservation_signal(143, 8, 30);
    assert_eq!(crs.duration_us, 143);
    assert_eq!(crs.target_slot_index, 8);
    assert_eq!(crs.scs_khz, 30);
    assert_eq!(crs.payload_bytes.len(), 143);

    // 3. Wideband Carrier Sensing & Dynamic Puncturing
    // Case A: Primary channel busy (-65 dBm >= -72 dBm) -> cannot transmit (0 MHz)
    let bw_a = engine.sense_and_puncture_wideband(-65.0, &[-85.0, -85.0, -85.0]);
    assert_eq!(bw_a, 0);

    // Case B: Primary idle, secondary 1 idle -> 40 MHz
    let bw_b = engine.sense_and_puncture_wideband(-80.0, &[-85.0]);
    assert_eq!(bw_b, 40);

    // Case C: Primary idle, all 3 secondaries idle -> 80 MHz
    let bw_c = engine.sense_and_puncture_wideband(-80.0, &[-85.0, -85.0, -85.0]);
    assert_eq!(bw_c, 80);

    // Case D: Primary idle, secondary 1 idle, but secondary 2 is busy (-60 dBm) -> punctures to 40 MHz!
    let bw_d = engine.sense_and_puncture_wideband(-80.0, &[-85.0, -60.0, -85.0]);
    assert_eq!(bw_d, 40);

    // Case E: Primary idle, 7 secondaries idle -> 160 MHz
    let secondary_7 = vec![-85.0; 7];
    let bw_e = engine.sense_and_puncture_wideband(-80.0, &secondary_7);
    assert_eq!(bw_e, 160);
}
