use toy_tcpip::detnet::{
    DetNetControlWord, DetNetEliminationFilter, DetNetPacket, DetNetPrefEngine,
};

#[test]
fn test_detnet_control_word_and_packet_codec() {
    let cw = DetNetControlWord::new(42);
    let cw_bytes = cw.serialize();
    let parsed_cw = DetNetControlWord::parse(&cw_bytes).expect("parse control word");
    assert_eq!(parsed_cw.sequence_number, 42);

    let pkt = DetNetPacket::new(0xABCD, 100, vec![1, 2, 3, 4, 5]);
    let wire = pkt.encode();
    assert_eq!(wire.len(), 8 + 5);

    let parsed_pkt = DetNetPacket::decode(&wire).expect("parse detnet packet");
    assert_eq!(parsed_pkt, pkt);
}

#[test]
fn test_detnet_pref_replication_and_elimination() {
    let mut pref = DetNetPrefEngine::new(3, 64);
    let payload = b"ROBOTIC_ARM_CONTROL_ANGLE_90";

    // Replicate packet into 3 copies
    let copies = pref.replicate(0x5001, payload);
    assert_eq!(copies.len(), 3);
    assert_eq!(copies[0].control_word.sequence_number, 1);
    assert_eq!(copies[1].control_word.sequence_number, 1);
    assert_eq!(copies[2].control_word.sequence_number, 1);

    // First arrival should be accepted and forwarded
    let first = pref.eliminate(copies[0].clone());
    assert!(first.is_some());
    assert_eq!(first.unwrap().payload, payload);

    // Subsequent copies arriving over alternate paths should be eliminated
    let second = pref.eliminate(copies[1].clone());
    assert!(second.is_none());

    let third = pref.eliminate(copies[2].clone());
    assert!(third.is_none());

    let stats = pref.get_flow_stats(0x5001).expect("flow stats");
    assert_eq!(stats.packets_received, 3);
    assert_eq!(stats.packets_forwarded, 1);
    assert_eq!(stats.duplicates_dropped, 2);
}

#[test]
fn test_detnet_elimination_filter_wraparound_and_reordering() {
    let mut filter = DetNetEliminationFilter::new(32);

    // Initial sequence
    assert!(filter.process_sequence(65530));
    assert!(filter.process_sequence(65535));

    // 16-bit wraparound to 0, 1, 2
    assert!(filter.process_sequence(0));
    assert!(filter.process_sequence(1));

    // Duplicate 0 should be dropped
    assert!(!filter.process_sequence(0));

    // Duplicate 65535 should be dropped
    assert!(!filter.process_sequence(65535));

    assert_eq!(filter.stats.packets_forwarded, 4);
    assert_eq!(filter.stats.duplicates_dropped, 2);
}
