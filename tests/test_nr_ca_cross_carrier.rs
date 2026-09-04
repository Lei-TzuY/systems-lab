//! Comprehensive Integration Tests for 3GPP Rel-17 Carrier Aggregation (CA) & Cross-Carrier Scheduling Engine
//! (TS 38.331, TS 38.213, TS 38.214, TS 38.321).

use std::collections::HashMap;
use toy_tcpip::nr_ca_cross_carrier::*;

#[test]
fn test_ca_serving_cell_hierarchy_and_pucch_groups() {
    // 1. PCell configuration: Sub-6 GHz FR1, 15 kHz SCS (mu=0), 100 MHz (273 PRBs)
    let pcell = CaServingCellConfig {
        serv_cell_index: 0,
        pci: 101,
        dl_carrier_freq_mhz: 3500.0,
        ul_carrier_freq_mhz: Some(3500.0),
        scs: NrSubcarrierSpacing::Scs15kHz,
        bandwidth_prb: 273,
        is_pcell: true,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 0, // Self-scheduled
        cif_presence: false,
        cif_val: 0,
    };
    assert!(!pcell.is_cross_carrier_scheduled());
    assert_eq!(pcell.scs.mu(), 0);
    assert_eq!(pcell.scs.slot_duration_us(), 1000);

    // 2. SCell 1: Mid-band FR1, 30 kHz SCS (mu=1), cross-carrier scheduled by PCell (cif=1)
    let scell1 = CaServingCellConfig {
        serv_cell_index: 1,
        pci: 102,
        dl_carrier_freq_mhz: 3700.0,
        ul_carrier_freq_mhz: Some(3700.0),
        scs: NrSubcarrierSpacing::Scs30kHz,
        bandwidth_prb: 273,
        is_pcell: false,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 0, // Cross-carrier scheduled by PCell
        cif_presence: true,
        cif_val: 1,
    };
    assert!(scell1.is_cross_carrier_scheduled());
    assert_eq!(scell1.scs.mu(), 1);
    assert_eq!(scell1.scs.slot_duration_us(), 500);

    // 3. SCell 2: Secondary PUCCH SCell, self-scheduled (cif=2)
    let pucch_scell2 = CaServingCellConfig {
        serv_cell_index: 2,
        pci: 201,
        dl_carrier_freq_mhz: 4800.0,
        ul_carrier_freq_mhz: Some(4800.0),
        scs: NrSubcarrierSpacing::Scs30kHz,
        bandwidth_prb: 133,
        is_pcell: false,
        is_pucch_scell: true,
        pucch_group: PucchGroupId::SecondaryGroup,
        scheduling_cell_id: 2, // Self-scheduled
        cif_presence: true,
        cif_val: 2,
    };
    assert!(!pucch_scell2.is_cross_carrier_scheduled());
    assert!(pucch_scell2.is_pucch_scell);
    assert_eq!(pucch_scell2.pucch_group, PucchGroupId::SecondaryGroup);

    // 4. SCell 3: Belongs to Secondary PUCCH Group, scheduled by SCell 2
    let scell3 = CaServingCellConfig {
        serv_cell_index: 3,
        pci: 202,
        dl_carrier_freq_mhz: 4900.0,
        ul_carrier_freq_mhz: Some(4900.0),
        scs: NrSubcarrierSpacing::Scs60kHz,
        bandwidth_prb: 66,
        is_pcell: false,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::SecondaryGroup,
        scheduling_cell_id: 2, // Cross-carrier scheduled by SCell 2
        cif_presence: true,
        cif_val: 3,
    };
    assert!(scell3.is_cross_carrier_scheduled());
    assert_eq!(scell3.scs.mu(), 2);
    assert_eq!(scell3.scs.slot_duration_us(), 250);
}

