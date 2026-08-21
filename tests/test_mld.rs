use std::str::FromStr;
use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::mld::{
    MLD_ALLOW_NEW_SOURCES, MLD_BLOCK_OLD_SOURCES, MLD_CHANGE_TO_EXCLUDE, MLD_CHANGE_TO_INCLUDE,
    MldGroupRecord, MldTable, Mldv2ReportPacket,
};

#[test]
fn test_mldv2_ssm_channel_subscriptions_and_filtering() {
    let mut table = MldTable::new();

    let group1 = Ipv6Address::from_str("ff3e::8000:1").unwrap();
    let group2 = Ipv6Address::from_str("ff3e::8000:2").unwrap();

    let camera1 = Ipv6Address::from_str("2001:db8:10::1").unwrap();
    let camera2 = Ipv6Address::from_str("2001:db8:10::2").unwrap();
    let rogue_sender = Ipv6Address::from_str("2001:db8:66::66").unwrap();

    // 1. Client joins SSM channel (camera1, group1) and (camera2, group1)
    let join_report = Mldv2ReportPacket::new(vec![MldGroupRecord {
        record_type: MLD_CHANGE_TO_INCLUDE,
        multicast_address: group1,
        source_addresses: vec![camera1, camera2],
    }]);

    table.process_report(&join_report);
    assert!(table.is_listener_interested(group1, camera1));
    assert!(table.is_listener_interested(group1, camera2));
    assert!(!table.is_listener_interested(group1, rogue_sender));
    assert!(!table.is_listener_interested(group2, camera1));

    // 2. Block camera2
    let block_report = Mldv2ReportPacket::new(vec![MldGroupRecord {
        record_type: MLD_BLOCK_OLD_SOURCES,
        multicast_address: group1,
        source_addresses: vec![camera2],
    }]);
    table.process_report(&block_report);
    assert!(table.is_listener_interested(group1, camera1));
    assert!(!table.is_listener_interested(group1, camera2));

    // 3. Any-source multicast for group2 (Exclude Mode)
    let asm_report = Mldv2ReportPacket::new(vec![MldGroupRecord {
        record_type: MLD_CHANGE_TO_EXCLUDE,
        multicast_address: group2,
        source_addresses: vec![],
    }]);
    table.process_report(&asm_report);
    assert!(table.is_listener_interested(group2, camera1));
    assert!(table.is_listener_interested(group2, rogue_sender));
}

#[test]
fn test_mldv2_packet_serialization_and_deserialization() {
    let group = Ipv6Address::from_str("ff38::1234").unwrap();
    let src1 = Ipv6Address::from_str("2001:db8::1").unwrap();
    let src2 = Ipv6Address::from_str("2001:db8::2").unwrap();
    let src3 = Ipv6Address::from_str("2001:db8::3").unwrap();

    let report = Mldv2ReportPacket::new(vec![MldGroupRecord {
        record_type: MLD_ALLOW_NEW_SOURCES,
        multicast_address: group,
        source_addresses: vec![src1, src2, src3],
    }]);

    let bytes = report.serialize();
    let parsed = Mldv2ReportPacket::parse(&bytes).unwrap();

    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.records[0].record_type, MLD_ALLOW_NEW_SOURCES);
    assert_eq!(parsed.records[0].multicast_address, group);
    assert_eq!(parsed.records[0].source_addresses.len(), 3);
    assert_eq!(parsed.records[0].source_addresses[0], src1);
    assert_eq!(parsed.records[0].source_addresses[1], src2);
    assert_eq!(parsed.records[0].source_addresses[2], src3);
}
