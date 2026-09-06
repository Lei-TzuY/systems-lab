//! Comprehensive Integration Tests for 3GPP Rel-17 5G NR RoHC Engine.

use toy_tcpip::nr_rohc_engine::*;

#[test]
fn test_nr_rohc_crc_and_wlsb_algorithms() {
    let test_data = b"ROHC_5G_NR_PDCP_HEADER_COMPRESSION_TEST_DATA";

    // 1. CRC Calculation Verification
    let crc3 = rohc_crc3(test_data);
    let crc7 = rohc_crc7(test_data);
    let crc8 = rohc_crc8(test_data);

    assert!(crc3 < 8, "CRC-3 must fit in 3 bits (0..7)");
    assert!(crc7 < 128, "CRC-7 must fit in 7 bits (0..127)");

    // Deterministic repeatability
    assert_eq!(rohc_crc3(test_data), crc3);
    assert_eq!(rohc_crc7(test_data), crc7);
    assert_eq!(rohc_crc8(test_data), crc8);

    // Bit sensitivity: flipping a single bit must alter CRC
    let mut modified_data = test_data.to_vec();
    modified_data[0] ^= 0x01;
    assert_ne!(rohc_crc3(&modified_data), crc3);
    assert_ne!(rohc_crc7(&modified_data), crc7);
    assert_ne!(rohc_crc8(&modified_data), crc8);

    // 2. W-LSB Encoding and Decoding Verification (RFC 3095 §4.5.1)
    // k = 4 bits, interpretation interval with p = 1: [-1, 14]
    let k = 4;
    let p = 1;

    // Sequential increment
    let v_ref = 10u32;
    let v_next = 11u32;
    let lsb = wlsb_encode(v_next, k);
    assert_eq!(lsb, 11);
    let decoded = wlsb_decode(v_ref, lsb, k, p);
    assert_eq!(decoded, v_next);

    // Wraparound across 15 -> 16
    let v_ref = 15u32;
    let v_next = 16u32;
    let lsb = wlsb_encode(v_next, k);
    assert_eq!(lsb, 0); // 16 & 0xF = 0
    let decoded = wlsb_decode(v_ref, lsb, k, p);
    assert_eq!(decoded, v_next);

    // Large 16-bit sequence number wraparound: 65535 -> 0
    let k_16 = 6;
    let v_ref = 65535u32;
    let v_next = 65536u32;
    let lsb = wlsb_encode(v_next, k_16);
    let decoded = wlsb_decode(v_ref, lsb, k_16, 1);
    assert_eq!(decoded, v_next);
}

#[test]
fn test_nr_rohc_compressor_state_transitions() {
    let mut comp = RohcCompressor::new(RohcMode::BidirectionalOptimistic, 0);
    assert_eq!(comp.state, CompressorState::InitializationRefresh);

    let dummy_packet = UncompressedPacket {
        ip: Ipv4Header {
            version: 4,
            tos: 0,
            total_length: 38,
            ip_id: 100,
            flags_and_offset: 0x4000,
            ttl: 64,
            protocol: 17,
            checksum: 0,
            src_ip: [192, 168, 1, 10],
            dst_ip: [10, 0, 0, 1],
        },
        udp: UdpHeader {
            src_port: 5004,
            dst_port: 5004,
            length: 18,
            checksum: 0,
        },
        rtp: None,
        payload: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    };

    // Promotion threshold is 3 packets
    // Packet 1: IR
    let p1 = comp.compress(&dummy_packet).unwrap();
    assert_eq!(p1[0], 0xFD, "Must be IR packet");
    assert_eq!(comp.state, CompressorState::InitializationRefresh);

    // Packet 2: IR
    let _ = comp.compress(&dummy_packet).unwrap();
    assert_eq!(comp.state, CompressorState::InitializationRefresh);

    // Packet 3: Promotes to First Order (FO)
    let _ = comp.compress(&dummy_packet).unwrap();
    assert_eq!(comp.state, CompressorState::FirstOrder);

    // In FO state: 3 more packets promote to Second Order (SO)
    let _ = comp.compress(&dummy_packet).unwrap();
    let _ = comp.compress(&dummy_packet).unwrap();
    let _ = comp.compress(&dummy_packet).unwrap();
    assert_eq!(comp.state, CompressorState::SecondOrder);

    // Test Feedback-driven state demotions
    // 1. NACK demotes to FirstOrder
    comp.process_feedback(&RohcFeedback {
        feedback_type: FeedbackType::Nack,
        cid: 0,
        acked_sn: Some(10),
    });
    assert_eq!(comp.state, CompressorState::FirstOrder);

    // 2. STATIC-NACK demotes all the way to IR
    comp.process_feedback(&RohcFeedback {
        feedback_type: FeedbackType::StaticNack,
        cid: 0,
        acked_sn: None,
    });
    assert_eq!(comp.state, CompressorState::InitializationRefresh);

    // 3. ACK promotes directly
    comp.process_feedback(&RohcFeedback {
        feedback_type: FeedbackType::Ack,
        cid: 0,
        acked_sn: Some(12),
    });
    assert_eq!(comp.state, CompressorState::FirstOrder);
    comp.process_feedback(&RohcFeedback {
        feedback_type: FeedbackType::Ack,
        cid: 0,
        acked_sn: Some(13),
    });
    assert_eq!(comp.state, CompressorState::SecondOrder);
}

