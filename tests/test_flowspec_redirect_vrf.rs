use toy_tcpip::flowspec_redirect_vrf::{
    FlowspecVrfAction, FlowspecVrfRule, FlowspecVrfScrubbingEngine,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_flowspec_redirect_to_vrf_and_dscp_remarking() {
    let mut engine = FlowspecVrfScrubbingEngine::new();
    let victim_ip = Ipv4Address::new(198, 51, 100, 10);

    // Rule 1: Divert DNS Flood (UDP port 53) to Scrubbing VRF
    engine.add_rule(FlowspecVrfRule {
        rule_id: 1,
        match_dst_ip: Some(victim_ip),
        match_protocol: Some(17), // UDP
        match_dst_port: Some(53),
        action: FlowspecVrfAction::RedirectVrf("VRF_DDoS_Cleaner".to_string()),
    });

    // Rule 2: Remark DSCP to 46 (EF) for HTTP traffic
    engine.add_rule(FlowspecVrfRule {
        rule_id: 2,
        match_dst_ip: Some(victim_ip),
        match_protocol: Some(6), // TCP
        match_dst_port: Some(80),
        action: FlowspecVrfAction::RemarkDscp(46),
    });

    // 1. Test DNS attack packet diversion
    let act1 = engine.evaluate_packet(victim_ip, 17, 53);
    assert_eq!(
        act1,
        FlowspecVrfAction::RedirectVrf("VRF_DDoS_Cleaner".to_string())
    );
    assert_eq!(engine.redirected_packets_count, 1);

    // 2. Test HTTP remarking
    let act2 = engine.evaluate_packet(victim_ip, 6, 80);
    assert_eq!(act2, FlowspecVrfAction::RemarkDscp(46));
    assert_eq!(engine.remarked_packets_count, 1);

    // 3. Test clean unmatched traffic
    let act3 = engine.evaluate_packet(victim_ip, 6, 443);
    assert_eq!(act3, FlowspecVrfAction::Pass);
    assert_eq!(engine.passed_packets_count, 1);
}
