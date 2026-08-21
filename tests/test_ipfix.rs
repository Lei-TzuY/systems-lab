use toy_tcpip::ipfix::{
    IPFIX_DEFAULT_TEMPLATE_ID, IPFIX_TCP_PORT, IPFIX_UDP_PORT, IPFIX_VERSION, IpfixFlowRecord,
    IpfixMessage,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_ipfix_constants_and_template_framing() {
    assert_eq!(IPFIX_VERSION, 10);
    assert_eq!(IPFIX_UDP_PORT, 4739);
    assert_eq!(IPFIX_TCP_PORT, 4739);
    assert_eq!(IPFIX_DEFAULT_TEMPLATE_ID, 256);

    let flows = vec![IpfixFlowRecord {
        src_ip: Ipv4Address::new(192, 168, 10, 5),
        dst_ip: Ipv4Address::new(172, 16, 20, 8),
        src_port: 49152,
        dst_port: 80,
        protocol: 6,
        packets: 500,
        octets: 750000,
        tcp_flags: 0x0010,
        vlan_id: 200,
    }];

    let msg = IpfixMessage::build_standard_flow_export(1700001000, 42, 7, &flows, true);
    let raw = msg.serialize();
    assert!(raw.len() >= 16);

    let parsed = IpfixMessage::parse(&raw).unwrap();
    assert_eq!(parsed.export_time, 1700001000);
    assert_eq!(parsed.sequence_number, 42);
    assert_eq!(parsed.observation_domain_id, 7);
    assert_eq!(parsed.flow_records.len(), 1);
    assert_eq!(parsed.flow_records[0].vlan_id, 200);
    assert_eq!(parsed.flow_records[0].octets, 750000);
}

#[test]
fn test_ipfix_multiple_data_records() {
    let flows = vec![
        IpfixFlowRecord {
            src_ip: Ipv4Address::new(10, 0, 0, 1),
            dst_ip: Ipv4Address::new(10, 0, 0, 2),
            src_port: 1234,
            dst_port: 5678,
            protocol: 17,
            packets: 10,
            octets: 1400,
            tcp_flags: 0,
            vlan_id: 10,
        },
        IpfixFlowRecord {
            src_ip: Ipv4Address::new(10, 0, 0, 3),
            dst_ip: Ipv4Address::new(10, 0, 0, 4),
            src_port: 4321,
            dst_port: 8765,
            protocol: 17,
            packets: 20,
            octets: 2800,
            tcp_flags: 0,
            vlan_id: 20,
        },
    ];

    let msg = IpfixMessage::build_standard_flow_export(1700002000, 99, 1, &flows, false);
    let raw = msg.serialize();

    let parsed = IpfixMessage::parse(&raw).unwrap();
    assert_eq!(parsed.flow_records.len(), 2);
    assert_eq!(parsed.flow_records[0].src_ip, Ipv4Address::new(10, 0, 0, 1));
    assert_eq!(parsed.flow_records[1].src_ip, Ipv4Address::new(10, 0, 0, 3));
    assert_eq!(parsed.flow_records[1].packets, 20);
}