#[test]
fn test_nr_rohc_udp_ip_compression_and_decompression() {
    let mut comp = RohcCompressor::new(RohcMode::BidirectionalOptimistic, 0);
    let mut decomp = RohcDecompressor::new();

    let src_ip = [192, 168, 100, 1];
    let dst_ip = [192, 168, 100, 2];
    let src_port = 8000;
    let dst_port = 9000;
    let payload = b"ROHC_UDP_USER_PAYLOAD_5G".to_vec();

    // Send 10 packets to progress through IR -> FO -> SO
    for i in 0..10 {
        let packet = UncompressedPacket {
            ip: Ipv4Header {
                version: 4,
                tos: 0,
                total_length: (28 + payload.len()) as u16,
                ip_id: 1000 + i as u16,
                flags_and_offset: 0x4000,
                ttl: 64,
                protocol: 17,
                checksum: 0,
                src_ip,
                dst_ip,
            },
            udp: UdpHeader {
                src_port,
                dst_port,
                length: (8 + payload.len()) as u16,
                checksum: 0,
            },
            rtp: None,
            payload: payload.clone(),
        };

        let compressed = comp.compress(&packet).expect("Compression must succeed");

        if comp.state == CompressorState::SecondOrder {
            // In SO state: 1-byte header!
            assert_eq!(
                compressed.len(),
                1 + payload.len(),
                "In SO state, compressed header must be exactly 1 byte (PT-0)"
            );
            assert_eq!(compressed[0] & 0x80, 0x00, "PT-0 format bit 7 must be 0");
        }

        let recovered = decomp
            .decompress(&compressed)
            .expect("Decompression must succeed");

        // Verify lossless reconstruction of IP/UDP headers and payload
        assert_eq!(recovered.ip.src_ip, src_ip);
        assert_eq!(recovered.ip.dst_ip, dst_ip);
        assert_eq!(recovered.ip.protocol, 17);
        assert_eq!(recovered.udp.src_port, src_port);
        assert_eq!(recovered.udp.dst_port, dst_port);
        assert_eq!(recovered.payload, payload);
    }

    assert_eq!(decomp.state, DecompressorState::FullContext);
    assert_eq!(decomp.packets_decompressed, 10);
    assert_eq!(decomp.crc_failures, 0);
    assert!(comp.compression_ratio() > 1.2);
}

