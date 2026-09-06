use std::str::FromStr;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::srv6_slicing::{
    NetworkSliceId, SliceType, Srv6SliceForwardingEngine, Srv6SlicePolicy,
};

#[test]
fn test_srv6_network_slicing_policy_and_packet_steering() {
    let mut engine = Srv6SliceForwardingEngine::new();

    let embb_slice_id = NetworkSliceId(1);
    let urllc_slice_id = NetworkSliceId(2);

    let sid1 = Ipv6Address::from_str("fc00:1::1").unwrap();
    let sid2 = Ipv6Address::from_str("fc00:2::1").unwrap();

    // Add eMBB High Throughput Slice
    engine.add_slice(Srv6SlicePolicy {
        slice_id: embb_slice_id,
        slice_name: "eMBB-4K-Video".to_string(),
        slice_type: SliceType::Embb,
        flex_algo: 129,
        guaranteed_bandwidth_kbps: 500_000,
        segment_list: vec![sid1],
        max_latency_microseconds: 20_000,
    });

    // Add URLLC Low Latency Slice
    engine.add_slice(Srv6SlicePolicy {
        slice_id: urllc_slice_id,
        slice_name: "URLLC-AutonomousDriving".to_string(),
        slice_type: SliceType::Urllc,
        flex_algo: 128,
        guaranteed_bandwidth_kbps: 50_000,
        segment_list: vec![sid2],
        max_latency_microseconds: 500,
    });

    let car_ue_ip = Ipv4Address::new(10, 50, 0, 99);
    let phone_ue_ip = Ipv4Address::new(10, 60, 0, 10);

    assert!(engine.bind_subscriber_to_slice(car_ue_ip, urllc_slice_id));
    assert!(engine.bind_subscriber_to_slice(phone_ue_ip, embb_slice_id));

    // Steer Autonomous Driving packet (URLLC)
    let steer_car = engine
        .steer_packet(car_ue_ip, 256)
        .expect("steer car packet");
    assert_eq!(steer_car.slice_id, urllc_slice_id);
    assert_eq!(steer_car.flex_algo, 128);
    assert_eq!(steer_car.srv6_sid_list, vec![sid2]);

    // Steer Phone Video packet (eMBB)
    let steer_phone = engine
        .steer_packet(phone_ue_ip, 1400)
        .expect("steer phone packet");
    assert_eq!(steer_phone.slice_id, embb_slice_id);
    assert_eq!(steer_phone.flex_algo, 129);
    assert_eq!(steer_phone.srv6_sid_list, vec![sid1]);

    assert_eq!(engine.steered_packets_count, 2);
    assert_eq!(
        engine.slice_metered_bytes.get(&urllc_slice_id).copied(),
        Some(256)
    );
    assert_eq!(
        engine.slice_metered_bytes.get(&embb_slice_id).copied(),
        Some(1400)
    );
}
