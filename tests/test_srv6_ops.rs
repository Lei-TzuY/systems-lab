use std::str::FromStr;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::srv6::Srv6Header;
use toy_tcpip::srv6_ops::{Srv6Behavior, Srv6Engine, Srv6ExecutionResult};

#[test]
fn test_srv6_endpoint_behaviors_pipeline() {
    let mut engine = Srv6Engine::new();

    let sid_transit = Ipv6Address::from_str("2001:db8:1::1").unwrap();
    let sid_x = Ipv6Address::from_str("2001:db8:1::2").unwrap();
    let sid_dx4 = Ipv6Address::from_str("2001:db8:1::3").unwrap();
    let sid_dx2 = Ipv6Address::from_str("2001:db8:1::4").unwrap();

    let next_hop_x = Ipv6Address::from_str("fe80::99").unwrap();
    let next_hop_dx4 = Ipv4Address::new(10, 0, 0, 1);

    engine.register_sid(sid_transit, Srv6Behavior::End);
    engine.register_sid(sid_x, Srv6Behavior::EndX { next_hop_ip: next_hop_x, out_if: "eth1".to_string() });
    engine.register_sid(sid_dx4, Srv6Behavior::EndDx4 { next_hop_ipv4: next_hop_dx4 });
    engine.register_sid(sid_dx2, Srv6Behavior::EndDx2 { out_if: "tap0".to_string() });

    // Test End Behavior (Transit Hop)
    let srh_transit = Srv6Header::build(4, &[sid_dx4, sid_transit]);
    let res1 = engine.process_srv6_packet(sid_transit, srh_transit, b"IPv4 Data");
    match res1 {
        Srv6ExecutionResult::ForwardNextSid { next_sid, updated_srh } => {
            assert_eq!(next_sid, sid_dx4);
            assert_eq!(updated_srh.segments_left, 0);
        }
        _ => panic!("Expected ForwardNextSid"),
    }

    // Test End.X Behavior (Adjacency Cross-Connect)
    let srh_x = Srv6Header::build(4, &[sid_dx4, sid_x]);
    let res2 = engine.process_srv6_packet(sid_x, srh_x, b"L3 Payload");
    match res2 {
        Srv6ExecutionResult::ForwardAdjacency { next_sid, next_hop, out_if } => {
            assert_eq!(next_sid, sid_dx4);
            assert_eq!(next_hop, next_hop_x);
            assert_eq!(out_if, "eth1");
        }
        _ => panic!("Expected ForwardAdjacency"),
    }

    // Test End.DX4 Behavior (Decapsulate to IPv4)
    let srh_dx4 = Srv6Header::build(4, &[sid_dx4]);
    let res3 = engine.process_srv6_packet(sid_dx4, srh_dx4, b"Raw IPv4 Packet");
    match res3 {
        Srv6ExecutionResult::DecapIpv4 { vrf_id, payload } => {
            assert_eq!(vrf_id, None);
            assert_eq!(payload, b"Raw IPv4 Packet");
        }
        _ => panic!("Expected DecapIpv4"),
    }

    // Test End.DX2 Behavior (Decapsulate to Layer 2)
    let srh_dx2 = Srv6Header::build(4, &[sid_dx2]);
    let res4 = engine.process_srv6_packet(sid_dx2, srh_dx2, b"Raw Ethernet Frame");
    match res4 {
        Srv6ExecutionResult::DecapEthernet { out_if, frame } => {
            assert_eq!(out_if, "tap0");
            assert_eq!(frame, b"Raw Ethernet Frame");
        }
        _ => panic!("Expected DecapEthernet"),
    }
}
