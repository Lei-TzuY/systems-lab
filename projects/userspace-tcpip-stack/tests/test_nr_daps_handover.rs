//! Integration tests for 3GPP Rel-17 5G NR Dual Active Protocol Stack (DAPS) Handover Engine.

use toy_tcpip::nr_daps_handover::{
    DapsCipherAlg, DapsEngine, DapsError, DapsFailureReason, DapsIntegrityAlg, DapsLeg, DapsPdu,
    DapsPowerManager, DapsSecurityContext, DapsSnSize, DapsState, DapsUlChannel,
};

#[test]
fn test_daps_state_transitions_happy_path() {
    let src_key = [0x11; 16];
    let tgt_key = [0x22; 16];

    let src_sec = DapsSecurityContext::new(
        src_key,
        DapsCipherAlg::Nea2,
        DapsIntegrityAlg::Nia2,
        1, // Bearer ID 1
    );

    let tgt_sec = DapsSecurityContext::new(tgt_key, DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);

    let mut engine = DapsEngine::new(DapsSnSize::Sn12Bits, src_sec, 23.0, 100);
    assert_eq!(engine.state, DapsState::SourceOnly);

    // 1. Send UL SDU on Source leg before handover
    let pdu1 = engine.send_ul_sdu(1, b"Source Packet 1".to_vec()).unwrap();
    assert_eq!(pdu1.leg, DapsLeg::Source);
    assert_eq!(pdu1.sn, 0);
    assert_eq!(engine.telemetry.source_tx_pdus, 1);
    assert_eq!(engine.telemetry.target_tx_pdus, 0);

    // 2. RRCReconfiguration with daps-Config received at t = 1000 ms
    engine.configure_target(tgt_sec.clone(), 1000).unwrap();
    assert_eq!(engine.state, DapsState::DapsConfigured);

    // 3. Initiate Target RACH
    engine.start_target_rach().unwrap();
    assert_eq!(engine.state, DapsState::TargetRachAttempting);

    // During RACH, UL data still transmits to Source leg
    let pdu2 = engine.send_ul_sdu(2, b"Source Packet 2".to_vec()).unwrap();
    assert_eq!(pdu2.leg, DapsLeg::Source);
    assert_eq!(pdu2.sn, 1);

    // 4. Target RACH success -> Uplink switched to Target leg
    engine.target_rach_success().unwrap();
    assert_eq!(engine.state, DapsState::UplinkSwitched);

    // Subsequent UL data immediately transmits to Target leg
    let pdu3 = engine.send_ul_sdu(3, b"Target Packet 3".to_vec()).unwrap();
    assert_eq!(pdu3.leg, DapsLeg::Target);
    assert_eq!(pdu3.sn, 2);
    assert_eq!(engine.telemetry.target_tx_pdus, 1);

    // 5. Enter full DualActive state
    engine.enter_dual_active().unwrap();
    assert_eq!(engine.state, DapsState::DualActive);

    // 6. Release Source cell at t = 1045 ms
    engine.release_source(1045).unwrap();
    assert_eq!(engine.state, DapsState::SourceReleased);

    // Verify performance telemetry: 0 ms interruption, 45 ms duration
    assert_eq!(engine.telemetry.interruption_duration_ms, 0);
    assert_eq!(engine.telemetry.handover_duration_ms, 45);
    assert!(!engine.telemetry.fallback_occurred);
}

