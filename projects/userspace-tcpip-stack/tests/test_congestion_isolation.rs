use toy_tcpip::congestion_isolation::{
    CongestionFlowKey, CongestionIsolationEngine, FlowCongestionEntry, FlowIsolationState,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_congestion_isolation_states_and_flow_entry() {
    let key = CongestionFlowKey {
        src_ip: Ipv4Address::new(10, 1, 1, 1),
        dst_ip: Ipv4Address::new(10, 1, 1, 2),
        protocol: 17,
        src_port: 12345,
        dst_port: 4791,
    };

    let entry = FlowCongestionEntry {
        key: key.clone(),
        state: FlowIsolationState::Normal,
        ecn_ce_count: 0,
        last_seen_us: 1000,
        assigned_queue_id: 0,
    };

    assert_eq!(entry.state, FlowIsolationState::Normal);
    assert_eq!(entry.assigned_queue_id, 0);
}

#[test]
fn test_congestion_isolation_multi_flow_segregation() {
    let mut engine = CongestionIsolationEngine::new(2); // 2 CE marks trigger isolation

    let flow_a = CongestionFlowKey {
        src_ip: Ipv4Address::new(10, 1, 1, 1),
        dst_ip: Ipv4Address::new(10, 1, 1, 2),
        protocol: 17,
        src_port: 10001,
        dst_port: 4791,
    };

    let flow_b = CongestionFlowKey {
        src_ip: Ipv4Address::new(10, 2, 2, 1),
        dst_ip: Ipv4Address::new(10, 2, 2, 2),
        protocol: 17,
        src_port: 20002,
        dst_port: 4791,
    };

    // Flow A experiences congestion
    engine.process_packet(flow_a.clone(), 0x03, 100);
    let qa = engine.process_packet(flow_a.clone(), 0x03, 150);
    assert_eq!(qa, 1); // Flow A isolated!

    // Flow B is clean
    let qb = engine.process_packet(flow_b.clone(), 0x00, 200);
    assert_eq!(qb, 0); // Flow B remains in standard queue (protected from HoL blocking!)
}
