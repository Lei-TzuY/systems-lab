use std::str::FromStr;
use toy_tcpip::flowspec::{
    BGP_SAFI_FLOWSPEC, FlowspecAction, FlowspecDecision, FlowspecEngine, FlowspecMatch,
    FlowspecRule,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_flowspec_constants_and_ddos_filtering() {
    assert_eq!(BGP_SAFI_FLOWSPEC, 133);

    let mut engine = FlowspecEngine::new();

    // 1. Drop NTP amplification flood targeting 10.0.0.50 (UDP port 123)
    engine.add_rule(FlowspecRule {
        id: 1,
        match_fields: FlowspecMatch {
            dst_prefix: Some((Ipv4Address::new(10, 0, 0, 50), 32)),
            src_prefix: None,
            ip_protocol: Some(17),
            dst_port: None,
            src_port: Some(123),
            tcp_flags: None,
        },
        action: FlowspecAction::Drop,
    });

    // 2. Redirect malicious subnet traffic 198.51.100.0/24 to scrubber IP 10.255.255.1
    engine.add_rule(FlowspecRule {
        id: 2,
        match_fields: FlowspecMatch {
            dst_prefix: None,
            src_prefix: Some((Ipv4Address::new(198, 51, 100, 0), 24)),
            ip_protocol: None,
            dst_port: None,
            src_port: None,
            tcp_flags: None,
        },
        action: FlowspecAction::RedirectIp(Ipv4Address::new(10, 255, 255, 1)),
    });

    // 3. Mark DSCP 46 (Expedited Forwarding) for VoIP SIP traffic (UDP 5060)
    engine.add_rule(FlowspecRule {
        id: 3,
        match_fields: FlowspecMatch {
            dst_prefix: None,
            src_prefix: None,
            ip_protocol: Some(17),
            dst_port: Some(5060),
            src_port: None,
            tcp_flags: None,
        },
        action: FlowspecAction::MarkDscp(46),
    });

    // Evaluate Attack 1 (NTP flood)
    let res1 = engine.evaluate(
        Ipv4Address::new(192, 0, 2, 1),
        Ipv4Address::new(10, 0, 0, 50),
        17,
        Some(123),
        Some(49152),
        None,
    );
    assert_eq!(res1, FlowspecDecision::Drop);

    // Evaluate Attack 2 (Subnet traffic redirected to scrubber)
    let res2 = engine.evaluate(
        Ipv4Address::new(198, 51, 100, 42),
        Ipv4Address::new(10, 0, 0, 10),
        6,
        Some(34567),
        Some(443),
        None,
    );
    assert_eq!(
        res2,
        FlowspecDecision::Redirect(Ipv4Address::new(10, 255, 255, 1))
    );

    // Evaluate SIP VoIP traffic
    let res3 = engine.evaluate(
        Ipv4Address::new(10, 1, 1, 10),
        Ipv4Address::new(10, 1, 1, 20),
        17,
        Some(5060),
        Some(5060),
        None,
    );
    assert_eq!(res3, FlowspecDecision::Mark(46));

    // Evaluate normal HTTP traffic
    let res4 = engine.evaluate(
        Ipv4Address::new(10, 1, 1, 10),
        Ipv4Address::new(10, 1, 1, 20),
        6,
        Some(50000),
        Some(80),
        None,
    );
    assert_eq!(res4, FlowspecDecision::Pass);
}

#[test]
fn test_flowspec_serialization_and_display() {
    let rule = FlowspecRule {
        id: 10,
        match_fields: FlowspecMatch {
            dst_prefix: Some((Ipv4Address::from_str("192.168.1.0").unwrap(), 24)),
            src_prefix: None,
            ip_protocol: Some(6),
            dst_port: Some(80),
            src_port: None,
            tcp_flags: None,
        },
        action: FlowspecAction::RateLimitBps(500_000),
    };

    let engine = FlowspecEngine::new();
    let bytes = engine.serialize_rule(&rule);
    assert!(!bytes.is_empty());
    assert_eq!(format!("{}", rule.action), "RATE-LIMIT (500000 bps)");
}
