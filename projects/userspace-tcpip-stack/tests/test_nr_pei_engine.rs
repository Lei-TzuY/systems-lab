//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR Paging Early Indication (PEI) & Subgrouping Engine
//! (TS 38.213 §10.5, TS 38.304 §7.4, and TS 38.331).

use toy_tcpip::nr_pei_engine::*;

#[test]
fn test_pei_config_and_subgrouping_algorithms() {
    // 1. Validate Default and Custom Configurations
    let config = PeiConfig::default();
    assert_eq!(config.pei_frame_offset, 2);
    assert_eq!(config.payload_size_dci_2_7, 16);
    assert_eq!(config.subgroups_per_po, 4);
    assert_eq!(config.pos_per_pei, 2);
    assert!(config.short_message_present);
    assert_eq!(config.subgrouping_scheme, SubgroupingScheme::UeIdBased);

    // 2. CN-Assigned Subgrouping with Valid, Out-of-Bounds, and None Fallbacks
    let ue_id = 123456789u64;

    // Explicit valid CN subgroup assignment
    let sg_cn_valid =
        PeiSubgroupEngine::calculate_subgroup_id(ue_id, SubgroupingScheme::CnAssigned, Some(2), 4);
    assert_eq!(sg_cn_valid, 2);

    // CN subgroup out of range (e.g. 5 >= 4) -> Fallback to UE-ID hashing
    let sg_cn_invalid =
        PeiSubgroupEngine::calculate_subgroup_id(ue_id, SubgroupingScheme::CnAssigned, Some(5), 4);
    let expected_hash = PeiSubgroupEngine::calculate_ue_id_hash_subgroup(ue_id, 4);
    assert_eq!(sg_cn_invalid, expected_hash);

    // CN subgroup None -> Fallback to UE-ID hashing
    let sg_cn_none =
        PeiSubgroupEngine::calculate_subgroup_id(ue_id, SubgroupingScheme::CnAssigned, None, 4);
    assert_eq!(sg_cn_none, expected_hash);

    // 3. Mathematical Properties of UE-ID Hashing per TS 38.304 §7.4:
    // SubgroupId = floor((UE_ID mod 4096) * N_subgroup / 4096)
    for num_subgroups in [1, 2, 4, 8] {
        for test_id in [0, 1, 1023, 1024, 2047, 2048, 4095, 4096, 5000, 100000] {
            let sg = PeiSubgroupEngine::calculate_ue_id_hash_subgroup(test_id, num_subgroups);
            assert!(sg < num_subgroups);
        }
    }

    // Exact boundary tests for 4 subgroups:
    // 0..1023 -> Subgroup 0
    // 1024..2047 -> Subgroup 1
    // 2048..3071 -> Subgroup 2
    // 3072..4095 -> Subgroup 3
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(0, 4), 0);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(1023, 4), 0);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(1024, 4), 1);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(2047, 4), 1);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(2048, 4), 2);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(3071, 4), 2);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(3072, 4), 3);
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(4095, 4), 3);
    // Wrap-around modulo 4096
    assert_eq!(PeiSubgroupEngine::calculate_ue_id_hash_subgroup(4096, 4), 0);
}

