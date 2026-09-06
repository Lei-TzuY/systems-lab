use toy_tcpip::ipsec::{EspPacket, SadTable, SecurityAssociation};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_ipsec_esp_framing_and_icv() {
    let key = [0x77; 16];
    let payload = b"Confidential Corporate IPsec Tunnel Packet";

    let esp = EspPacket::build(0xDEADBEEF, 42, 6, payload, &key);
    let raw = esp.serialize();

    let parsed = EspPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.spi, 0xDEADBEEF);
    assert_eq!(parsed.header.seq_num, 42);
    assert_eq!(parsed.next_header, 6); // TCP
    assert_eq!(parsed.payload, payload);
    assert_eq!(parsed.icv, esp.icv);
}

#[test]
fn test_ipsec_sad_table_and_anti_replay() {
    let mut sad = SadTable::new();
    let mut sa = SecurityAssociation::new(
        0x5000,
        Ipv4Address::new(10, 0, 0, 1),
        Ipv4Address::new(10, 0, 0, 2),
        [0xAA; 16],
    );

    // Sequence 1..10 in order
    for s in 1..=10 {
        assert!(sa.check_anti_replay(s));
    }

    // Replay attack with seq 5
    assert!(!sa.check_anti_replay(5));

    // Replay attack with seq 1
    assert!(!sa.check_anti_replay(1));

    sad.insert_sa(sa, true);
    assert!(sad.inbound.contains_key(&0x5000));
}
