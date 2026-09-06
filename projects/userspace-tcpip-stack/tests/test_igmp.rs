use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::igmp::{
    ALL_HOSTS_MULTICAST_IP, IGMP_TYPE_LEAVE_GROUP, IGMP_TYPE_MEMBERSHIP_QUERY,
    IGMP_TYPE_V2_MEMBERSHIP_REPORT, IgmpPacket, MulticastGroupTable, multicast_ip_to_mac,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_igmp_packet_construction_and_checksum() {
    let group = Ipv4Address::new(239, 255, 0, 1);

    // V2 Membership Report
    let report = IgmpPacket::build_v2_membership_report(group);
    let raw_report = report.serialize();
    let parsed_report = IgmpPacket::parse(&raw_report, true).unwrap();
    assert_eq!(parsed_report.msg_type, IGMP_TYPE_V2_MEMBERSHIP_REPORT);
    assert_eq!(parsed_report.group_address, group);

    // Membership Query
    let query = IgmpPacket::build_membership_query(group, 10);
    let raw_query = query.serialize();
    let parsed_query = IgmpPacket::parse(&raw_query, true).unwrap();
    assert_eq!(parsed_query.msg_type, IGMP_TYPE_MEMBERSHIP_QUERY);
    assert_eq!(parsed_query.max_response_time, 100);

    // Leave Group
    let leave = IgmpPacket::build_leave_group(group);
    let raw_leave = leave.serialize();
    let parsed_leave = IgmpPacket::parse(&raw_leave, true).unwrap();
    assert_eq!(parsed_leave.msg_type, IGMP_TYPE_LEAVE_GROUP);
}

#[test]
fn test_multicast_mac_mapping_and_membership() {
    let group = Ipv4Address::new(224, 128, 64, 32);
    let mac = multicast_ip_to_mac(group);
    // 01:00:5e:(128 & 0x7F):64:32 -> 01:00:5e:00:40:20
    assert_eq!(mac, MacAddress([0x01, 0x00, 0x5e, 0x00, 0x40, 0x20]));

    let mut table = MulticastGroupTable::new();
    assert!(table.is_member(&ALL_HOSTS_MULTICAST_IP));

    table.join(group);
    assert!(table.is_member(&group));

    table.leave(&group);
    assert!(!table.is_member(&group));
}