#[test]
fn test_cross_carrier_scheduling_cif_and_timing() {
    let pcell = CaServingCellConfig {
        serv_cell_index: 0,
        pci: 101,
        dl_carrier_freq_mhz: 3500.0,
        ul_carrier_freq_mhz: Some(3500.0),
        scs: NrSubcarrierSpacing::Scs15kHz, // mu=0
        bandwidth_prb: 273,
        is_pcell: true,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 0,
        cif_presence: false,
        cif_val: 0,
    };

    let scell1 = CaServingCellConfig {
        serv_cell_index: 1,
        pci: 102,
        dl_carrier_freq_mhz: 3700.0,
        ul_carrier_freq_mhz: Some(3700.0),
        scs: NrSubcarrierSpacing::Scs30kHz, // mu=1
        bandwidth_prb: 100,
        is_pcell: false,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 0, // Scheduled by PCell
        cif_presence: true,
        cif_val: 1,
    };

    // Valid DL Cross-Carrier Grant
    let grant = CrossCarrierScheduler::create_cross_carrier_grant(
        &pcell, &scell1, true, // is_downlink
        2,    // k0 offset
        0,    // prb_start
        50,   // prb_count
        16,   // mcs
        3,    // harq_pid
    )
    .expect("Failed to create valid cross-carrier grant");

    assert_eq!(grant.scheduling_cell_index, 0);
    assert_eq!(grant.scheduled_cell_index, 1);
    assert_eq!(grant.cif, 1);
    assert_eq!(grant.k_offset_slots, 2);
    assert_eq!(grant.prb_count, 50);
    assert_eq!(grant.harq_process_id, 3);
    assert!(grant.ndi);
    assert_eq!(grant.rv, 0);

    // PRB overflow validation
    let prb_overflow_err = CrossCarrierScheduler::create_cross_carrier_grant(
        &pcell, &scell1, true, 2, 80, 30, // 80 + 30 = 110 > 100 PRBs
        16, 3,
    );
    assert!(prb_overflow_err.is_err());

    // CIF missing validation
    let mut invalid_scell = scell1.clone();
    invalid_scell.cif_presence = false;
    let cif_missing_err = CrossCarrierScheduler::create_cross_carrier_grant(
        &pcell,
        &invalid_scell,
        true,
        2,
        0,
        20,
        16,
        1,
    );
    assert!(cif_missing_err.is_err());

    // Mismatched scheduling cell validation
    let wrong_scheduling_cell = CaServingCellConfig {
        serv_cell_index: 5,
        pci: 999,
        dl_carrier_freq_mhz: 2100.0,
        ul_carrier_freq_mhz: None,
        scs: NrSubcarrierSpacing::Scs15kHz,
        bandwidth_prb: 50,
        is_pcell: false,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 5,
        cif_presence: false,
        cif_val: 5,
    };
    let wrong_cell_err = CrossCarrierScheduler::create_cross_carrier_grant(
        &wrong_scheduling_cell,
        &scell1,
        true,
        2,
        0,
        20,
        16,
        1,
    );
    assert!(wrong_cell_err.is_err());

    // Mixed Numerology Timing Verification (TS 38.214)
    // 1. Scheduling on 15 kHz (mu=0), Scheduled on 30 kHz (mu=1)
    // delta_mu = +1, factor = 2. Scheduling slot 4 -> base target slot = 8.
    // With k_offset = 2 => target slot = 10.
    let target_slot_1 = CrossCarrierScheduler::calculate_target_slot(
        NrSubcarrierSpacing::Scs15kHz,
        NrSubcarrierSpacing::Scs30kHz,
        4,
        2,
    );
    assert_eq!(target_slot_1, 10);

    // 2. Scheduling on 30 kHz (mu=1), Scheduled on 15 kHz (mu=0)
    // delta_mu = -1, factor = 2. Scheduling slot 8 -> base target slot = 4.
    // With k_offset = 1 => target slot = 5.
    let target_slot_2 = CrossCarrierScheduler::calculate_target_slot(
        NrSubcarrierSpacing::Scs30kHz,
        NrSubcarrierSpacing::Scs15kHz,
        8,
        1,
    );
    assert_eq!(target_slot_2, 5);

    // 3. Scheduling on 30 kHz (mu=1), Scheduled on 60 kHz (mu=2)
    // delta_mu = +1, factor = 2. Scheduling slot 5 -> base target slot = 10.
    // With k_offset = 3 => target slot = 13.
    let target_slot_3 = CrossCarrierScheduler::calculate_target_slot(
        NrSubcarrierSpacing::Scs30kHz,
        NrSubcarrierSpacing::Scs60kHz,
        5,
        3,
    );
    assert_eq!(target_slot_3, 13);

    // 4. Same SCS (mu=1 -> mu=1): slot 7 + offset 2 = 9
    let target_slot_4 = CrossCarrierScheduler::calculate_target_slot(
        NrSubcarrierSpacing::Scs30kHz,
        NrSubcarrierSpacing::Scs30kHz,
        7,
        2,
    );
    assert_eq!(target_slot_4, 9);
}

