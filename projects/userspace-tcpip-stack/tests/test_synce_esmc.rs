use toy_tcpip::synce_esmc::{
    ESMC_SUBTYPE, ITU_T_ESMC_SUBTYPE, ITU_T_OUI, QualityLevel, SyncEEsmcEngine, SyncEEsmcPacket,
};

#[test]
fn test_synce_esmc_pdu_serialization_and_parsing() {
    let pkt = SyncEEsmcPacket::new(true, QualityLevel::QlPrc);
    let wire = pkt.serialize();

    assert!(wire.len() >= 36);
    assert_eq!(wire[0], ESMC_SUBTYPE);
    assert_eq!(&wire[1..4], &ITU_T_OUI);
    assert_eq!(u16::from_be_bytes([wire[4], wire[5]]), ITU_T_ESMC_SUBTYPE);
    assert_eq!(wire[6] & 0x08, 0x08); // Event flag set

    let parsed = SyncEEsmcPacket::parse(&wire).expect("parse ESMC PDU");
    assert_eq!(parsed.event_flag, true);
    assert_eq!(parsed.quality_level, QualityLevel::QlPrc);
}

#[test]
fn test_synce_clock_selection_arbitration_and_failover() {
    let mut engine = SyncEEsmcEngine::new();

    engine.set_port_priority(1, 10);
    engine.set_port_priority(2, 20);
    engine.set_port_priority(3, 5);

    // Port 1 receives QL-SSU-A (Rank 2)
    engine.process_rx_esmc(1, &SyncEEsmcPacket::new(false, QualityLevel::QlSsuA));
    assert_eq!(engine.selected_port, Some(1));
    assert_eq!(engine.selected_ql, QualityLevel::QlSsuA);

    // Port 2 receives QL-PRC (Rank 1 - Superior Quality)
    engine.process_rx_esmc(2, &SyncEEsmcPacket::new(false, QualityLevel::QlPrc));
    assert_eq!(engine.selected_port, Some(2));
    assert_eq!(engine.selected_ql, QualityLevel::QlPrc);

    // Port 3 receives QL-PRC (Rank 1, but Priority 5 is higher than Port 2's Priority 20)
    engine.process_rx_esmc(3, &SyncEEsmcPacket::new(false, QualityLevel::QlPrc));
    assert_eq!(engine.selected_port, Some(3));
    assert_eq!(engine.selected_ql, QualityLevel::QlPrc);

    // Port 3 fails and sends QL-DNU (Do Not Use)
    engine.process_rx_esmc(3, &SyncEEsmcPacket::new(true, QualityLevel::QlDnu));
    // Failover back to Port 2 (next best QL-PRC)
    assert_eq!(engine.selected_port, Some(2));
    assert_eq!(engine.selected_ql, QualityLevel::QlPrc);
}

#[test]
fn test_synce_extended_ql_tlv_serialization_and_parsing() {
    use toy_tcpip::synce_esmc::{EnhancedQualityLevel, ExtendedQlTlv};

    let clock_id = [0x00, 0x11, 0x22, 0xFF, 0xFE, 0x33, 0x44, 0x55];
    let mut ext_tlv = ExtendedQlTlv::new(EnhancedQualityLevel::QlEprtc, clock_id);
    ext_tlv.mixed_network = true;
    ext_tlv.cascaded_eeec_count = 3;
    ext_tlv.cascaded_eprtc_count = 1;

    let wire_tlv = ext_tlv.serialize();
    assert_eq!(wire_tlv.len(), 20);

    let parsed_tlv = ExtendedQlTlv::parse(&wire_tlv).expect("parse extended QL TLV");
    assert_eq!(parsed_tlv.enhanced_ql, EnhancedQualityLevel::QlEprtc);
    assert_eq!(parsed_tlv.clock_identity, clock_id);
    assert_eq!(parsed_tlv.mixed_network, true);
    assert_eq!(parsed_tlv.cascaded_eeec_count, 3);
    assert_eq!(parsed_tlv.cascaded_eprtc_count, 1);

    // Full packet serialization with both base QL and Extended QL
    let mut pkt = SyncEEsmcPacket::new(false, QualityLevel::QlPrc);
    pkt.extended_ql = Some(parsed_tlv);

    let wire_pkt = pkt.serialize();
    let parsed_pkt = SyncEEsmcPacket::parse(&wire_pkt).expect("parse packet with ext QL");
    assert_eq!(parsed_pkt.quality_level, QualityLevel::QlPrc);
    let parsed_ext = parsed_pkt.extended_ql.expect("has ext QL");
    assert_eq!(parsed_ext.enhanced_ql, EnhancedQualityLevel::QlEprtc);
    assert_eq!(parsed_ext.clock_identity, clock_id);
}

