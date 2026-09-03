//! Integration tests for the eCPRI V2.0 fronthaul transport protocol.

use toy_tcpip::ecpri::{
    ECPRI_COMMON_HEADER_LEN, ECPRI_ETHERTYPE, ECPRI_MSG_DELAY_MEASUREMENT, ECPRI_MSG_IQ_DATA,
    ECPRI_MSG_RT_CONTROL, ECPRI_OWD_PAYLOAD_MIN_LEN, ECPRI_UDP_PORT, EcpriCommonHeader,
    EcpriDelayAction, EcpriDelayMeasurement, EcpriError, EcpriIqReassembler, EcpriMessage,
    EcpriMessageType, EcpriOwdEngine, EcpriPacket, EcpriSeqId, EcpriTimestamp, IqReassemblyEvent,
    OwdEvent, estimate_link_asymmetry_ns,
};

#[test]
fn test_ecpri_common_header_round_trip_and_transport_constants() {
    assert_eq!(ECPRI_ETHERTYPE, 0xAEFE);
    assert_eq!(ECPRI_UDP_PORT, 5391);

    let header = EcpriCommonHeader::new(ECPRI_MSG_IQ_DATA, 260);
    let bytes = header.serialize();

    // Revision 1 in the top nibble, reserved bits and C bit clear.
    assert_eq!(bytes[0], 0x10);
    assert_eq!(bytes[1], 0x00);
    assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]), 260);

    let parsed = EcpriCommonHeader::parse(&bytes).unwrap();
    assert_eq!(parsed, header);
    assert!(!parsed.concatenated);

    // 4 + 260 = 264 is already 4-byte aligned.
    assert_eq!(parsed.aligned_len(), 264);
    // 4 + 21 = 25 rounds up to 28.
    assert_eq!(
        EcpriCommonHeader::new(ECPRI_MSG_RT_CONTROL, 21).aligned_len(),
        28
    );

    // Reserved bits set on the wire must be ignored, the C bit must not.
    let with_reserved = EcpriCommonHeader::parse(&[0x1F, 0x02, 0x00, 0x08]).unwrap();
    assert_eq!(with_reserved.protocol_revision, 1);
    assert!(with_reserved.concatenated);
    assert_eq!(with_reserved.message_type, ECPRI_MSG_RT_CONTROL);

    // Only revision 1 exists today.
    assert_eq!(
        EcpriCommonHeader::parse(&[0x20, 0x00, 0x00, 0x00]),
        Err(EcpriError::UnsupportedRevision(2))
    );
    assert_eq!(
        EcpriCommonHeader::parse(&[0x10, 0x00]),
        Err(EcpriError::HeaderTooShort(2))
    );
}

#[test]
fn test_ecpri_iq_data_framing_and_seq_id_bit_layout() {
    let samples = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
    let message = EcpriMessage::IqData {
        pc_id: 0x0102,
        seq_id: EcpriSeqId::fragment(0x7B, 3, false),
        samples: samples.clone(),
    };
    let wire = EcpriPacket::new(message.clone()).serialize();

    // 4-byte common header + PC_ID + SEQ_ID + 6 IQ bytes.
    assert_eq!(wire.len(), ECPRI_COMMON_HEADER_LEN + 4 + samples.len());
    assert_eq!(&wire[0..4], &[0x10, 0x00, 0x00, 0x0A]);
    assert_eq!(&wire[4..6], &[0x01, 0x02]); // PC_ID
    // SEQ_ID: sequence 0x7B, E bit clear, subsequence 3.
    assert_eq!(&wire[6..8], &[0x7B, 0x03]);

    let parsed = EcpriPacket::parse(&wire).unwrap();
    assert_eq!(parsed.message, message);
    assert_eq!(parsed.header.payload_size, 10);

    // The E bit is the MSB of the second SEQ_ID byte.
    let last = EcpriSeqId::fragment(0x7B, 3, true);
    assert_eq!(last.serialize(), [0x7B, 0x83]);
    assert_eq!(EcpriSeqId::parse([0x7B, 0x83]), last);
    assert_eq!(EcpriSeqId::single(9).serialize(), [0x09, 0x80]);

    // Ethernet pads short frames: trailing bytes past ecpriPayloadSize are not payload.
    let mut padded = wire.clone();
    padded.extend_from_slice(&[0u8; 46]);
    assert_eq!(EcpriPacket::parse(&padded).unwrap().message, message);

    // A payload size larger than the buffer is a truncated capture.
    let mut truncated = wire.clone();
    truncated.truncate(wire.len() - 2);
    assert_eq!(
        EcpriPacket::parse(&truncated),
        Err(EcpriError::PayloadTruncated {
            declared: 10,
            available: 8
        })
    );

    // PC_ID + SEQ_ID are mandatory.
    assert_eq!(
        EcpriMessage::parse_payload(ECPRI_MSG_IQ_DATA, &[0x01, 0x02, 0x03]),
        Err(EcpriError::PayloadTooShort {
            message_type: ECPRI_MSG_IQ_DATA,
            need: 4,
            got: 3
        })
    );
}

