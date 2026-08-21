use toy_tcpip::bfd::{BfdControlPacket, BfdSession, BfdState, BFD_CONTROL_PORT, BFD_MIN_PACKET_LEN};

#[test]
fn test_bfd_control_packet_serialization() {
    let pkt = BfdControlPacket::build_control(BfdState::Up, 0x11223344, 0x55667788, 50_000);
    let raw = pkt.serialize();

    assert_eq!(raw.len(), BFD_MIN_PACKET_LEN);
    let parsed = BfdControlPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.state, BfdState::Up);
    assert_eq!(parsed.my_discriminator, 0x11223344);
    assert_eq!(parsed.your_discriminator, 0x55667788);
    assert_eq!(BFD_CONTROL_PORT, 3784);
}

#[test]
fn test_bfd_three_way_handshake_fsm() {
    let mut session = BfdSession::new(101, 50_000);
    assert_eq!(session.state, BfdState::Down);

    // 1. Receive Down from remote -> Transition to Init
    let remote_down = BfdControlPacket::build_control(BfdState::Down, 202, 0, 50_000);
    let reply1 = session.process_packet(&remote_down).unwrap();
    assert_eq!(session.state, BfdState::Init);
    assert_eq!(reply1.state, BfdState::Init);
    assert_eq!(reply1.your_discriminator, 202);

    // 2. Receive Init from remote -> Transition to Up
    let remote_init = BfdControlPacket::build_control(BfdState::Init, 202, 101, 50_000);
    let reply2 = session.process_packet(&remote_init).unwrap();
    assert_eq!(session.state, BfdState::Up);
    assert_eq!(reply2.state, BfdState::Up);
}
