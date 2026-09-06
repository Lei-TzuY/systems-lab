//! Integration tests for 3GPP Rel-18 5G-Advanced Extended Reality (XR) & PDU Set Protocol Engine.
//!
//! Tests multi-modal traffic generation, PDU Set header binary encoding/decoding,
//! PSDB delay budget verification, cascading discard propagation, priority multiplexing,
//! and QoE metrics tracking in pure standard Rust.

use toy_tcpip::nr_xr_pdu_set::*;

#[test]
fn test_pdu_set_header_binary_encoding_decoding() {
    let header = PduSetHeader::new(
        1042,      // PSSN
        3,         // PSN
        8,         // PDU Set Size (8 slices)
        false,     // Not end of PDU Set
        6,         // Importance 6 (High / I-Frame)
        5_000_000, // 5.0 seconds in us
        1380,      // 1380 bytes payload
    );

    // Encode to 15-byte wire buffer
    let encoded = PduSetBinaryCodec::encode_header(&header);
    assert_eq!(encoded.len(), PDU_SET_HEADER_SIZE_BYTES);

    // Byte 0: Present (0x80) | Not EOP (0x00) | Importance 6 (0x06) = 0x86
    assert_eq!(encoded[0], 0x86);

    // Byte 1..2: PSSN = 1042 (0x0412)
    assert_eq!(encoded[1], 0x04);
    assert_eq!(encoded[2], 0x12);

    // Byte 3: PSN = 3
    assert_eq!(encoded[3], 3);

    // Byte 4: Set Size = 8
    assert_eq!(encoded[4], 8);

    // Decode header and verify round-trip integrity
    let decoded =
        PduSetBinaryCodec::decode_header(&encoded).expect("PduSetHeader decoding must succeed");

    assert_eq!(decoded.pdu_set_present, true);
    assert_eq!(decoded.end_of_pdu_set, false);
    assert_eq!(decoded.importance, 6);
    assert_eq!(decoded.pssn, 1042);
    assert_eq!(decoded.psn, 3);
    assert_eq!(decoded.pdu_set_size, 8);
    assert_eq!(decoded.generation_ts_us, 5_000_000);
    assert_eq!(decoded.payload_size_bytes, 1380);

    // Test full packet serialize & deserialize
    let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
    let modality = XrModality::VideoFrame {
        frame_type: VideoFrameType::IFrame,
        width: 3840,
        height: 2160,
    };
    let packet = PduSetPacket::new(header, modality.clone(), payload.clone());
    let wire_bytes = packet.serialize();
    assert_eq!(wire_bytes.len(), PDU_SET_HEADER_SIZE_BYTES + payload.len());

    let parsed_packet = PduSetPacket::deserialize(&wire_bytes, modality)
        .expect("PduSetPacket deserialize must succeed");
    assert_eq!(parsed_packet.header.pssn, 1042);
    assert_eq!(parsed_packet.payload, payload);
}

#[test]
fn test_multi_modal_xr_traffic_burst_generation() {
    let mut generator = XrTrafficGenerator::new(XR_REFRESH_RATE_90_HZ, 1400);

    // 1. Generate 6DoF Pose packet
    let pose_pkt = generator.generate_pose(1_000_000, 1.25, -0.50, 1.80);
    assert_eq!(pose_pkt.header.psn, 0);
    assert_eq!(pose_pkt.header.pdu_set_size, 1);
    assert_eq!(pose_pkt.header.end_of_pdu_set, true);
    assert_eq!(pose_pkt.header.importance, 7); // Maximum priority for pose
    assert_eq!(pose_pkt.payload.len(), 24); // 6 x f32

    // 2. Generate 4K I-Frame burst (28 KB -> 20 PDUs of 1400 bytes)
    let iframe_pkts = generator.generate_video_frame(VideoFrameType::IFrame, 28_000, 1_000_100);
    assert_eq!(iframe_pkts.len(), 20);
    let pssn_iframe = iframe_pkts[0].header.pssn;

    for (i, p) in iframe_pkts.iter().enumerate() {
        assert_eq!(p.header.pssn, pssn_iframe);
        assert_eq!(p.header.psn, i as u8);
        assert_eq!(p.header.pdu_set_size, 20);
        assert_eq!(p.header.importance, 6);
        if i == 19 {
            assert_eq!(p.header.end_of_pdu_set, true);
        } else {
            assert_eq!(p.header.end_of_pdu_set, false);
        }
    }

    // 3. Generate 4K P-Frame burst (8.4 KB -> 6 PDUs of 1400 bytes)
    let pframe_pkts = generator.generate_video_frame(VideoFrameType::PFrame, 8_400, 1_011_211);
    assert_eq!(pframe_pkts.len(), 6);
    assert_ne!(pframe_pkts[0].header.pssn, pssn_iframe);
    assert_eq!(pframe_pkts[5].header.end_of_pdu_set, true);
    assert_eq!(pframe_pkts[0].header.importance, 4);

    // 4. Generate Spatial Audio packet
    let audio_pkt = generator.generate_audio(1_000_500, 320);
    assert_eq!(audio_pkt.header.pdu_set_size, 1);
    assert_eq!(audio_pkt.header.end_of_pdu_set, true);
    assert_eq!(audio_pkt.payload.len(), 320);
}

