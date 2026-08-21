use toy_tcpip::mpls::{LfibAction, LfibTable, MplsHeader, MplsPacket};

#[test]
fn test_mpls_shim_header_encoding_and_bos() {
    let shim1 = MplsHeader::new(500, 2, false, 32);
    let shim2 = MplsHeader::new(600, 0, true, 31);

    let pkt = MplsPacket {
        labels: vec![shim1, shim2],
        payload: b"IPv4_PAYLOAD_HERE".to_vec(),
    };

    let serialized = pkt.serialize();
    assert_eq!(serialized.len(), 8 + 17);

    let parsed = MplsPacket::parse(&serialized).unwrap();
    assert_eq!(parsed.labels.len(), 2);
    assert_eq!(parsed.labels[0].label, 500);
    assert_eq!(parsed.labels[0].tc, 2);
    assert!(!parsed.labels[0].bottom_of_stack);
    assert_eq!(parsed.labels[1].label, 600);
    assert!(parsed.labels[1].bottom_of_stack);
    assert_eq!(parsed.payload, b"IPv4_PAYLOAD_HERE");
}

#[test]
fn test_mpls_lfib_operations() {
    let mut lfib = LfibTable::new();
    lfib.insert(1001, LfibAction::Push(2001));
    lfib.insert(2001, LfibAction::Swap(3001, "eth1".to_string()));
    lfib.insert(3001, LfibAction::Pop);

    assert_eq!(lfib.lookup(1001), Some(&LfibAction::Push(2001)));
    assert_eq!(
        lfib.lookup(2001),
        Some(&LfibAction::Swap(3001, "eth1".to_string()))
    );
    assert_eq!(lfib.lookup(3001), Some(&LfibAction::Pop));
}