#[test]
fn test_scell_activation_deactivation_mac_ces() {
    // 1. 1-Octet MAC CE (LCID 62, TS 38.321 §6.1.3.10)
    // Activate SCell 1, SCell 3, and SCell 7 (Ci bits for i=1..7)
    // Octet layout: [C7, C6, C5, C4, C3, C2, C1, R]
    // C7=1, C6=0, C5=0, C4=0, C3=1, C2=0, C1=1, R=0 => 0b10001010 = 0x8A (with bit 0 as reserved R)
    let encoded_1_octet = ScellMacCeCodec::encode_one_octet_indices(&[1, 3, 7]);
    assert_eq!(encoded_1_octet.len(), 1);
    assert_eq!(encoded_1_octet[0], 0b10001010);

    let decoded_1_octet = ScellMacCeCodec::decode_one_octet(&encoded_1_octet)
        .expect("Failed to decode 1-octet MAC CE");
    assert!(decoded_1_octet[1]);
    assert!(!decoded_1_octet[2]);
    assert!(decoded_1_octet[3]);
    assert!(!decoded_1_octet[4]);
    assert!(!decoded_1_octet[5]);
    assert!(!decoded_1_octet[6]);
    assert!(decoded_1_octet[7]);

    // Invalid length check
    assert!(ScellMacCeCodec::decode_one_octet(&[]).is_err());
    assert!(ScellMacCeCodec::decode_one_octet(&[0x12, 0x34]).is_err());

    // 2. 4-Octet MAC CE (LCID 61, TS 38.321 §6.1.3.10)
    // Activate SCell 2, SCell 8, SCell 15, SCell 29, SCell 31
    let encoded_4_octet = ScellMacCeCodec::encode_four_octet_indices(&[2, 8, 15, 29, 31]);
    assert_eq!(encoded_4_octet.len(), 4);

    let decoded_4_octet = ScellMacCeCodec::decode_four_octet(&encoded_4_octet)
        .expect("Failed to decode 4-octet MAC CE");
    assert!(!decoded_4_octet[1]);
    assert!(decoded_4_octet[2]);
    assert!(!decoded_4_octet[3]);
    assert!(decoded_4_octet[8]);
    assert!(decoded_4_octet[15]);
    assert!(decoded_4_octet[29]);
    assert!(!decoded_4_octet[30]);
    assert!(decoded_4_octet[31]);

    // Invalid length check for 4-octet CE
    assert!(ScellMacCeCodec::decode_four_octet(&[0x01, 0x02, 0x03]).is_err());
    assert!(ScellMacCeCodec::decode_four_octet(&[0x01, 0x02, 0x03, 0x04, 0x05]).is_err());
}

#[test]
fn test_scell_deactivation_timer_and_state_machine() {
    let scell_cfg = CaServingCellConfig {
        serv_cell_index: 1,
        pci: 102,
        dl_carrier_freq_mhz: 3700.0,
        ul_carrier_freq_mhz: Some(3700.0),
        scs: NrSubcarrierSpacing::Scs30kHz,
        bandwidth_prb: 273,
        is_pcell: false,
        is_pucch_scell: false,
        pucch_group: PucchGroupId::PrimaryGroup,
        scheduling_cell_id: 0,
        cif_presence: true,
        cif_val: 1,
    };

    let mut scell_mgr = ScellManager::new(scell_cfg, 100); // 100 subframes (100 ms)
    assert_eq!(scell_mgr.state, ScellState::Deactivated);
    assert!(!scell_mgr.pdcch_monitoring_active);
    assert!(!scell_mgr.csi_reporting_active);
    assert!(!scell_mgr.srs_transmission_active);
    assert!(scell_mgr.harq_buffers_flushed);

    // Activate SCell
    scell_mgr.activate();
    assert_eq!(scell_mgr.state, ScellState::Active);
    assert_eq!(scell_mgr.timer_countdown, 100);
    assert!(scell_mgr.pdcch_monitoring_active);
    assert!(scell_mgr.csi_reporting_active);
    assert!(scell_mgr.srs_transmission_active);
    assert!(!scell_mgr.harq_buffers_flushed);

    // Tick 40 subframes down
    scell_mgr.tick_subframes(40);
    assert_eq!(scell_mgr.state, ScellState::Active);
    assert_eq!(scell_mgr.timer_countdown, 60);

    // Data activity resets timer
    scell_mgr.restart_deactivation_timer();
    assert_eq!(scell_mgr.timer_countdown, 100);

    // Transition to Dormant BWP (Rel-17 fast recovery)
    scell_mgr.transition_to_dormant();
    assert_eq!(scell_mgr.state, ScellState::Dormant);
    assert!(!scell_mgr.pdcch_monitoring_active); // No PDCCH monitoring in dormant state
    assert!(scell_mgr.csi_reporting_active); // CSI reporting remains active
    assert!(!scell_mgr.srs_transmission_active);
    assert_eq!(scell_mgr.timer_countdown, 0); // Timer stopped in dormant state

    // Fast resume back to Active
    scell_mgr.activate();
    assert_eq!(scell_mgr.state, ScellState::Active);
    assert_eq!(scell_mgr.timer_countdown, 100);
    assert!(scell_mgr.pdcch_monitoring_active);

    // Tick down to expiry (100 subframes)
    let deactivated = scell_mgr.tick_subframes(100);
    assert!(deactivated);
    assert_eq!(scell_mgr.state, ScellState::Deactivated);
    assert_eq!(scell_mgr.timer_countdown, 0);
    assert!(!scell_mgr.pdcch_monitoring_active);
    assert!(!scell_mgr.csi_reporting_active);
    assert!(!scell_mgr.srs_transmission_active);
    assert!(scell_mgr.harq_buffers_flushed); // HARQ buffers flushed upon deactivation

    // Explicit deactivate call
    scell_mgr.activate();
    assert_eq!(scell_mgr.state, ScellState::Active);
    scell_mgr.deactivate();
    assert_eq!(scell_mgr.state, ScellState::Deactivated);
    assert!(scell_mgr.harq_buffers_flushed);
}