#[test]
fn test_zero_interruption_downlink_reordering_and_deduplication() {
    let src_key = [0xAA; 16];
    let tgt_key = [0xBB; 16];

    let src_sec = DapsSecurityContext::new(src_key, DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 2);

    let tgt_sec = DapsSecurityContext::new(tgt_key, DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 2);

    let mut engine = DapsEngine::new(DapsSnSize::Sn12Bits, src_sec.clone(), 23.0, 100);
    engine.configure_target(tgt_sec.clone(), 100).unwrap();
    engine.start_target_rach().unwrap();
    engine.target_rach_success().unwrap();
    engine.enter_dual_active().unwrap();

    // Helper to generate valid encrypted PDU for a leg
    let create_pdu = |leg: DapsLeg, sec: &DapsSecurityContext, count: u32, data: &[u8]| {
        let ciphered = sec.process_cipher(count, data);
        let mac = sec.compute_mac(count, &ciphered);
        DapsPdu {
            leg,
            sn: count & 0xFFF,
            count,
            payload: ciphered,
            mac,
        }
    };

    // Receive COUNT 0 from Source leg -> should deliver immediately
    let pdu0 = create_pdu(DapsLeg::Source, &src_sec, 0, b"Payload 0");
    let deliv0 = engine.receive_dl_pdu(pdu0).unwrap();
    assert_eq!(deliv0.len(), 1);
    assert_eq!(deliv0[0], b"Payload 0");

    // Receive COUNT 2 from Target leg -> should be buffered awaiting missing COUNT 1
    let pdu2 = create_pdu(DapsLeg::Target, &tgt_sec, 2, b"Payload 2");
    let deliv2 = engine.receive_dl_pdu(pdu2).unwrap();
    assert_eq!(deliv2.len(), 0); // Out of order, buffered

    // Receive COUNT 1 from Source leg -> should trigger delivery of both COUNT 1 and COUNT 2
    let pdu1 = create_pdu(DapsLeg::Source, &src_sec, 1, b"Payload 1");
    let deliv1 = engine.receive_dl_pdu(pdu1).unwrap();
    assert_eq!(deliv1.len(), 2);
    assert_eq!(deliv1[0], b"Payload 1");
    assert_eq!(deliv1[1], b"Payload 2");

    // DUPLICATE ARRIVAL: Target leg delivers duplicate copy of COUNT 1 (forwarded over Xn)
    let pdu1_dup = create_pdu(DapsLeg::Target, &tgt_sec, 1, b"Payload 1");
    let deliv_dup1 = engine.receive_dl_pdu(pdu1_dup).unwrap();
    assert_eq!(deliv_dup1.len(), 0); // Suppressed as duplicate

    // DUPLICATE ARRIVAL: Source leg delivers duplicate copy of COUNT 2
    let pdu2_dup = create_pdu(DapsLeg::Source, &src_sec, 2, b"Payload 2");
    let deliv_dup2 = engine.receive_dl_pdu(pdu2_dup).unwrap();
    assert_eq!(deliv_dup2.len(), 0); // Suppressed as duplicate

    // Telemetry check
    assert_eq!(engine.telemetry.duplicates_suppressed, 2);
    assert_eq!(engine.telemetry.total_delivered_sdus, 3);
}

