use toy_tcpip::sbfd::{SbfdPacket, SbfdReflector, SbfdState, SBFD_REFLECTOR_PORT};

#[test]
fn test_sbfd_probe_and_reflection() {
    let mut reflector = SbfdReflector::new();
    reflector.register_discriminator(0xAA112233);

    let probe = SbfdPacket::build_initiator_probe(0x55443322, 0xAA112233, 20_000);
    assert_eq!(probe.my_discriminator, 0x55443322);
    assert_eq!(probe.your_discriminator, 0xAA112233);
    assert_eq!(probe.poll, true);

    let raw = probe.serialize();
    let parsed_probe = SbfdPacket::parse(&raw).unwrap();

    let response = reflector.process_probe(&parsed_probe).unwrap();
    assert_eq!(response.state, SbfdState::Up);
    assert_eq!(response.my_discriminator, 0xAA112233);
    assert_eq!(response.your_discriminator, 0x55443322);
    assert_eq!(response.final_bit, true);
}

#[test]
fn test_sbfd_mismatched_target_discriminator() {
    let mut reflector = SbfdReflector::new();
    reflector.register_discriminator(0x11111111);

    let probe = SbfdPacket::build_initiator_probe(0x55443322, 0x22222222, 20_000);
    assert!(reflector.process_probe(&probe).is_none());
}

#[test]
fn test_sbfd_port_constant() {
    assert_eq!(SBFD_REFLECTOR_PORT, 7784);
}
