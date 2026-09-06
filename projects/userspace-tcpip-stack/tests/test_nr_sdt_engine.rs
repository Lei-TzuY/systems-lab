//! Integration tests for 3GPP Rel-17 5G NR Small Data Transmission (SDT) in RRC_INACTIVE Engine
//!
//! Conforms to 3GPP TS 38.300 §18, TS 38.321 §5.27, and TS 38.331.

use toy_tcpip::nr_sdt_engine::{
    SdtConfig, SdtEngine, SdtMacPdu, SdtProcedureState, SdtResponseAction, SdtType,
};

#[test]
fn test_sdt_criteria_and_transmission_mode_selection() {
    let mut config = SdtConfig::default();
    config.data_volume_threshold_bytes = 1024;
    config.rsrp_threshold_dbm = -105.0;
    config.cg_configured = true;
    config.support_2step_ra = true;

    let mut engine = SdtEngine::new(config);
    engine.update_timing_alignment(); // Valid TA

    // 1. Valid buffer (256 bytes) and good RSRP (-90 dBm) with valid TA -> CG-SDT
    let sdt_mode = engine.evaluate_sdt_criteria(256, -90.0);
    assert_eq!(sdt_mode, SdtType::CgSdt);

    // 2. Buffer too large (1500 bytes > 1024 byte threshold) -> Fallback to legacy RRC Resume
    let fallback_buffer = engine.evaluate_sdt_criteria(1500, -90.0);
    assert_eq!(fallback_buffer, SdtType::None);

    // 3. RSRP below threshold (-110 dBm < -105 dBm) -> Fallback to legacy RRC Resume
    let fallback_rsrp = engine.evaluate_sdt_criteria(256, -110.0);
    assert_eq!(fallback_rsrp, SdtType::None);

    // 4. TA expired (or CG not configured) -> Falls back to RA-SDT (2-step RA supported)
    engine.advance_time_ms(3000); // 3000 ms > 2560 ms TA timer
    assert!(!engine.is_ta_valid());
    let ra_2step = engine.evaluate_sdt_criteria(256, -90.0);
    assert_eq!(ra_2step, SdtType::RaSdt2Step);

    // 5. 2-step RA not supported -> Falls back to 4-step RA-SDT
    engine.config.support_2step_ra = false;
    let ra_4step = engine.evaluate_sdt_criteria(256, -90.0);
    assert_eq!(ra_4step, SdtType::RaSdt4Step);
}

#[test]
fn test_sdt_mac_pdu_multiplexing_and_codec() {
    let ccch_data = vec![0x01, 0x12, 0x34, 0x56, 0x78, 0xCA, 0xFE]; // 7-byte RRCResumeRequest1
    let dtch_data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x42, 0x43, 0x44, 0x45]; // 8-byte user payload
    let bsr_value = Some(1200u32);

    let original_pdu = SdtMacPdu::new(ccch_data.clone(), dtch_data.clone(), bsr_value);

    // Serialize to binary wire bytes
    let wire_bytes = original_pdu.serialize();
    assert!(wire_bytes.len() > 15);

    // Parse back from wire bytes
    let parsed_pdu = SdtMacPdu::parse(&wire_bytes).expect("Should successfully parse SDT MAC PDU");
    assert_eq!(parsed_pdu.ccch_sdu, ccch_data);
    assert_eq!(parsed_pdu.dtch_sdu, dtch_data);
    assert_eq!(parsed_pdu.remaining_bsr_bytes, bsr_value);

    // Test malformed / truncated buffer
    let malformed_bytes = vec![0x00, 0x05]; // Incomplete header
    assert!(SdtMacPdu::parse(&malformed_bytes).is_err());
}