#[test]
fn test_ecpri_message_concatenation_alignment_and_c_bit() {
    // A 5-byte control payload forces 3 padding bytes before the next message.
    let control = EcpriMessage::RealTimeControl {
        rtc_id: 0x0007,
        seq_id: EcpriSeqId::single(1),
        data: vec![0x55],
    };
    let iq = EcpriMessage::IqData {
        pc_id: 0x0020,
        seq_id: EcpriSeqId::single(2),
        samples: vec![0x11, 0x22, 0x33, 0x44],
    };
    let sync = EcpriMessage::DelayMeasurement(EcpriDelayMeasurement::new(
        7,
        EcpriDelayAction::Request,
        EcpriTimestamp::new(1_600_000_000, 123_456_789),
        1_000,
    ));

    let pdu =
        EcpriPacket::serialize_concatenated(&[control.clone(), iq.clone(), sync.clone()]).unwrap();

    // Message 1: header + 5 payload bytes = 9, padded to 12.
    assert_eq!(pdu[0] & 0x01, 1, "C bit set on the first message");
    assert_eq!(&pdu[9..12], &[0, 0, 0], "alignment padding is zero filled");
    // Message 2 starts on the 4-byte boundary at offset 12, C bit still set.
    assert_eq!(pdu[12] & 0x01, 1);
    // Message 3 (offset 24) is the last: C bit clear.
    assert_eq!(pdu[24] & 0x01, 0);
    assert_eq!(pdu[25], ECPRI_MSG_DELAY_MEASUREMENT);
    assert_eq!(
        pdu.len(),
        12 + 12 + ECPRI_COMMON_HEADER_LEN + ECPRI_OWD_PAYLOAD_MIN_LEN
    );

    let packets = EcpriPacket::parse_concatenated(&pdu).unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[0].message, control);
    assert_eq!(packets[1].message, iq);
    assert_eq!(packets[2].message, sync);
    assert!(packets[0].header.concatenated);
    assert!(packets[1].header.concatenated);
    assert!(!packets[2].header.concatenated);

    // Ethernet padding after the final message must not be parsed as a fourth message.
    let mut padded = pdu.clone();
    padded.extend_from_slice(&[0u8; 8]);
    assert_eq!(EcpriPacket::parse_concatenated(&padded).unwrap().len(), 3);

    // A C bit with nothing behind it is a malformed PDU, not a silent truncation.
    let mut dangling = EcpriPacket::serialize_concatenated(&[iq.clone(), iq.clone()]).unwrap();
    dangling.truncate(12);
    assert_eq!(
        EcpriPacket::parse_concatenated(&dangling),
        Err(EcpriError::MisalignedConcatenation(12))
    );
}

