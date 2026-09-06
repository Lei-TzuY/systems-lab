use toy_tcpip::gtpu_loss_telemetry::GtpuLossTelemetryEngine;

#[test]
fn test_gtpu_packet_loss_measurement_lifecycle() {
    let mut upf_origin = GtpuLossTelemetryEngine::new();
    let mut upf_reflector = GtpuLossTelemetryEngine::new();

    let session_id = 0x88001122;
    let qfi = 5;

    // Simulate Origin transmitting 5000 frames
    for _ in 0..5000 {
        upf_origin.record_tx_packet(session_id, qfi);
    }

    // Simulate Reflector receiving 4950 frames (50 lost in forward path)
    for _ in 0..4950 {
        upf_reflector.record_rx_packet(session_id, qfi);
    }

    // Origin generates LMM query
    let lmm = upf_origin.create_lmm(session_id, qfi, 1_000_000);
    assert_eq!(lmm.tx_fc_f, 5000);
    assert!(!lmm.is_reply);

    // Reflector handles LMM and returns LMR response
    let lmr = upf_reflector.handle_lmm_as_reflector(&lmm, 1_002_000);
    assert!(lmr.is_reply);
    assert_eq!(lmr.tx_fc_f, 5000);
    assert_eq!(lmr.rx_fc_f, 4950);

    // Origin processes LMR and calculates dual-ended loss
    let result = upf_origin.process_lmr(&lmr);
    assert_eq!(result.session_id, session_id);
    assert_eq!(result.qfi, qfi);
    assert_eq!(result.forward_tx_count, 5000);
    assert_eq!(result.forward_rx_count, 4950);
    assert_eq!(result.far_end_loss_frames, 50);
    // 50 / 5000 = 1.00% = 100 basis points
    assert_eq!(result.far_end_loss_ratio_bp, 100);
    assert_eq!(result.near_end_loss_frames, 0);
}
