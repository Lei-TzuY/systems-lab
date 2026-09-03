use toy_tcpip::bfd::{BfdControlPacket, BfdSession, BfdState};

#[test]
fn init_packet_with_zero_your_discriminator_is_discarded_without_mutation() {
    let mut session = BfdSession::new(0x1001, 100_000);
    session.remote_discriminator = 0x2002;
    let incoming =
        BfdControlPacket::build_control(BfdState::Init, 0x3003, 0, 100_000);

    assert!(session.process_packet(&incoming).is_none());
    assert_eq!(session.state, BfdState::Down);
    assert_eq!(session.remote_discriminator, 0x2002);
}
