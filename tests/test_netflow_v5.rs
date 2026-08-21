use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::netflow_v5::{
    NETFLOW_V5_HEADER_LEN, NETFLOW_V5_RECORD_LEN, NETFLOW_V5_UDP_PORT, NetflowV5Packet,
    NetflowV5Table,
};

#[test]
fn test_netflow_v5_table_aggregation_and_export() {
    let mut table = NetflowV5Table::new();
    let client = Ipv4Address::new(192, 168, 1, 50);
    let server = Ipv4Address::new(10, 0, 0, 1);
    let gateway = Ipv4Address::new(192, 168, 1, 1);

    // Stream 10 packets on flow (client:44321 -> server:443)
    for i in 0..10 {
        table.record_flow(
            client,
            server,
            gateway,
            44321,
            443,
            6,
            1500,
            1000 + (i * 10),
        );
    }

    let export_pkt = table.export_packet(1100, 1700000000);
    assert_eq!(export_pkt.header.version, 5);
    assert_eq!(export_pkt.header.count, 1);
    assert_eq!(export_pkt.records.len(), 1);

    let rec = &export_pkt.records[0];
    assert_eq!(rec.src_addr, client);
    assert_eq!(rec.dst_addr, server);
    assert_eq!(rec.src_port, 44321);
    assert_eq!(rec.dst_port, 443);
    assert_eq!(rec.packet_count, 10);
    assert_eq!(rec.octet_count, 15000);

    // Verify raw wire format
    let raw = export_pkt.serialize();
    assert_eq!(raw.len(), NETFLOW_V5_HEADER_LEN + NETFLOW_V5_RECORD_LEN);

    let parsed = NetflowV5Packet::parse(&raw).unwrap();
    assert_eq!(parsed.header.version, 5);
    assert_eq!(parsed.header.count, 1);
    assert_eq!(parsed.records[0].octet_count, 15000);
}

#[test]
fn test_netflow_v5_port_constant() {
    assert_eq!(NETFLOW_V5_UDP_PORT, 2055);
}