#[test]
fn test_ecpri_one_way_delay_measurement_one_step() {
    let mut o_ru = EcpriOwdEngine::new("O-RU-1", 0);
    // The O-DU compensates 250 ns of its own transmit path (SerDes + PHY).
    let mut o_du = EcpriOwdEngine::new("O-DU-1", 250);

    let t1 = 5_000_000_000i128; // O-DU transmit timestamp
    let request = o_du.build_request(t1);

    // Wire encoding: measurement ID, action 0x00, 10-byte timestamp, 8-byte compensation.
    let wire = request.serialize();
    assert_eq!(wire[1], ECPRI_MSG_DELAY_MEASUREMENT);
    assert_eq!(wire[4], 1, "first measurement ID");
    assert_eq!(wire[5], 0x00, "action type Request");
    assert_eq!(u16::from_be_bytes([wire[2], wire[3]]), 20);

    let decoded = EcpriPacket::parse(&wire).unwrap();
    let EcpriMessage::DelayMeasurement(dm) = &decoded.message else {
        panic!("expected a delay measurement message");
    };
    assert_eq!(dm.timestamp, EcpriTimestamp::new(5, 0));
    // Compensation is carried in correctionField units (nanoseconds x 2^16).
    assert_eq!(dm.compensation_value, 250 << 16);
    assert_eq!(dm.compensation_ns(), 250);

    // The O-RU stamps reception 5.75 us later.
    let t2 = t1 + 5_750;
    let event = o_ru.on_receive(&decoded, t2).unwrap();
    let OwdEvent::Completed(result) = event else {
        panic!("a one-step Request completes immediately");
    };
    // t_D = (t2 - t1) - compensation = 5750 - 250 = 5500 ns.
    assert_eq!(result.one_way_delay_ns, 5_500);
    assert!(!result.two_step);
    assert_eq!(o_ru.completed.len(), 1);
    assert_eq!(o_ru.pending_count(), 0);
    assert_eq!(o_ru.average_one_way_delay_ns(), Some(5_500));
}

#[test]
fn test_ecpri_two_step_follow_up_and_remote_request() {
    let mut o_ru = EcpriOwdEngine::new("O-RU-2", 0);
    let mut o_du = EcpriOwdEngine::new("O-DU-2", 100);

    // Two-step: the Request leaves before hardware reports its precise egress time.
    let request = o_du.build_request_with_follow_up();
    let EcpriMessage::DelayMeasurement(req_dm) = &request.message else {
        panic!("expected a delay measurement message");
    };
    let measurement_id = req_dm.measurement_id;
    assert_eq!(req_dm.action, EcpriDelayAction::RequestWithFollowUp);
    // A Request with Follow_Up must transmit zeroed timestamp and compensation fields.
    assert!(req_dm.timestamp.is_zero());
    assert_eq!(req_dm.compensation_value, 0);

    let t2 = 9_000_004_000i128;
    assert_eq!(
        o_ru.on_receive(&request, t2).unwrap(),
        OwdEvent::AwaitingFollowUp { measurement_id }
    );
    assert_eq!(o_ru.pending_count(), 1);
    assert!(o_ru.completed.is_empty());

    // The Follow_Up carries the precise t1 captured by the transmit hardware.
    let t1 = 9_000_000_000i128;
    let follow_up = o_du.build_follow_up(measurement_id, t1);
    let event = o_ru
        .on_receive(
            &EcpriPacket::parse(&follow_up.serialize()).unwrap(),
            t2 + 999_999,
        )
        .unwrap();
    let OwdEvent::Completed(result) = event else {
        panic!("the Follow_Up completes the measurement");
    };
    // t2 is the arrival time of the Request, not of the Follow_Up: 4000 - 100 = 3900 ns.
    assert_eq!(result.t2_ns, t2);
    assert_eq!(result.one_way_delay_ns, 3_900);
    assert!(result.two_step);
    assert_eq!(o_ru.pending_count(), 0);

    // A Follow_Up whose Request was lost is discarded, not applied to another measurement.
    let orphan = o_du.build_follow_up(200, t1);
    assert_eq!(
        o_ru.on_receive(&orphan, t2).unwrap(),
        OwdEvent::OrphanFollowUp {
            measurement_id: 200
        }
    );
    assert_eq!(o_ru.orphan_follow_ups, 1);
    assert_eq!(o_ru.completed.len(), 1);

    // Remote Request: the O-RU asks the O-DU to measure the reverse direction.
    let remote = o_ru.build_remote_request(false);
    let EcpriMessage::DelayMeasurement(remote_dm) = &remote.message else {
        panic!("expected a delay measurement message");
    };
    assert!(remote_dm.action.is_remote_request());
    assert!(remote_dm.timestamp.is_zero());

    let OwdEvent::RemoteRequest {
        measurement_id: remote_id,
        expects_follow_up,
    } = o_du.on_receive(&remote, 0).unwrap()
    else {
        panic!("a Remote Request never completes a measurement itself");
    };
    assert!(!expects_follow_up);
    assert!(o_du.completed.is_empty());

    // The O-DU answers with a Response carrying its own t1; the O-RU then completes.
    let response = o_du.build_response(remote_id, 9_100_000_000);
    let OwdEvent::Completed(reverse) = o_ru.on_receive(&response, 9_100_004_500).unwrap() else {
        panic!("a Response completes like a one-step Request");
    };
    assert_eq!(reverse.one_way_delay_ns, 4_400); // 4500 - 100 compensation

    // Forward 3900 ns vs reverse 4400 ns leaves 250 ns of correction per direction.
    assert_eq!(
        estimate_link_asymmetry_ns(result.one_way_delay_ns, reverse.one_way_delay_ns),
        -250
    );
}

