use toy_tcpip::srv6_usid::{UsidBehavior, UsidCarrier, UsidForwardingEngine};

#[test]
fn test_srv6_usid_carrier_roundtrip_and_shift_forwarding() {
    let mut engine = UsidForwardingEngine::new();
    engine.register_usid(0x1001, UsidBehavior::EndUN);
    engine.register_usid(0x2002, UsidBehavior::EndUA);
    engine.register_usid(0x3003, UsidBehavior::EndUN);
    engine.register_usid(0xEEEE, UsidBehavior::EndUDT6);

    let carrier = UsidCarrier::new(0xFC000001, vec![0x1001, 0x2002, 0x3003, 0xEEEE]);
    let initial_da = carrier.to_ipv6();

    // Node 1 (0x1001)
    let (hop1_da, beh1) = engine.process_destination_address(&initial_da).unwrap();
    assert_eq!(beh1, UsidBehavior::EndUN);
    let hop1_c = UsidCarrier::from_ipv6(&hop1_da);
    assert_eq!(hop1_c.micro_sids, vec![0x2002, 0x3003, 0xEEEE]);

    // Node 2 (0x2002)
    let (hop2_da, beh2) = engine.process_destination_address(&hop1_da).unwrap();
    assert_eq!(beh2, UsidBehavior::EndUA);
    let hop2_c = UsidCarrier::from_ipv6(&hop2_da);
    assert_eq!(hop2_c.micro_sids, vec![0x3003, 0xEEEE]);

    // Node 3 (0x3003)
    let (hop3_da, beh3) = engine.process_destination_address(&hop2_da).unwrap();
    assert_eq!(beh3, UsidBehavior::EndUN);
    let hop3_c = UsidCarrier::from_ipv6(&hop3_da);
    assert_eq!(hop3_c.micro_sids, vec![0xEEEE]);

    // Node 4 Egress Decap (0xEEEE)
    let (_, beh4) = engine.process_destination_address(&hop3_da).unwrap();
    assert_eq!(beh4, UsidBehavior::EndUDT6);
}

#[test]
fn test_usid_ipv6_direct_conversion() {
    let carrier = UsidCarrier::new(0xFC000002, vec![0xAAAA, 0xBBBB]);
    let ipv6 = carrier.to_ipv6();
    let parsed = UsidCarrier::from_ipv6(&ipv6);
    assert_eq!(parsed.block_prefix, 0xFC000002);
    assert_eq!(parsed.micro_sids, vec![0xAAAA, 0xBBBB]);
}