#[test]
fn test_synce_wtr_flap_damping_lifecycle() {
    use toy_tcpip::synce_esmc::PortSyncState;

    let mut engine = SyncEEsmcEngine::new();
    engine.set_wtr_duration(3); // 3 ticks WTR

    engine.set_port_priority(1, 10);
    engine.set_port_priority(2, 20);

    // Port 1 receives QL-PRC and becomes active clock source
    engine.process_rx_esmc(1, &SyncEEsmcPacket::new(false, QualityLevel::QlPrc));
    assert_eq!(engine.selected_port, Some(1));

    // Port 1 fails -> receives QL-DNU
    engine.process_rx_esmc(1, &SyncEEsmcPacket::new(true, QualityLevel::QlDnu));
    assert_eq!(engine.selected_port, None);
    assert!(engine.holdover_active);
    assert_eq!(engine.port_states.get(&1), Some(&PortSyncState::Failed));

    // Port 1 recovers and announces QL-PRC again -> enters WaitToRestore
    engine.process_rx_esmc(1, &SyncEEsmcPacket::new(true, QualityLevel::QlPrc));
    assert_eq!(
        engine.port_states.get(&1),
        Some(&PortSyncState::WaitToRestore { remaining_ticks: 3 })
    );
    // Port 1 is not selected yet during WTR dampening
    assert_eq!(engine.selected_port, None);

    // Tick 1 -> remaining 2
    engine.tick_wtr();
    assert_eq!(
        engine.port_states.get(&1),
        Some(&PortSyncState::WaitToRestore { remaining_ticks: 2 })
    );
    assert_eq!(engine.selected_port, None);

    // Tick 2 -> remaining 1
    engine.tick_wtr();
    assert_eq!(
        engine.port_states.get(&1),
        Some(&PortSyncState::WaitToRestore { remaining_ticks: 1 })
    );
    assert_eq!(engine.selected_port, None);

    // Tick 3 -> WTR expires, Port 1 transitions to Active and is re-elected
    engine.tick_wtr();
    assert_eq!(engine.port_states.get(&1), Some(&PortSyncState::Active));
    assert_eq!(engine.selected_port, Some(1));
    assert_eq!(engine.selected_ql, QualityLevel::QlPrc);
    assert!(!engine.holdover_active);
}

#[test]
fn test_synce_enhanced_ql_selection_and_tx_generation_timing_loop_prevention() {
    use toy_tcpip::synce_esmc::{EnhancedQualityLevel, ExtendedQlTlv};

    let mut engine = SyncEEsmcEngine::new();
    let local_clock_id = [0xAA, 0xBB, 0xCC, 0xFF, 0xFE, 0x11, 0x22, 0x33];

    // Port 1: Standard legacy PRC (rank 4)
    engine.process_rx_esmc(1, &SyncEEsmcPacket::new(false, QualityLevel::QlPrc));

    // Port 2: ePRTC (enhanced PRTC, rank 1 - higher quality than standard PRC)
    let upstream_clock_id = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33];
    let mut pkt_eprtc = SyncEEsmcPacket::new(false, QualityLevel::QlPrc);
    let mut ext_eprtc = ExtendedQlTlv::new(EnhancedQualityLevel::QlEprtc, upstream_clock_id);
    ext_eprtc.cascaded_eeec_count = 2;
    pkt_eprtc.extended_ql = Some(ext_eprtc);

    engine.process_rx_esmc(2, &pkt_eprtc);

    // Port 2 wins arbitration due to ePRTC
    assert_eq!(engine.selected_port, Some(2));
    assert_eq!(engine.selected_ql, QualityLevel::QlPrc);
    assert_eq!(engine.selected_ext_ql, Some(EnhancedQualityLevel::QlEprtc));

    // 1. Outbound transmission on Port 2 (our master input port)
    // MUST send QL-DNU to prevent timing loop! (ITU-T G.781)
    let tx_p2 = engine.generate_tx_esmc(2, local_clock_id);
    assert_eq!(tx_p2.quality_level, QualityLevel::QlDnu);
    assert_eq!(tx_p2.extended_ql, None);

    // 2. Outbound transmission on downstream Port 3
    // Forwards synchronized ePRTC clock with incremented cascaded hop count
    let tx_p3 = engine.generate_tx_esmc(3, local_clock_id);
    assert_eq!(tx_p3.quality_level, QualityLevel::QlPrc);
    let p3_ext = tx_p3.extended_ql.expect("has downstream ext QL");
    assert_eq!(p3_ext.enhanced_ql, EnhancedQualityLevel::QlEprtc);
    assert_eq!(p3_ext.clock_identity, local_clock_id);
    assert_eq!(p3_ext.cascaded_eeec_count, 3); // 2 + 1 hop
}
