use toy_tcpip::quic::{
    decode_vint, encode_vint, QuicPacket, QUIC_PKT_HANDSHAKE, QUIC_PKT_INITIAL, QUIC_VERSION_1,
};

#[test]
fn test_quic_vint_roundtrips() {
    let numbers = [0, 1, 63, 64, 16383, 16384, 1073741823, 1073741824, 4611686018427387903];
    for &n in &numbers {
        let enc = encode_vint(n);
        let (dec, len) = decode_vint(&enc).unwrap();
        assert_eq!(dec, n);
        assert_eq!(len, enc.len());
    }
}

#[test]
fn test_quic_packet_initial_and_short_headers() {
    // 1. Initial Handshake Packet
    let initial = QuicPacket::build_initial(vec![0x01, 0x02, 0x03], vec![0x04, 0x05], b"Initial Token");
    let serialized = initial.serialize();
    let parsed = QuicPacket::parse(&serialized).unwrap();

    if let QuicPacket::Long { packet_type, version, dcid, scid, payload } = parsed {
        assert_eq!(packet_type, QUIC_PKT_INITIAL);
        assert_eq!(version, QUIC_VERSION_1);
        assert_eq!(dcid, vec![0x01, 0x02, 0x03]);
        assert_eq!(scid, vec![0x04, 0x05]);
        assert_eq!(payload, b"Initial Token");
    } else {
        panic!("Expected Long Header");
    }

    // 2. Handshake Packet
    let handshake = QuicPacket::Long {
        packet_type: QUIC_PKT_HANDSHAKE,
        version: QUIC_VERSION_1,
        dcid: vec![0x0a],
        scid: vec![0x0b],
        payload: vec![0x99, 0x88],
    };
    let parsed_hs = QuicPacket::parse(&handshake.serialize()).unwrap();
    assert_eq!(parsed_hs, handshake);

    // 3. 1-RTT Short Header Packet
    let short = QuicPacket::build_1rtt(vec![0xAA; 8], 42, b"HTTP/3 Stream Chunk");
    let parsed_short = QuicPacket::parse(&short.serialize()).unwrap();
    if let QuicPacket::Short { dcid, packet_number, payload, .. } = parsed_short {
        assert_eq!(dcid, vec![0xAA; 8]);
        assert_eq!(packet_number, 42);
        assert_eq!(payload, b"HTTP/3 Stream Chunk");
    } else {
        panic!("Expected Short Header");
    }
}
