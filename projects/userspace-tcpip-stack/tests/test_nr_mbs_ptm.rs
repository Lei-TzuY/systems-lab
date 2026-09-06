//! Integration tests for 3GPP Rel-17 5G NR Multicast and Broadcast Services (MBS) PTM Engine.

use toy_tcpip::nr_mbs_ptm::*;

#[test]
fn test_mrb_pdcp_routing_and_split_mrb() {
    let tmgi = MbsTmgi::new([0x11, 0x22, 0x33], "208", "95");
    assert_eq!(tmgi.to_string_id(), "112233-208-95");

    // 1. PTM-Only Bearer
    let ptm_config = MrbConfig {
        mrb_id: 1,
        tmgi: tmgi.clone(),
        delivery_mode: MbsDeliveryMode::PtmOnly,
        sn_size: MrbPdcpSnSize::Len12Bits,
        split_policy: SplitMrbRoutingPolicy::PrimaryPtm,
        g_rnti: 0x1001,
        mtch_lcid: 5,
    };
    let mut ptm_bearer = MrbEntity::new(ptm_config);
    let pdus = ptm_bearer.transmit_sdu(b"MBS Broadcast Video Frame 1");
    assert_eq!(pdus.len(), 1);
    assert_eq!(pdus[0].sn, 0);
    assert_eq!(
        pdus[0].leg,
        MbsDeliveryLeg::PtmMtch {
            g_rnti: 0x1001,
            lcid: 5
        }
    );
    assert_eq!(ptm_bearer.ptm_tx_queue.len(), 1);

    // 2. PTP-Only Bearer with two connected UEs
    let ptp_config = MrbConfig {
        mrb_id: 2,
        tmgi: tmgi.clone(),
        delivery_mode: MbsDeliveryMode::PtpOnly,
        sn_size: MrbPdcpSnSize::Len18Bits,
        split_policy: SplitMrbRoutingPolicy::PrimaryPtm,
        g_rnti: 0x1002,
        mtch_lcid: 6,
    };
    let mut ptp_bearer = MrbEntity::new(ptp_config);
    ptp_bearer.add_ptp_subscriber(0x2001, 10);
    ptp_bearer.add_ptp_subscriber(0x2002, 11);

    let ptp_pdus = ptp_bearer.transmit_sdu(b"Unicast Multicast Audio Stream");
    assert_eq!(ptp_pdus.len(), 2);
    assert_eq!(ptp_bearer.ptp_tx_queues.get(&0x2001).unwrap().len(), 1);
    assert_eq!(ptp_bearer.ptp_tx_queues.get(&0x2002).unwrap().len(), 1);

    // 3. Split MRB with Packet Duplication
    let split_config = MrbConfig {
        mrb_id: 3,
        tmgi: tmgi.clone(),
        delivery_mode: MbsDeliveryMode::SplitMrb,
        sn_size: MrbPdcpSnSize::Len12Bits,
        split_policy: SplitMrbRoutingPolicy::Duplication,
        g_rnti: 0x1003,
        mtch_lcid: 7,
    };
    let mut split_bearer = MrbEntity::new(split_config);
    split_bearer.add_ptp_subscriber(0x2001, 10);

    let dup_pdus = split_bearer.transmit_sdu(b"Mission-Critical Push-to-Talk (MCPTT) Alert");
    // Should produce 1 for PTM + 1 for PTP subscriber
    assert_eq!(dup_pdus.len(), 2);
    assert_eq!(split_bearer.stats_duplicated_packets, 1);
    assert_eq!(split_bearer.ptm_tx_queue.len(), 1);
    assert_eq!(split_bearer.ptp_tx_queues.get(&0x2001).unwrap().len(), 1);

    // 4. Test Receiver Reordering and Deduplication
    let mut rx_entity = MrbEntity::new(MrbConfig {
        mrb_id: 4,
        tmgi,
        delivery_mode: MbsDeliveryMode::PtmOnly,
        sn_size: MrbPdcpSnSize::Len12Bits,
        split_policy: SplitMrbRoutingPolicy::PrimaryPtm,
        g_rnti: 0x1001,
        mtch_lcid: 5,
    });

    let sdu_data = b"Sequential SDU Content";
    let tx_pdus = ptm_bearer.transmit_sdu(sdu_data); // SN = 1
    let raw_pdu = &tx_pdus[0].payload;

    // Receive in-order
    let rx_result = rx_entity.receive_pdu(raw_pdu).unwrap();
    // rx_entity expects SN 0 first, so SN 1 is buffered as out-of-order
    assert_eq!(rx_result, None);

    // Receive SN 0 (which was the first PDU from step 1)
    let rx_sn0 = rx_entity.receive_pdu(&pdus[0].payload).unwrap();
    assert_eq!(rx_sn0, Some(b"MBS Broadcast Video Frame 1".to_vec()));
}