#[test]
fn test_ecpri_delay_measurement_wire_validation_and_padding() {
    // Dummy bytes inflate the frame without changing the measurement fields.
    let padded = EcpriDelayMeasurement::new(
        42,
        EcpriDelayAction::Response,
        EcpriTimestamp::new(12, 999_999_999),
        0,
    )
    .with_dummy_payload(64);
    let wire = EcpriPacket::new(EcpriMessage::DelayMeasurement(padded)).serialize();
    assert_eq!(u16::from_be_bytes([wire[2], wire[3]]), 64);

    let parsed = EcpriPacket::parse(&wire).unwrap();
    let EcpriMessage::DelayMeasurement(dm) = &parsed.message else {
        panic!("expected a delay measurement message");
    };
    assert_eq!(dm.measurement_id, 42);
    assert_eq!(dm.timestamp, EcpriTimestamp::new(12, 999_999_999));
    assert_eq!(dm.dummy_bytes, 44);

    // Action types above 0x05 are undefined.
    let mut bad_action = wire.clone();
    bad_action[5] = 0x09;
    assert_eq!(
        EcpriPacket::parse(&bad_action),
        Err(EcpriError::UnsupportedActionType(0x09))
    );

    // A nanoseconds field of one second or more is malformed.
    let mut bad_ts = wire.clone();
    bad_ts[12..16].copy_from_slice(&1_000_000_000u32.to_be_bytes());
    assert_eq!(
        EcpriPacket::parse(&bad_ts),
        Err(EcpriError::InvalidTimestamp(1_000_000_000))
    );

    // The 20 mandatory payload bytes must be present.
    assert_eq!(
        EcpriMessage::parse_payload(ECPRI_MSG_DELAY_MEASUREMENT, &[0u8; 19]),
        Err(EcpriError::PayloadTooShort {
            message_type: ECPRI_MSG_DELAY_MEASUREMENT,
            need: 20,
            got: 19
        })
    );

    // The 48-bit seconds field wraps rather than corrupting neighbouring fields.
    let wide = EcpriTimestamp::new(0xFFFF_1234_5678_9ABC, 5);
    let round_tripped = EcpriTimestamp::parse(&wide.serialize()).unwrap();
    assert_eq!(round_tripped.seconds, 0x1234_5678_9ABC);
    assert_eq!(round_tripped.nanoseconds, 5);

    // Feeding a non-Type-5 message to the delay engine is rejected.
    let mut engine = EcpriOwdEngine::new("O-DU-3", 0);
    let iq = EcpriPacket::new(EcpriMessage::IqData {
        pc_id: 1,
        seq_id: EcpriSeqId::single(0),
        samples: vec![0; 8],
    });
    assert_eq!(
        engine.on_receive(&iq, 0),
        Err(EcpriError::NotADelayMeasurement(ECPRI_MSG_IQ_DATA))
    );
}

