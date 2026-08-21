use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::netflow::{NETFLOW_V9_UDP_PORT, NetflowFlowTable, NetflowPacket, NetflowRecord};

#[test]
fn test_netflow_v9_template_and_data_flowsets() {
    let r1 = NetflowRecord {
        src_ip: Ipv4Address::new(192, 168, 1, 100),
        dst_ip: Ipv4Address::new(192, 168, 1, 10),
        src_port: 55000,
        dst_port: 80,
        protocol: 6,
        packets: 10,
        bytes: 8400,
        tcp_flags: 0x18,
    };

    let pkt = NetflowPacket::build_export(42, vec![r1.clone()]);
    let raw = pkt.serialize();

    let parsed = NetflowPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.version, 9);
    assert_eq!(parsed.header.sequence_number, 42);
    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.records[0].bytes, 8400);
    assert_eq!(NETFLOW_V9_UDP_PORT, 2055);
}

#[test]
fn test_netflow_flow_table_aggregation() {
    let mut table = NetflowFlowTable::new();
    table.record_traffic(
        Ipv4Address::new(10, 0, 0, 1),
        Ipv4Address::new(10, 0, 0, 2),
        4000,
        5000,
        17,
        5,
        1500,
        0,
    );

    let records = table.export_records();
    assert!(records.len() >= 3);
}