#[test]
fn test_sdt_state_machine_and_rrc_release() {
    let mut engine = SdtEngine::new(SdtConfig::default());
    assert_eq!(engine.state, SdtProcedureState::InactiveStandby);

    let small_data = vec![0xAA; 128]; // 128 bytes small data

    // 1. Initiate SDT
    let mac_pdu = engine
        .initiate_sdt(&small_data, -95.0)
        .expect("SDT initiation should succeed");
    assert_eq!(mac_pdu.dtch_sdu.len(), 128);

    // State is AwaitingResponse
    match engine.state {
        SdtProcedureState::AwaitingResponse {
            sdt_type,
            transmitted_bytes,
        } => {
            assert_ne!(sdt_type, SdtType::None);
            assert_eq!(transmitted_bytes, 128);
        }
        _ => panic!("Expected AwaitingResponse state"),
    }

    // 2. gNB completes SDT with RRCRelease (with suspendConfig)
    engine
        .handle_network_response(SdtResponseAction::RrcReleaseWithSuspend)
        .expect("RRCRelease should be processed");

    assert_eq!(
        engine.state,
        SdtProcedureState::TerminatedSuccess {
            total_transferred_bytes: 128
        }
    );
    assert_eq!(engine.total_user_bytes_transferred, 128);

    // 3. Reset back to RRC_INACTIVE standby
    engine.reset_to_inactive();
    assert_eq!(engine.state, SdtProcedureState::InactiveStandby);
}

#[test]
fn test_sdt_subsequent_data_and_inactivity_timer() {
    let mut engine = SdtEngine::new(SdtConfig::default());
    let first_packet = vec![0x11; 64];

    // Initial packet
    engine.initiate_sdt(&first_packet, -90.0).unwrap();

    // gNB sends dynamic grant for subsequent data
    engine
        .handle_network_response(SdtResponseAction::DynamicGrant { granted_bytes: 200 })
        .unwrap();

    // Verify SubsequentData state
    match engine.state {
        SdtProcedureState::SubsequentData {
            remaining_inactivity_ms,
            total_transferred_bytes,
        } => {
            assert_eq!(remaining_inactivity_ms, 160);
            assert_eq!(total_transferred_bytes, 64);
        }
        _ => panic!("Expected SubsequentData state"),
    }

    // Transmit subsequent data (128 bytes)
    let second_packet = vec![0x22; 128];
    let pdu2 = engine
        .transmit_subsequent_data(&second_packet)
        .expect("Subsequent data transmission should succeed");
    assert!(!pdu2.is_empty());

    // Advance time by 80 ms (less than 160 ms inactivity timer)
    engine.advance_time_ms(80);
    match engine.state {
        SdtProcedureState::SubsequentData {
            remaining_inactivity_ms,
            total_transferred_bytes,
        } => {
            assert_eq!(remaining_inactivity_ms, 80);
            assert_eq!(total_transferred_bytes, 64 + 128);
        }
        _ => panic!("Expected SubsequentData state"),
    }

    // Advance time by 100 ms (exceeds remaining 80 ms -> timer expires!)
    engine.advance_time_ms(100);

    // SDT should be autonomously concluded successfully
    assert_eq!(
        engine.state,
        SdtProcedureState::TerminatedSuccess {
            total_transferred_bytes: 192
        }
    );
    assert_eq!(engine.total_user_bytes_transferred, 192);
}

#[test]
fn test_sdt_signaling_reduction_and_energy_metrics() {
    let mut engine = SdtEngine::new(SdtConfig::default());
    let payload = vec![0x99; 256];

    engine.initiate_sdt(&payload, -92.0).unwrap();
    engine
        .handle_network_response(SdtResponseAction::RrcReleaseWithSuspend)
        .unwrap();

    let metrics = engine.compute_performance_metrics();
    assert_eq!(metrics.user_data_bytes_transferred, 256);
    assert!(
        metrics.signaling_reduction_percentage > 65.0,
        "SDT must achieve > 65% signaling overhead reduction (got {:.1}%)",
        metrics.signaling_reduction_percentage
    );
    assert!(
        metrics.estimated_energy_saved_mj > 40.0,
        "SDT must provide significant energy savings (got {:.2} mJ)",
        metrics.estimated_energy_saved_mj
    );
}
