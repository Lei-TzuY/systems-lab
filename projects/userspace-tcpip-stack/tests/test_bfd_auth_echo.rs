use toy_tcpip::bfd::*;

#[test]
fn test_bfd_auth_simple_password_and_keyed_sha1_codec() {
    // 1. Simple Password
    let auth_pwd = BfdAuthHeader::SimplePassword {
        key_id: 1,
        password: b"secret123".to_vec(),
    };
    let pkt_pwd = BfdControlPacket::build_authenticated(
        BfdState::Up,
        0x11223344,
        0x55667788,
        50_000,
        auth_pwd.clone(),
    );
    let raw_pwd = pkt_pwd.serialize();
    let parsed_pwd = BfdControlPacket::parse(&raw_pwd).unwrap();
    assert!(parsed_pwd.auth);
    assert_eq!(parsed_pwd.auth_header, Some(auth_pwd));

    // 2. Keyed SHA1
    let auth_sha = BfdAuthHeader::KeyedSha1 {
        meticulous: true,
        key_id: 2,
        sequence_number: 100,
        auth_key_hash: [0xab; 20],
    };
    let pkt_sha = BfdControlPacket::build_authenticated(
        BfdState::Up,
        0x11223344,
        0x55667788,
        50_000,
        auth_sha.clone(),
    );
    let raw_sha = pkt_sha.serialize();
    let parsed_sha = BfdControlPacket::parse(&raw_sha).unwrap();
    assert!(parsed_sha.auth);
    assert_eq!(parsed_sha.auth_header, Some(auth_sha));
}

#[test]
fn test_bfd_echo_packet_codec_and_session_rtt() {
    let mut session = BfdSession::new(0x99887766, 20_000);

    // 1. Generate outbound echo packet at t = 100,000 us
    let echo_raw = session.generate_echo_packet(100_000);
    assert_eq!(session.echo_sequence, 2);

    let parsed_echo = BfdEchoPacket::parse(&echo_raw).unwrap();
    assert_eq!(parsed_echo.my_discriminator, 0x99887766);
    assert_eq!(parsed_echo.sender_timestamp_us, 100_000);
    assert_eq!(parsed_echo.sequence_number, 1);

    // 2. Peer reflects packet back; local processes it at t = 104,500 us
    let ok = session.process_echo_packet(&echo_raw, 104_500);
    assert!(ok);
    assert_eq!(session.last_echo_rtt_us, Some(4_500)); // 4.5ms RTT
}

#[test]
fn test_bfd_authenticated_session_fsm() {
    let mut session = BfdSession::new(0x1001, 100_000);
    session.auth_key = Some(BfdAuthHeader::SimplePassword {
        key_id: 1,
        password: b"bfdpass".to_vec(),
    });

    let incoming_down = BfdControlPacket::build_control(BfdState::Down, 0x2002, 0, 100_000);
    let resp = session.process_packet(&incoming_down).unwrap();
    assert_eq!(session.state, BfdState::Init);
    assert!(resp.auth);
    assert_eq!(
        resp.auth_header,
        Some(BfdAuthHeader::SimplePassword {
            key_id: 1,
            password: b"bfdpass".to_vec()
        })
    );
}