#[test]
fn test_nr_rohc_rtp_udp_ip_compression_and_decompression() {
    let mut comp = RohcCompressor::new(RohcMode::BidirectionalOptimistic, 0);
    let mut decomp = RohcDecompressor::new();

    let src_ip = [10, 20, 30, 40];
    let dst_ip = [10, 20, 30, 50];
    let src_port = 5004;
    let dst_port = 5004;
    let ssrc = 0xA1B2_C3D4;
    let voice_payload = vec![0x33u8; 32]; // 32 bytes AMR-WB voice frame

    let mut timestamp: u32 = 160000;
    let ts_stride = 160; // 20ms at 8kHz

    for i in 0..12 {
        let sn = 1000 + i as u16;
        let rtp = RtpHeader {
            version: 2,
            padding: false,
            extension: false,
            marker: i == 0,
            payload_type: 96,
            sequence_number: sn,
            timestamp,
            ssrc,
        };

        let packet = UncompressedPacket {
            ip: Ipv4Header {
                version: 4,
                tos: 0xB8, // DiffServ EF (Expedited Forwarding for voice)
                total_length: (40 + voice_payload.len()) as u16,
                ip_id: 500 + i as u16,
                flags_and_offset: 0x4000,
                ttl: 64,
                protocol: 17,
                checksum: 0,
                src_ip,
                dst_ip,
            },
            udp: UdpHeader {
                src_port,
                dst_port,
                length: (20 + voice_payload.len()) as u16,
                checksum: 0,
            },
            rtp: Some(rtp),
            payload: voice_payload.clone(),
        };

        let compressed = comp.compress(&packet).expect("Compression must succeed");

        if comp.state == CompressorState::SecondOrder {
            // Uncompressed header was 40 bytes (IP:20 + UDP:8 + RTP:12)
            // In SO state, compressed header is reduced to 1 BYTE (PT-0)!
            assert_eq!(
                compressed.len(),
                1 + voice_payload.len(),
                "VoNR RTP/UDP/IP header must compress to a single byte in SO state!"
            );
        }

        let recovered = decomp
            .decompress(&compressed)
            .expect("Decompression must succeed");

        assert_eq!(recovered.ip.src_ip, src_ip);
        assert_eq!(recovered.ip.dst_ip, dst_ip);
        assert_eq!(recovered.udp.src_port, src_port);
        assert_eq!(recovered.payload, voice_payload);

        let rec_rtp = recovered.rtp.expect("RTP header must be reconstructed");
        assert_eq!(rec_rtp.sequence_number, sn);
        assert_eq!(rec_rtp.timestamp, timestamp);
        assert_eq!(rec_rtp.ssrc, ssrc);

        timestamp += ts_stride;
    }

    assert_eq!(decomp.packets_decompressed, 12);
    assert_eq!(decomp.crc_failures, 0);
    assert!(comp.compression_ratio() > 1.5);
}

#[test]
fn test_nr_rohc_feedback_and_loss_recovery() {
    let mut comp = RohcCompressor::new(RohcMode::BidirectionalOptimistic, 0);
    let mut decomp = RohcDecompressor::new();

    let dummy_packet = UncompressedPacket {
        ip: Ipv4Header {
            version: 4,
            tos: 0,
            total_length: 38,
            ip_id: 100,
            flags_and_offset: 0x4000,
            ttl: 64,
            protocol: 17,
            checksum: 0,
            src_ip: [192, 168, 1, 1],
            dst_ip: [192, 168, 1, 2],
        },
        udp: UdpHeader {
            src_port: 1234,
            dst_port: 5678,
            length: 18,
            checksum: 0,
        },
        rtp: None,
        payload: vec![0xAA; 10],
    };

    // Establish context and promote to SO
    for _ in 0..6 {
        let compressed = comp.compress(&dummy_packet).unwrap();
        let _ = decomp.decompress(&compressed).unwrap();
    }
    assert_eq!(comp.state, CompressorState::SecondOrder);
    assert_eq!(decomp.state, DecompressorState::FullContext);

    // Simulate corrupted packet over radio channel in SO state
    let mut corrupted = comp.compress(&dummy_packet).unwrap();
    corrupted[0] ^= 0x07; // Corrupt CRC-3 field

    let err = decomp.decompress(&corrupted);
    assert!(err.is_err(), "CRC error must be detected");
    assert_eq!(decomp.crc_failures, 1);

    // Decompressor generates NACK feedback
    let feedback = decomp.generate_feedback(0, FeedbackType::Nack);
    assert_eq!(feedback.feedback_type, FeedbackType::Nack);

    // Compressor receives NACK and downgrades to FirstOrder (FO)
    comp.process_feedback(&feedback);
    assert_eq!(comp.state, CompressorState::FirstOrder);

    // Next compressed packet is Type-1 with richer synchronization bits
    let recover_pkt = comp.compress(&dummy_packet).unwrap();
    assert_eq!(
        recover_pkt[0] & 0xC0,
        0x80,
        "Type-1 packet must have leading bits 10"
    );

    // Decompressor successfully recovers context
    let recovered = decomp
        .decompress(&recover_pkt)
        .expect("Decompressor must recover using Type-1 packet");
    assert_eq!(recovered.payload, dummy_packet.payload);
}