#[test]
fn test_ptm_harq_dual_schemes() {
    // -----------------------------------------------------------------------
    // Option 1: Individual ACK/NACK (Dedicated PUCCH per UE)
    // -----------------------------------------------------------------------
    let mut harq_opt1 = PtmHarqManager::new(PtmHarqScheme::Option1IndividualAckNack, 4, 0x1001, 3);
    let ues = vec![0x2001, 0x2002, 0x2003];

    // Load TB into process 0
    harq_opt1.processes[0].load_tb(b"TransportBlock_A".to_vec());
    assert!(harq_opt1.processes[0].is_active);
    assert_eq!(harq_opt1.processes[0].rv, 0);

    // UE 1 reports ACK -> Still waiting for UE 2 and 3
    let status_ue1 = harq_opt1.handle_option1_feedback(0, 0x2001, true, &ues);
    assert_eq!(status_ue1, None);

    // UE 2 reports NACK -> Immediate retransmit needed
    let status_ue2 = harq_opt1.handle_option1_feedback(0, 0x2002, false, &ues);
    assert_eq!(status_ue2, Some(false));

    // Advance HARQ: RV transitions 0 -> 2
    let (retransmit, next_rv) = harq_opt1.evaluate_and_advance(0, false);
    assert!(retransmit);
    assert_eq!(next_rv, 2);
    assert_eq!(harq_opt1.stats_retransmissions, 1);

    // Retransmit succeeds: all UEs report ACK on RV 2
    harq_opt1.handle_option1_feedback(0, 0x2001, true, &ues);
    harq_opt1.handle_option1_feedback(0, 0x2002, true, &ues);
    let status_all = harq_opt1.handle_option1_feedback(0, 0x2003, true, &ues);
    assert_eq!(status_all, Some(true));

    let (retransmit, _) = harq_opt1.evaluate_and_advance(0, true);
    assert!(!retransmit);
    assert!(!harq_opt1.processes[0].is_active);
    assert_eq!(harq_opt1.stats_successful_tbs, 1);

    // -----------------------------------------------------------------------
    // Option 2: Shared NACK-Only (Common PUCCH with energy threshold)
    // -----------------------------------------------------------------------
    let mut harq_opt2 = PtmHarqManager::new(PtmHarqScheme::Option2SharedNackOnly, 4, 0x1002, 2);
    harq_opt2.shared_pucch_energy_threshold = 0.20;

    // Load TB into process 1
    harq_opt2.processes[1].load_tb(b"TransportBlock_B".to_vec());

    // Scenario A: All UEs decoded successfully -> No one transmits NACK (DTX)
    // Measured energy = 0.05 < 0.20 threshold => Evaluates to ACK
    let status_dtx = harq_opt2.handle_option2_feedback(1, 0.05);
    assert_eq!(status_dtx, Some(true));

    let (retransmit_dtx, _) = harq_opt2.evaluate_and_advance(1, true);
    assert!(!retransmit_dtx);
    assert!(!harq_opt2.processes[1].is_active);
    assert_eq!(harq_opt2.stats_successful_tbs, 1);

    // Scenario B: Some UE fails decoding -> Transmits NACK on shared resource
    // Measured energy = 0.85 >= 0.20 threshold => Evaluates to NACK
    harq_opt2.processes[2].load_tb(b"TransportBlock_C".to_vec());
    let status_nack = harq_opt2.handle_option2_feedback(2, 0.85);
    assert_eq!(status_nack, Some(false));

    let (retrans_b, rv_b) = harq_opt2.evaluate_and_advance(2, false);
    assert!(retrans_b);
    assert_eq!(rv_b, 2);

    // Exceed max retransmissions (max = 2)
    harq_opt2.evaluate_and_advance(2, false); // retrans count = 2, rv = 3
    let (retrans_drop, _) = harq_opt2.evaluate_and_advance(2, false); // retrans count = 3 > 2 => Drop
    assert!(!retrans_drop);
    assert!(!harq_opt2.processes[2].is_active);
    assert_eq!(harq_opt2.stats_dropped_tbs, 1);
}