#[test]
fn test_multi_carrier_harq_codebook_multiplexer() {
    let mut cell_configs = HashMap::new();

    // PCell: Primary PUCCH Group
    cell_configs.insert(
        0,
        CaServingCellConfig {
            serv_cell_index: 0,
            pci: 101,
            dl_carrier_freq_mhz: 3500.0,
            ul_carrier_freq_mhz: Some(3500.0),
            scs: NrSubcarrierSpacing::Scs15kHz,
            bandwidth_prb: 273,
            is_pcell: true,
            is_pucch_scell: false,
            pucch_group: PucchGroupId::PrimaryGroup,
            scheduling_cell_id: 0,
            cif_presence: false,
            cif_val: 0,
        },
    );

    // SCell 1: Primary PUCCH Group
    cell_configs.insert(
        1,
        CaServingCellConfig {
            serv_cell_index: 1,
            pci: 102,
            dl_carrier_freq_mhz: 3700.0,
            ul_carrier_freq_mhz: Some(3700.0),
            scs: NrSubcarrierSpacing::Scs30kHz,
            bandwidth_prb: 273,
            is_pcell: false,
            is_pucch_scell: false,
            pucch_group: PucchGroupId::PrimaryGroup,
            scheduling_cell_id: 0,
            cif_presence: true,
            cif_val: 1,
        },
    );

    // SCell 2: PUCCH SCell, Secondary PUCCH Group
    cell_configs.insert(
        2,
        CaServingCellConfig {
            serv_cell_index: 2,
            pci: 201,
            dl_carrier_freq_mhz: 4800.0,
            ul_carrier_freq_mhz: Some(4800.0),
            scs: NrSubcarrierSpacing::Scs30kHz,
            bandwidth_prb: 133,
            is_pcell: false,
            is_pucch_scell: true,
            pucch_group: PucchGroupId::SecondaryGroup,
            scheduling_cell_id: 2,
            cif_presence: true,
            cif_val: 2,
        },
    );

    // SCell 3: Secondary PUCCH Group
    cell_configs.insert(
        3,
        CaServingCellConfig {
            serv_cell_index: 3,
            pci: 202,
            dl_carrier_freq_mhz: 4900.0,
            ul_carrier_freq_mhz: Some(4900.0),
            scs: NrSubcarrierSpacing::Scs60kHz,
            bandwidth_prb: 66,
            is_pcell: false,
            is_pucch_scell: false,
            pucch_group: PucchGroupId::SecondaryGroup,
            scheduling_cell_id: 2,
            cif_presence: true,
            cif_val: 3,
        },
    );

    let feedbacks = vec![
        CellHarqFeedback {
            serv_cell_index: 0,
            harq_process_id: 0,
            is_ack: true,
        },
        CellHarqFeedback {
            serv_cell_index: 1,
            harq_process_id: 1,
            is_ack: false,
        },
        CellHarqFeedback {
            serv_cell_index: 2,
            harq_process_id: 0,
            is_ack: true,
        },
        CellHarqFeedback {
            serv_cell_index: 3,
            harq_process_id: 2,
            is_ack: true,
        },
    ];

    let report = CaHarqMultiplexer::multiplex_feedback(&feedbacks, &cell_configs);

    // Primary group contains feedbacks for Cell 0 (ACK=true) and Cell 1 (NACK=false)
    assert_eq!(report.primary_group_harq_bits, vec![true, false]);

    // Secondary group contains feedbacks for Cell 2 (ACK=true) and Cell 3 (ACK=true)
    assert_eq!(report.secondary_group_harq_bits, vec![true, true]);
}