#[test]
fn test_ecpri_iq_subsequence_reassembly_and_gap_handling() {
    let mut reassembler = EcpriIqReassembler::new(0x0040);

    let fragment = |seq: u8, sub: u8, last: bool, byte: u8| EcpriMessage::IqData {
        pc_id: 0x0040,
        seq_id: EcpriSeqId::fragment(seq, sub, last),
        samples: vec![byte; 4],
    };

    assert_eq!(
        reassembler.accept(&fragment(1, 0, false, 0xA0)),
        IqReassemblyEvent::Buffered {
            sequence_id: 1,
            buffered_len: 4
        }
    );
    assert_eq!(
        reassembler.accept(&fragment(1, 1, false, 0xA1)),
        IqReassemblyEvent::Buffered {
            sequence_id: 1,
            buffered_len: 8
        }
    );
    // The E bit terminates the burst.
    assert_eq!(
        reassembler.accept(&fragment(1, 2, true, 0xA2)),
        IqReassemblyEvent::BurstComplete {
            sequence_id: 1,
            payload_len: 12
        }
    );
    assert_eq!(reassembler.completed_bursts.len(), 1);
    assert_eq!(reassembler.completed_bursts[0].len(), 12);
    assert_eq!(reassembler.completed_bursts[0][8], 0xA2);
    assert_eq!(reassembler.buffered_len(), 0);

    // A burst may not start in the middle of a subsequence.
    assert!(matches!(
        reassembler.accept(&fragment(2, 1, false, 0xB0)),
        IqReassemblyEvent::Discarded { .. }
    ));
    assert_eq!(reassembler.discarded_fragments, 1);

    // A lost middle fragment is detected as a subsequence gap.
    reassembler.accept(&fragment(3, 0, false, 0xC0));
    assert_eq!(
        reassembler.accept(&fragment(3, 2, true, 0xC2)),
        IqReassemblyEvent::Discarded {
            reason: "subsequence gap or duplicate"
        }
    );
    assert_eq!(reassembler.completed_bursts.len(), 1);

    // The sender moving to a new sequence abandons the incomplete burst.
    assert_eq!(
        reassembler.accept(&fragment(4, 0, true, 0xD0)),
        IqReassemblyEvent::BurstComplete {
            sequence_id: 4,
            payload_len: 4
        }
    );
    assert_eq!(reassembler.aborted_bursts, 1);
    assert_eq!(reassembler.completed_bursts.len(), 2);
    assert_eq!(reassembler.completed_bursts[1], vec![0xD0; 4]);

    // Fragments of another antenna-carrier flow are not mixed in.
    let foreign = EcpriMessage::IqData {
        pc_id: 0x0041,
        seq_id: EcpriSeqId::single(0),
        samples: vec![0xEE; 4],
    };
    assert_eq!(
        reassembler.accept(&foreign),
        IqReassemblyEvent::Discarded {
            reason: "PC_ID does not belong to this flow"
        }
    );
    assert_eq!(reassembler.completed_bursts.len(), 2);
}

#[test]
fn test_ecpri_message_type_classification() {
    assert_eq!(EcpriMessageType::from_u8(0x00), EcpriMessageType::IqData);
    assert_eq!(
        EcpriMessageType::from_u8(0x05),
        EcpriMessageType::OneWayDelayMeasurement
    );
    assert_eq!(EcpriMessageType::from_u8(0x09), EcpriMessageType::Iwf(0x09));
    assert_eq!(
        EcpriMessageType::from_u8(0x0C),
        EcpriMessageType::Reserved(0x0C)
    );
    assert_eq!(
        EcpriMessageType::from_u8(0x40),
        EcpriMessageType::VendorSpecific(0x40)
    );
    assert_eq!(EcpriMessageType::from_u8(0x3F).code(), 0x3F);
    assert_eq!(
        EcpriMessageType::from_u8(0x02).name(),
        "Real-Time Control Data"
    );

    // Only user plane, real-time control and sync traffic are latency critical.
    assert!(EcpriMessageType::IqData.is_time_critical());
    assert!(EcpriMessageType::RealTimeControl.is_time_critical());
    assert!(EcpriMessageType::OneWayDelayMeasurement.is_time_critical());
    assert!(!EcpriMessageType::GenericDataTransfer.is_time_critical());
    assert!(!EcpriMessageType::EventIndication.is_time_critical());

    // Unmodelled message types survive a decode/encode round trip byte for byte.
    let raw = EcpriMessage::Raw {
        message_type: 0x04,
        payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };
    let wire = EcpriPacket::new(raw.clone()).serialize();
    assert_eq!(EcpriPacket::parse(&wire).unwrap().message, raw);

    // Event Indication exposes its fault reporting header.
    let event = EcpriMessage::EventIndication {
        event_id: 0x11,
        event_type: 0x00,
        sequence_number: 7,
        element_count: 1,
        elements: vec![0x00, 0x01, 0x02, 0x03],
    };
    let wire = EcpriPacket::new(event.clone()).serialize();
    assert_eq!(wire[1], 0x07);
    assert_eq!(EcpriPacket::parse(&wire).unwrap().message, event);
}
