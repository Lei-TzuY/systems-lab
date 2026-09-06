use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::pfcp_5g::{
    ForwardingActionRule, PFCP_APPLY_ACTION_DROP, PFCP_APPLY_ACTION_FORWARD,
    PFCP_MSG_ASSOCIATION_SETUP_REQUEST, PFCP_MSG_SESSION_ESTABLISHMENT_REQUEST,
    PFCP_SRC_INTERFACE_ACCESS, PFCP_SRC_INTERFACE_CORE, PFCP_UDP_PORT, PacketDetectionRule,
    PfcpNode,
};

#[test]
fn test_pfcp_constants_and_lifecycle() {
    assert_eq!(PFCP_UDP_PORT, 8805);
    assert_eq!(PFCP_MSG_ASSOCIATION_SETUP_REQUEST, 5);
    assert_eq!(PFCP_MSG_SESSION_ESTABLISHMENT_REQUEST, 50);
}

#[test]
fn test_pfcp_pdr_far_installation_and_session_lookup() {
    let mut upf = PfcpNode::new("upf-hsinchu-01");
    assert!(upf.handle_association_setup("smf-control-01"));

    let pdr1 = PacketDetectionRule {
        pdr_id: 10,
        precedence: 50,
        source_interface: PFCP_SRC_INTERFACE_ACCESS,
        teid: Some(0x20001),
        ue_ip: Some(Ipv4Address::new(10, 80, 1, 5)),
    };
    let far1 = ForwardingActionRule {
        far_id: 10,
        apply_action: PFCP_APPLY_ACTION_FORWARD,
        destination_interface: PFCP_SRC_INTERFACE_CORE,
        outer_header_creation: None,
    };

    let pdr2 = PacketDetectionRule {
        pdr_id: 20,
        precedence: 50,
        source_interface: PFCP_SRC_INTERFACE_ACCESS,
        teid: Some(0x20002),
        ue_ip: Some(Ipv4Address::new(10, 80, 1, 6)),
    };
    let far2 = ForwardingActionRule {
        far_id: 20,
        apply_action: PFCP_APPLY_ACTION_DROP,
        destination_interface: PFCP_SRC_INTERFACE_CORE,
        outer_header_creation: None,
    };

    let up_seid = upf.establish_session(0x11223344, vec![pdr1, pdr2], vec![far1, far2]);
    assert_eq!(up_seid, 101);

    // Match TEID 0x20001 -> Forward
    let match1 = upf.match_and_forward(up_seid, 0x20001).unwrap();
    assert_eq!(match1.apply_action, PFCP_APPLY_ACTION_FORWARD);

    // Match TEID 0x20002 -> Drop
    let match2 = upf.match_and_forward(up_seid, 0x20002).unwrap();
    assert_eq!(match2.apply_action, PFCP_APPLY_ACTION_DROP);

    // Unmatched TEID
    assert!(upf.match_and_forward(up_seid, 0x99999).is_none());
}