#[test]
fn test_dci_format_2_7_wire_encoding_and_decoding() {
    // 2 POs, 4 subgroups per PO => 8 subgroup bits
    // Followed by Short Message: 1 indicator bit + 8 payload bits => 9 bits
    // Total required: 17 bits -> configure 24-bit DCI
    let mut dci = DciFormat2_7::new(24);

    // Set Subgroup Indications:
    // PO 0: Subgroup 1 and 3 are paged
    dci.set_subgroup_indication(0, 1, 4, true).unwrap();
    dci.set_subgroup_indication(0, 3, 4, true).unwrap();
    // PO 1: Subgroup 0 is paged, Subgroup 2 is not
    dci.set_subgroup_indication(1, 0, 4, true).unwrap();
    dci.set_subgroup_indication(1, 2, 4, false).unwrap();

    // Set Short Message (start offset = 8):
    // systemInfoModification (bit 1 = 0x80)
    let short_msg_val = 0b10000000;
    dci.set_short_message(8, true, short_msg_val).unwrap();

    // Serialize to bytes (24 bits = 3 bytes)
    let bytes = dci.to_bytes();
    assert_eq!(bytes.len(), 3);

    // Deserialize and verify bit-exact fidelity
    let decoded = DciFormat2_7::from_bytes(&bytes, 24).expect("Failed to deserialize DCI 2_7");
    assert_eq!(decoded.total_bits, 24);

    // PO 0 Verifications
    assert!(!decoded.get_subgroup_indication(0, 0, 4).unwrap());
    assert!(decoded.get_subgroup_indication(0, 1, 4).unwrap());
    assert!(!decoded.get_subgroup_indication(0, 2, 4).unwrap());
    assert!(decoded.get_subgroup_indication(0, 3, 4).unwrap());

    // PO 1 Verifications
    assert!(decoded.get_subgroup_indication(1, 0, 4).unwrap());
    assert!(!decoded.get_subgroup_indication(1, 1, 4).unwrap());
    assert!(!decoded.get_subgroup_indication(1, 2, 4).unwrap());
    assert!(!decoded.get_subgroup_indication(1, 3, 4).unwrap());

    // Short Message Verification
    let (has_short_msg, msg) = decoded.get_short_message(8).unwrap();
    assert!(has_short_msg);
    assert_eq!(msg, short_msg_val);

    // Boundary & Error Checks
    assert!(dci.set_subgroup_indication(0, 4, 4, true).is_err()); // Subgroup 4 >= 4
    assert!(dci.set_subgroup_indication(6, 0, 4, true).is_err()); // Bit offset 24 >= 24
    assert!(DciFormat2_7::from_bytes(&[0x12, 0x34], 24).is_err()); // 2 bytes < 3 bytes
}

#[test]
fn test_pei_frame_and_slot_timing_calculator() {
    let ue_id = 456789u64;
    let drx_cycle = 128u32;
    let n_param = 128u32;
    let pf_offset = 0u32;

    // 1. Calculate Paging Frame SFN
    let pf_sfn = PeiTimingCalculator::calculate_paging_frame(ue_id, drx_cycle, n_param, pf_offset);
    assert!(pf_sfn < MAX_SFN);
    assert_eq!(pf_sfn, (ue_id % (n_param as u64)) as u32);

    // 2. Calculate Paging Occasion index i_s
    let po_idx = PeiTimingCalculator::calculate_po_index(ue_id, n_param, 2);
    assert!(po_idx < 2);

    // 3. Calculate PEI Frame with Frame Offset
    // Case A: Standard offset without wrap-around
    let pei_sfn_normal = PeiTimingCalculator::calculate_pei_frame(50, 2);
    assert_eq!(pei_sfn_normal, 48);

    // Case B: Boundary wrap-around (SFN 0..1023)
    // PF SFN = 1, frame_offset = 3 => (1 + 1024 - 3) = 1022
    let pei_sfn_wrap = PeiTimingCalculator::calculate_pei_frame(1, 3);
    assert_eq!(pei_sfn_wrap, 1022);

    // PF SFN = 0, frame_offset = 1 => 1023
    let pei_sfn_zero = PeiTimingCalculator::calculate_pei_frame(0, 1);
    assert_eq!(pei_sfn_zero, 1023);
}

#[test]
fn test_pei_receiver_wake_up_decision_logic() {
    let config = PeiConfig {
        pei_frame_offset: 2,
        payload_size_dci_2_7: 24,
        subgroups_per_po: 4,
        pos_per_pei: 2,
        short_message_present: true,
        subgrouping_scheme: SubgroupingScheme::CnAssigned,
    };

    // UE 1: Assigned to subgroup 2
    let mut ue_rx = PeiUeReceiver::new(1001, Some(2));
    assert_eq!(ue_rx.cn_subgroup, Some(2));

    // Case 1: PEI indicates paging for subgroup 2 on PO 0
    let mut dci1 = DciFormat2_7::new(24);
    dci1.set_subgroup_indication(0, 2, 4, true).unwrap();
    let decision1 = ue_rx.evaluate_pei(Some(&dci1), &config, 0);
    assert_eq!(
        decision1,
        PeiWakeupDecision::WakeUpPaging {
            po_index: 0,
            subgroup_id: 2,
        }
    );

    // Case 2: PEI indicates NO paging for subgroup 2 (bit is false) -> Skip PO!
    let mut dci2 = DciFormat2_7::new(24);
    dci2.set_subgroup_indication(0, 1, 4, true).unwrap(); // Paging for subgroup 1, not 2
    let decision2 = ue_rx.evaluate_pei(Some(&dci2), &config, 0);
    assert_eq!(
        decision2,
        PeiWakeupDecision::SkipPaging {
            po_index: 0,
            subgroup_id: 2,
        }
    );

    // Case 3: Short Message present -> All UEs wake up regardless of subgroup bit
    let mut dci3 = DciFormat2_7::new(24);
    dci3.set_short_message(8, true, 0b01000000).unwrap(); // ETWS notification
    let decision3 = ue_rx.evaluate_pei(Some(&dci3), &config, 0);
    assert_eq!(
        decision3,
        PeiWakeupDecision::WakeUpShortMessage {
            short_message: 0b01000000,
        }
    );

    // Case 4: PEI DTX (None) -> Defensive fallback wake-up
    let decision4 = ue_rx.evaluate_pei(None, &config, 0);
    match decision4 {
        PeiWakeupDecision::WakeUpFallback { ref reason } => {
            assert!(reason.contains("DTX"));
        }
        _ => panic!("Expected WakeUpFallback on DTX"),
    }

    // Verify Metric Aggregations
    assert_eq!(ue_rx.metrics.total_pei_evaluated, 4);
    assert_eq!(ue_rx.metrics.paging_wakeups, 1);
    assert_eq!(ue_rx.metrics.paging_skipped, 1);
    assert_eq!(ue_rx.metrics.short_message_wakeups, 1);
    assert_eq!(ue_rx.metrics.fallback_wakeups, 1);
}