#[test]
fn test_pdu_set_delay_budget_expiration() {
    let budget = PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_PFRAME_US); // 10,000 us (10 ms)
    let gen_ts = 100_000;

    // Inside budget: +5 ms
    assert!(!budget.is_expired(gen_ts, 105_000));
    assert_eq!(budget.remaining_budget_us(gen_ts, 105_000), 5_000);

    // Exact boundary: +10 ms
    assert!(!budget.is_expired(gen_ts, 110_000));
    assert_eq!(budget.remaining_budget_us(gen_ts, 110_000), 0);

    // Exceeded budget: +10.001 ms
    assert!(budget.is_expired(gen_ts, 110_001));
    assert_eq!(budget.remaining_budget_us(gen_ts, 110_001), -1);

    // Heavily expired: +25 ms
    assert!(budget.is_expired(gen_ts, 125_000));
    assert_eq!(budget.remaining_budget_us(gen_ts, 125_000), -15_000);
}

#[test]
fn test_cascading_discard_on_missing_key_slice() {
    let mut discard_mgr = CascadingDiscardManager::new(64);
    let budget = PduSetDelayBudget::new(DEFAULT_PSDB_VIDEO_IFRAME_US); // 15,000 us

    let pssn = 77;
    let gen_ts = 50_000;

    // Construct 4 slices of an I-frame
    let make_slice = |psn: u8, eop: bool| {
        let hdr = PduSetHeader::new(pssn, psn, 4, eop, 6, gen_ts, 1000);
        PduSetPacket::new(
            hdr,
            XrModality::VideoFrame {
                frame_type: VideoFrameType::IFrame,
                width: 3840,
                height: 2160,
            },
            vec![0xAA; 1000],
        )
    };

    let slice0 = make_slice(0, false);
    let slice1 = make_slice(1, false);
    let slice2 = make_slice(2, false);
    let slice3 = make_slice(3, true);

    // Slice 0 arrives on time (+2 ms) -> Accept/Deliver
    let act0 = discard_mgr.process_pdu(&slice0, 52_000, &budget);
    assert_eq!(act0, PduHandlingAction::Deliver);

    // Slice 1 arrives on time (+6 ms) -> Accept/Deliver
    let act1 = discard_mgr.process_pdu(&slice1, 56_000, &budget);
    assert_eq!(act1, PduHandlingAction::Deliver);

    // Slice 2 suffers severe transmission delay (+18 ms > 15 ms PSDB) -> Trigger Cascading Discard
    let act2 = discard_mgr.process_pdu(&slice2, 68_000, &budget);
    assert_eq!(
        act2,
        PduHandlingAction::TriggerCascadingDiscard {
            pssn,
            reason: DiscardReason::DelayBudgetExpired,
        }
    );
    assert!(discard_mgr.is_pssn_discarded(pssn));

    // Slice 3 arrives (+19 ms) -> Must be dropped via CascadingDependencyLost!
    let act3 = discard_mgr.process_pdu(&slice3, 69_000, &budget);
    assert_eq!(
        act3,
        PduHandlingAction::DiscardSingle {
            reason: DiscardReason::CascadingDependencyLost,
        }
    );

    // Verify telemetry
    assert_eq!(discard_mgr.total_accepted_pdus, 2);
    assert_eq!(discard_mgr.total_discarded_pdus, 2);
    assert_eq!(discard_mgr.total_cascading_drops, 1);
    assert!(!discard_mgr.is_pdu_set_complete(pssn));
}

#[test]
fn test_xr_multi_modal_priority_multiplexing() {
    let mut scheduler = XrMultiModalScheduler::new(32);
    let mut generator = XrTrafficGenerator::new(XR_REFRESH_RATE_90_HZ, 1400);

    let base_ts = 1_000_000;

    // Enqueue 4 video slices of a P-Frame
    let video_pkts = generator.generate_video_frame(VideoFrameType::PFrame, 4200, base_ts);
    for pkt in video_pkts {
        scheduler
            .enqueue(pkt, base_ts)
            .expect("Video enqueue must succeed");
    }

    // Enqueue 1 audio packet
    let audio_pkt = generator.generate_audio(base_ts + 100, 200);
    scheduler
        .enqueue(audio_pkt, base_ts + 100)
        .expect("Audio enqueue must succeed");

    // Enqueue 1 ultra-urgent 6DoF pose packet
    let pose_pkt = generator.generate_pose(base_ts + 200, 0.5, 0.2, 1.0);
    scheduler
        .enqueue(pose_pkt, base_ts + 200)
        .expect("Pose enqueue must succeed");

    assert_eq!(scheduler.total_queued_packets(), 5);

    // Schedule next: 6DoF Pose must PREEMPT Video and Audio (Priority 0)
    let first = scheduler
        .schedule_next(base_ts + 300)
        .expect("Must schedule first packet");
    assert!(matches!(first.modality, XrModality::SixDofPose { .. }));

    // Schedule next: Audio must come before Video (Priority 2 before Priority 3)
    let second = scheduler
        .schedule_next(base_ts + 400)
        .expect("Must schedule second packet");
    assert!(matches!(second.modality, XrModality::SpatialAudio { .. }));

    // Schedule next: Video slices follow
    let third = scheduler
        .schedule_next(base_ts + 500)
        .expect("Must schedule third packet");
    assert!(matches!(third.modality, XrModality::VideoFrame { .. }));

    // Test Grant scheduling (remaining 2 video slices fit in 3500-byte grant)
    let grant_pkts = scheduler.schedule_grant(4000, base_ts + 600);
    assert_eq!(grant_pkts.len(), 2);
    assert_eq!(scheduler.total_queued_packets(), 0);
}

