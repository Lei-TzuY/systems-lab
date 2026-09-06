use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::nef_traffic_influence::{
    EdgeSteeringDecision, NefTrafficInfluenceEngine, NefTrafficInfluenceSub, SliceId, TrafficFilter,
};

#[test]
fn test_nef_traffic_influence_subscription_lifecycle() {
    let mut engine = NefTrafficInfluenceEngine::new();
    let slice = SliceId {
        sst: 2,
        sd: 0x000002,
    };
    let filter = TrafficFilter {
        dst_ip: Ipv4Address::new(198, 51, 100, 50),
        dst_port: 9000,
        protocol: 17, // UDP
    };

    let sub_id = engine.create_subscription(
        "af-autonomous-driving",
        "v2x-telemetry",
        "v2x.5g",
        slice.clone(),
        filter,
        "DNAI-Edge-V2X",
        Ipv4Address::new(10, 50, 0, 1),
    );
    assert_eq!(sub_id, 1);
    assert_eq!(engine.subscriptions.len(), 1);

    let sub: &NefTrafficInfluenceSub = &engine.subscriptions[0];
    assert_eq!(sub.af_service_id, "v2x-telemetry");

    // Evaluate matching traffic
    let decision: EdgeSteeringDecision = engine
        .evaluate_packet(
            "v2x.5g",
            &slice,
            Ipv4Address::new(198, 51, 100, 50),
            9000,
            17,
        )
        .unwrap();
    assert_eq!(decision.target_dnai, "DNAI-Edge-V2X");
    assert_eq!(decision.local_breakout_ip, Ipv4Address::new(10, 50, 0, 1));

    // Delete subscription
    assert!(engine.delete_subscription(sub_id));
    assert_eq!(engine.subscriptions.len(), 0);
}