#[test]
fn test_pei_energy_saving_and_false_alarm_metrics() {
    let config = PeiConfig {
        pei_frame_offset: 2,
        payload_size_dci_2_7: 24,
        subgroups_per_po: 4,
        pos_per_pei: 1,
        short_message_present: true,
        subgrouping_scheme: SubgroupingScheme::UeIdBased,
    };

    // UE with hash subgroup
    let ue_id = 500u64; // (500 mod 4096) * 4 / 4096 = 0
    let target_subgroup = PeiSubgroupEngine::calculate_ue_id_hash_subgroup(ue_id, 4);
    assert_eq!(target_subgroup, 0);

    let mut ue_rx = PeiUeReceiver::new(ue_id, None);

    // Simulate 100 consecutive PEI occasions:
    // - 80 occasions: No paging for subgroup 0 (sleep)
    // - 12 occasions: Paging targeted to subgroup 0 (wake)
    // - 5 occasions: System Information Modification (Short Msg)
    // - 3 occasions: Channel fading DTX (fallback)
    let mut dci_quiet = DciFormat2_7::new(24);
    dci_quiet.set_subgroup_indication(0, 1, 4, true).unwrap(); // Other subgroup

    let mut dci_paged = DciFormat2_7::new(24);
    dci_paged.set_subgroup_indication(0, 0, 4, true).unwrap(); // Target subgroup

    let mut dci_short_msg = DciFormat2_7::new(24);
    dci_short_msg.set_short_message(4, true, 0x01).unwrap();

    for _ in 0..80 {
        ue_rx.evaluate_pei(Some(&dci_quiet), &config, 0);
    }
    for _ in 0..12 {
        ue_rx.evaluate_pei(Some(&dci_paged), &config, 0);
    }
    for _ in 0..5 {
        ue_rx.evaluate_pei(Some(&dci_short_msg), &config, 0);
    }
    for _ in 0..3 {
        ue_rx.evaluate_pei(None, &config, 0);
    }

    assert_eq!(ue_rx.metrics.total_pei_evaluated, 100);
    assert_eq!(ue_rx.metrics.paging_skipped, 80);
    assert_eq!(ue_rx.metrics.paging_wakeups, 12);
    assert_eq!(ue_rx.metrics.short_message_wakeups, 5);
    assert_eq!(ue_rx.metrics.fallback_wakeups, 3);

    // Total wakeups = 12 + 5 + 3 = 20 wakeups out of 100 POs
    // Legacy Rel-15/16 requires 100 wakeups (100 * 100 uJ = 10,000 uJ)
    // PEI requires: (100 PEI checks * 10 uJ) + (20 PO wakeups * 100 uJ) = 1,000 + 2,000 = 3,000 uJ
    // Energy Savings = (10,000 - 3,000) / 10,000 = 70.0%
    let pei_cost = 10.0;
    let po_cost = 100.0;
    let savings_pct = ue_rx.calculate_energy_savings_percentage(pei_cost, po_cost);

    assert!((savings_pct - 70.0).abs() < 1e-6);
    assert!(savings_pct > 65.0);
}