#[test]
fn test_uplink_switching_and_buffer_forwarding() {
    let src_sec =
        DapsSecurityContext::new([0x33; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);
    let tgt_sec =
        DapsSecurityContext::new([0x44; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);

    let mut engine = DapsEngine::new(DapsSnSize::Sn18Bits, src_sec, 23.0, 100);

    // Transmit 3 packets on Source
    engine.send_ul_sdu(10, b"SDU 10".to_vec()).unwrap();
    engine.send_ul_sdu(11, b"SDU 11".to_vec()).unwrap();
    engine.send_ul_sdu(12, b"SDU 12".to_vec()).unwrap();
    assert_eq!(engine.pending_ul_count(), 3);

    // Acknowledge SDU 10
    engine.acknowledge_ul_sdu(10);
    assert_eq!(engine.pending_ul_count(), 2);

    // Handover to Target
    engine.configure_target(tgt_sec, 500).unwrap();
    engine.start_target_rach().unwrap();
    engine.target_rach_success().unwrap();

    // SDU 13 sent on Target
    let pdu13 = engine.send_ul_sdu(13, b"SDU 13".to_vec()).unwrap();
    assert_eq!(pdu13.leg, DapsLeg::Target);
    assert_eq!(pdu13.sn, 3); // Continuous sequence number
    assert_eq!(engine.pending_ul_count(), 3); // 11, 12, 13
}

#[test]
fn test_target_failure_and_fallback_to_source() {
    let src_sec =
        DapsSecurityContext::new([0x55; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);
    let tgt_sec =
        DapsSecurityContext::new([0x66; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);

    let mut engine = DapsEngine::new(DapsSnSize::Sn12Bits, src_sec, 23.0, 50); // T304 = 50 ms

    engine.configure_target(tgt_sec, 100).unwrap();
    engine.start_target_rach().unwrap();

    // Advance 30 ms (less than 50 ms)
    let failure = engine.tick_timer(30);
    assert_eq!(failure, None);
    assert_eq!(engine.state, DapsState::TargetRachAttempting);

    // Advance 25 ms (total 55 ms >= 50 ms) -> T304 expires!
    let failure2 = engine.tick_timer(25);
    assert_eq!(failure2, Some(DapsFailureReason::T304Expiry));
    assert_eq!(engine.state, DapsState::TargetFailureFallback);
    assert!(engine.telemetry.fallback_occurred);
    assert_eq!(
        engine.telemetry.last_failure_reason,
        Some(DapsFailureReason::T304Expiry)
    );

    // Seamless fallback: Source leg continues to transmit without dropping call
    let pdu = engine.send_ul_sdu(20, b"Recovered SDU".to_vec()).unwrap();
    assert_eq!(pdu.leg, DapsLeg::Source);
}

#[test]
fn test_dual_active_power_sharing() {
    let pm = DapsPowerManager::new(23.0); // 23.0 dBm max = 199.53 mW (~200 mW)

    // Case 1: Both within budget (15 dBm = 31.62 mW, 18 dBm = 63.10 mW, sum = 94.72 mW <= 200 mW)
    let (src_p, tgt_p) = pm.allocate_power(
        15.0,
        DapsUlChannel::SourcePusch,
        18.0,
        DapsUlChannel::TargetPusch,
    );
    assert!((src_p - 15.0).abs() < 1e-3);
    assert!((tgt_p - 18.0).abs() < 1e-3);

    // Case 2: Target PRACH (priority 5) vs Source PUSCH (priority 1) exceeding P_CMAX
    // Source requests 20 dBm (100 mW), Target requests 21 dBm (125.89 mW). Sum = 225.89 mW > 200 mW.
    let (src_p2, tgt_p2) = pm.allocate_power(
        20.0,
        DapsUlChannel::SourcePusch,
        21.0,
        DapsUlChannel::TargetPrach,
    );
    // Target PRACH takes precedence: gets full 21.0 dBm (125.89 mW)
    assert!((tgt_p2 - 21.0).abs() < 1e-3);
    // Source PUSCH gets remaining power: 200 - 125.89 = 74.11 mW = 18.70 dBm
    assert!(src_p2 < 20.0);
    assert!((src_p2 - 18.70).abs() < 0.1);

    // Case 3: Source PUCCH (priority 3) vs Target PUSCH (priority 2)
    // Source requests 22 dBm (158.49 mW), Target requests 20 dBm (100 mW). Sum > 200 mW.
    let (src_p3, tgt_p3) = pm.allocate_power(
        22.0,
        DapsUlChannel::SourcePucch,
        20.0,
        DapsUlChannel::TargetPusch,
    );
    // Source PUCCH takes precedence
    assert!((src_p3 - 22.0).abs() < 1e-3);
    // Target PUSCH scaled down: 200 - 158.49 = 41.51 mW = 16.18 dBm
    assert!(tgt_p3 < 20.0);
    assert!((tgt_p3 - 16.18).abs() < 0.1);
}

#[test]
fn test_independent_security_contexts_and_integrity_verification() {
    let src_sec =
        DapsSecurityContext::new([0x77; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);
    let tgt_sec =
        DapsSecurityContext::new([0x88; 16], DapsCipherAlg::Nea2, DapsIntegrityAlg::Nia2, 1);

    let mut engine = DapsEngine::new(DapsSnSize::Sn12Bits, src_sec, 23.0, 100);
    engine.configure_target(tgt_sec, 0).unwrap();
    engine.start_target_rach().unwrap();
    engine.target_rach_success().unwrap();
    engine.enter_dual_active().unwrap();

    // Create a PDU with corrupted MAC
    let bad_pdu = DapsPdu {
        leg: DapsLeg::Source,
        sn: 0,
        count: 0,
        payload: b"Corrupted Data".to_vec(),
        mac: [0xDE, 0xAD, 0xBE, 0xEF],
    };

    let res = engine.receive_dl_pdu(bad_pdu);
    assert_eq!(
        res,
        Err(DapsError::IntegrityCheckFailed {
            leg: DapsLeg::Source,
            count: 0
        })
    );
}

#[test]
fn test_error_formatting_and_display() {
    let err_trans = DapsError::InvalidStateTransition {
        from: DapsState::SourceOnly,
        to: DapsState::DualActive,
    };
    let s = format!("{}", err_trans);
    assert!(s.contains("Invalid DAPS state transition"));

    let err_sec = DapsError::SecurityContextMissing(DapsLeg::Target);
    let s2 = format!("{}", err_sec);
    assert!(s2.contains("Missing security context"));

    let err_buf = DapsError::BufferOverflow {
        capacity: 100,
        requested: 105,
    };
    let s3 = format!("{}", err_buf);
    assert!(s3.contains("DAPS buffer overflow"));

    let err_fail = DapsError::TargetFailure(DapsFailureReason::TargetRachFailure);
    let s4 = format!("{}", err_fail);
    assert!(s4.contains("DAPS target failure"));
}