#[test]
fn test_dynamic_ptm_ptp_switching_controller() {
    let config = PtmPtpControllerConfig {
        min_ptm_ue_count: 3,
        ptm_bler_threshold: 0.20,
        outlier_cqi_threshold: 4,
        hysteresis_cycles: 1, // immediate switch for test clarity
    };
    let mut controller = PtmPtpController::new(config);

    // 1. Only 2 UEs present (< min_ptm_ue_count = 3) -> AllPtp
    controller.update_ue_telemetry(UeTelemetry::new(0x2001, 10, 18.0));
    controller.update_ue_telemetry(UeTelemetry::new(0x2002, 12, 22.0));
    assert_eq!(controller.evaluate(), SwitchingDecision::AllPtp);

    // 2. Add 2 more healthy UEs -> Total 4 UEs (all CQI >= 8) -> AllPtm
    controller.update_ue_telemetry(UeTelemetry::new(0x2003, 11, 20.0));
    controller.update_ue_telemetry(UeTelemetry::new(0x2004, 9, 16.0));
    assert_eq!(controller.evaluate(), SwitchingDecision::AllPtm);

    // 3. One UE drops into severe fading (CQI = 2 <= outlier_cqi_threshold = 4)
    // Healthy UEs = 3 (>= min_ptm_ue_count). Controller isolates outlier to PTP!
    controller.update_ue_telemetry(UeTelemetry::new(0x2004, 2, -2.5));
    let decision = controller.evaluate();
    match decision {
        SwitchingDecision::SplitSelective {
            ptm_ues,
            ptp_isolated_ues,
        } => {
            assert_eq!(ptm_ues, vec![0x2001, 0x2002, 0x2003]);
            assert_eq!(ptp_isolated_ues, vec![0x2004]);
        }
        other => panic!("Expected SplitSelective, got {:?}", other),
    }

    // 4. Overall group channel degrades with high BLER (> 20%)
    let mut degraded_ue = UeTelemetry::new(0x2001, 10, 18.0);
    degraded_ue.ack_count = 5;
    degraded_ue.nack_count = 10; // BLER = 66%
    controller.update_ue_telemetry(degraded_ue);

    assert_eq!(controller.evaluate(), SwitchingDecision::AllPtp);
}

