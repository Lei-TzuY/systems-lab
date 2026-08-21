use toy_tcpip::p4runtime::{
    P4MatchField, P4MatchKind, P4PacketOut, P4RuntimeServer, P4TableEntry, P4RUNTIME_PORT,
    P4RUNTIME_VERSION,
};

#[test]
fn test_p4runtime_match_action_table_and_packet_io() {
    let mut server = P4RuntimeServer::new(10);
    assert_eq!(P4RUNTIME_PORT, 9559);
    assert_eq!(P4RUNTIME_VERSION, "v1.3.0");

    let entry = P4TableEntry {
        table_name: "FabricIngress.forwarding".to_string(),
        matches: vec![P4MatchField {
            field_name: "hdr.ethernet.dst_addr".to_string(),
            match_value: P4MatchKind::Exact(vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
        }],
        action_name: "FabricIngress.set_egress_port".to_string(),
        action_params: vec![("egress_port".to_string(), vec![0, 0, 0, 4])],
        priority: 1,
    };

    server.write_table_entry(entry);
    assert_eq!(server.table_entries["FabricIngress.forwarding"].len(), 1);

    // Test Packet-Out
    let sent = server.handle_packet_out(P4PacketOut {
        egress_port: 4,
        payload: b"Data Plane Packet".to_vec(),
    });
    assert_eq!(sent, 17);
    assert_eq!(server.packet_out_count, 1);

    // Test Packet-In
    let pin = server.emit_packet_in(2, b"Control Trap");
    assert_eq!(pin.ingress_port, 2);
    assert_eq!(server.packet_in_count, 1);
}
