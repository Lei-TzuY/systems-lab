use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::openflow::{
    OFP_TCP_PORT, OFP_VERSION_1_3, OFPT_FEATURES_REPLY, OFPT_HELLO, OfpAction, OfpFlowTable,
    OfpMatch, OfpMessage,
};

#[test]
fn test_openflow_flow_table_lookup_and_pipeline() {
    let mut table = OfpFlowTable::new();

    // High priority rule: Ingress port 2, IPv4 dst 192.168.1.50 -> Output port 3
    table.add_entry(
        500,
        OfpMatch {
            in_port: Some(2),
            eth_type: Some(0x0800),
            ip_dst: Some(Ipv4Address::new(192, 168, 1, 50)),
        },
        vec![OfpAction::Output(3)],
    );

    // Medium priority rule: VLAN tagging
    table.add_entry(
        300,
        OfpMatch {
            in_port: Some(2),
            eth_type: Some(0x0800),
            ip_dst: None,
        },
        vec![OfpAction::SetVlan(100), OfpAction::Output(4)],
    );

    // Low priority rule: Drop
    table.add_entry(0, OfpMatch::default(), vec![OfpAction::Drop]);

    // Test specific match
    let act1 = table.lookup_and_execute(2, 0x0800, Some(Ipv4Address::new(192, 168, 1, 50)), 128);
    assert_eq!(act1, Some(vec![OfpAction::Output(3)]));

    // Test wildcard match
    let act2 = table.lookup_and_execute(2, 0x0800, Some(Ipv4Address::new(192, 168, 1, 99)), 64);
    assert_eq!(
        act2,
        Some(vec![OfpAction::SetVlan(100), OfpAction::Output(4)])
    );

    // Test default drop
    let act3 = table.lookup_and_execute(1, 0x86DD, None, 100);
    assert_eq!(act3, Some(vec![OfpAction::Drop]));
}

#[test]
fn test_openflow_features_and_hello_framing() {
    let (hdr, msg) = OfpMessage::build_features_reply(0x55AA1122, 0x0000000000000001);
    let raw = msg.serialize(&hdr);

    let (p_hdr, p_msg) = OfpMessage::parse(&raw).unwrap();
    assert_eq!(p_hdr.version, OFP_VERSION_1_3);
    assert_eq!(p_hdr.msg_type, OFPT_FEATURES_REPLY);
    assert_eq!(p_hdr.xid, 0x55AA1122);

    if let OfpMessage::FeaturesReply {
        datapath_id,
        n_buffers,
        n_tables,
    } = p_msg
    {
        assert_eq!(datapath_id, 1);
        assert_eq!(n_buffers, 256);
        assert_eq!(n_tables, 64);
    } else {
        panic!("Expected OFPT_FEATURES_REPLY");
    }

    assert_eq!(OFP_TCP_PORT, 6653);
    assert_eq!(OFPT_HELLO, 0);
}