#[test]
fn test_mcch_state_machine_and_interest_indication() {
    let mcch_config = McchConfig {
        repetition_period_frames: 32,   // every 320ms
        modification_period_frames: 64, // every 640ms
        offset_frames: 0,
        subframe_allocation: 0x0001,
    };
    let mut mcch = McchStateMachine::new(mcch_config);

    let session1 = MbsSessionInfo {
        tmgi: MbsTmgi::new([0xAA, 0xBB, 0xCC], "001", "01"),
        session_id: Some(1),
        g_rnti: 0x1001,
        g_cs_rnti: None,
        mtch_lcid: 3,
        service_type: NrMbsServiceType::Broadcast,
        fsai_list: vec![1001, 1002],
    };

    // Staging update
    mcch.update_sessions(vec![session1.clone()]);
    assert!(mcch.short_message_mcch_indication);
    assert!(mcch.active_sessions.is_empty()); // not active until modification boundary

    // Step 64 frames (sfn 0 through 63)
    for _ in 0..64 {
        mcch.step_frame();
    }
    // Repetition transmissions triggered at sfn 0 and sfn 32, but active_sessions still empty
    assert_eq!(mcch.stats_mcch_transmissions, 2);
    assert!(mcch.active_sessions.is_empty());

    // Step into frame 64: modification boundary reached and applied
    mcch.step_frame();
    assert_eq!(mcch.active_sessions.len(), 1);
    assert!(!mcch.short_message_mcch_indication); // cleared after application
    assert_eq!(mcch.stats_mcch_transmissions, 3);

    // Test wire serialization & deserialization
    let pdu_bytes = mcch.serialize_mcch_pdu();
    let decoded_sessions = McchStateMachine::deserialize_mcch_pdu(&pdu_bytes).unwrap();
    assert_eq!(decoded_sessions.len(), 1);
    assert_eq!(decoded_sessions[0].tmgi.service_id, [0xAA, 0xBB, 0xCC]);
    assert_eq!(decoded_sessions[0].g_rnti, 0x1001);
    assert_eq!(decoded_sessions[0].mtch_lcid, 3);
    assert_eq!(
        decoded_sessions[0].service_type,
        NrMbsServiceType::Broadcast
    );
    assert_eq!(decoded_sessions[0].fsai_list, vec![1001, 1002]);

    // Test MBS Interest Indication (MII)
    let mii = MbsInterestIndication {
        ue_c_rnti: 0x2001,
        interested_tmgis: vec![MbsTmgi::new([0xAA, 0xBB, 0xCC], "001", "01")],
        priority: 1,
    };
    assert_eq!(mii.ue_c_rnti, 0x2001);
    assert_eq!(mii.interested_tmgis.len(), 1);
}

#[test]
fn test_mbs_drx_and_mac_pdu_framing() {
    // -----------------------------------------------------------------------
    // 1. MBS DRX Engine
    // -----------------------------------------------------------------------
    let drx_config = MbsDrxConfig {
        on_duration_slots: 4,
        inactivity_slots: 6,
        harq_rtt_slots: 3,
        retransmission_slots: 4,
        cycle_slots: 20,
        slot_offset: 0,
    };
    let mut drx = MbsDrxEngine::new(drx_config);

    // Slot 0: Cycle begins, on_duration starts
    drx.step_slot();
    assert!(drx.is_active_time());

    // Advance 3 more slots: still in on_duration
    drx.step_slot();
    drx.step_slot();
    drx.step_slot();
    assert!(drx.is_active_time());

    // Slot 4: on_duration expires
    drx.step_slot();
    assert!(!drx.is_active_time());

    // PDCCH grant occurs: wakes inactivity timer
    drx.on_pdcch_grant(true);
    assert!(drx.is_active_time());

    // -----------------------------------------------------------------------
    // 2. MBS MAC PDU Framing & Multiplexing
    // -----------------------------------------------------------------------
    let sdu_mcch = MbsMacSdu {
        lcid: LCID_MCCH,
        data: vec![0x5B, 0x01, 0xAA, 0xBB, 0xCC],
    };
    let sdu_mtch = MbsMacSdu {
        lcid: 3,
        data: vec![0x10; 300], // length > 255 tests 16-bit length indicator
    };

    let encoded_pdu = MbsMacMultiplexer::encode_mac_pdu(&[sdu_mcch.clone(), sdu_mtch.clone()], 4);
    assert!(!encoded_pdu.is_empty());

    let decoded_sdus = MbsMacMultiplexer::decode_mac_pdu(&encoded_pdu).unwrap();
    assert_eq!(decoded_sdus.len(), 2);
    assert_eq!(decoded_sdus[0].lcid, LCID_MCCH);
    assert_eq!(decoded_sdus[0].data, sdu_mcch.data);
    assert_eq!(decoded_sdus[1].lcid, 3);
    assert_eq!(decoded_sdus[1].data, sdu_mtch.data);
}