#[test]
fn test_qoe_metrics_goodput_and_fsr_calculation() {
    let mut tracker = XrQoeTracker::new();

    // 8 frames succeeded: 25,000 bytes each, 8,500 us latency
    for _ in 0..8 {
        tracker.record_frame_success(25_000, 8_500);
    }

    // 2 frames failed: 5,000 bytes transmitted before dropping, 20,000 bytes saved by cascading drop
    for _ in 0..2 {
        tracker.record_frame_failure(5_000, 20_000);
    }

    // Check Frame Success Rate: 8/10 = 80.0%
    assert!((tracker.frame_success_rate() - 80.0).abs() < 0.001);

    // Check PDU Set Error Rate: 2/10 = 0.20
    assert!((tracker.pdu_set_error_rate() - 0.20).abs() < 0.001);

    // Check average MTP latency: 8,500 us / 1000 = 8.5 ms
    assert!((tracker.average_mtp_latency_ms() - 8.5).abs() < 0.001);

    // Transmitted bytes: 8 * 25,000 + 2 * 5,000 = 210,000 bytes
    // Goodput bytes: 8 * 25,000 = 200,000 bytes
    // Goodput ratio: 200,000 / 210,000 = 95.238%
    assert_eq!(tracker.total_bytes_transmitted, 210_000);
    assert_eq!(tracker.total_goodput_bytes, 200_000);
    assert!((tracker.goodput_ratio() - 95.238).abs() < 0.01);

    // Total bytes saved via cascading discard: 2 * 20,000 = 40,000 bytes
    assert_eq!(tracker.total_cascading_saved_bytes, 40_000);
}

#[test]
fn test_error_handling_and_boundary_cases() {
    // 1. Buffer too short for header
    let short_buf = [0u8; 10];
    let err = PduSetBinaryCodec::decode_header(&short_buf).unwrap_err();
    assert!(matches!(
        err,
        XrError::BufferTooShort {
            needed: 15,
            provided: 10
        }
    ));
    assert!(format!("{}", err).contains("needed 15 bytes, got 10"));

    // 2. Buffer too short for payload
    let hdr = PduSetHeader::new(1, 0, 1, true, 5, 0, 100);
    let mut encoded = PduSetBinaryCodec::encode_header(&hdr).to_vec();
    encoded.extend_from_slice(&[0u8; 20]); // only 20 bytes provided, need 100
    let err_pkt = PduSetPacket::deserialize(
        &encoded,
        XrModality::HapticFeedback {
            actuator_id: 1,
            intensity: 100,
            frequency_hz: 200,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err_pkt,
        XrError::BufferTooShort {
            needed: 115,
            provided: 35
        }
    ));

    // 3. Error formatting
    let err_size = XrError::PduSetSizeMismatch {
        expected: 10,
        got: 5,
    };
    assert!(format!("{}", err_size).contains("expected 10, got 5"));

    let err_q = XrError::QueueFull { capacity: 64 };
    assert!(format!("{}", err_q).contains("capacity 64"));

    let err_mod = XrError::UnknownModality;
    assert_eq!(format!("{}", err_mod), "Unknown or unsupported XR modality");

    // 4. Modality defaults
    let haptic = XrModality::HapticFeedback {
        actuator_id: 0,
        intensity: 255,
        frequency_hz: 150,
    };
    assert_eq!(haptic.default_priority(), 1);
    assert_eq!(haptic.default_importance(), 5);

    // 5. Constants verification
    assert_eq!(XR_REFRESH_RATE_60_HZ, 60);
    assert_eq!(XR_REFRESH_RATE_90_HZ, 90);
    assert_eq!(XR_REFRESH_RATE_120_HZ, 120);
    assert_eq!(XR_FRAME_INTERVAL_60HZ_US, 16_667);
    assert_eq!(XR_FRAME_INTERVAL_90HZ_US, 11_111);
    assert_eq!(XR_FRAME_INTERVAL_120HZ_US, 8_333);
}
