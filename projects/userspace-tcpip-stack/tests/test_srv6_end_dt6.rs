//! Integration tests for SRv6 End.DT6 IPv6-Only Multi-VRF Routing (RFC 8986 §4.14).

use std::str::FromStr;
use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::srv6::Srv6Header;
use toy_tcpip::srv6_end_dt6::{EndDt6ForwardVerdict, Ipv6VrfRoute, Srv6EndDt6Router};

#[test]
fn test_srv6_end_dt6_vrf_isolation_and_forwarding() {
    let mut router = Srv6EndDt6Router::new();

    let sid_vrf10 = Ipv6Address::from_str("fd00:10::d06").unwrap();
    let sid_vrf20 = Ipv6Address::from_str("fd00:20::d06").unwrap();

    router.register_end_dt6_sid(sid_vrf10, 10);
    router.register_end_dt6_sid(sid_vrf20, 20);

    // Route in VRF 10: 2001:db8:10::/64 -> veth10
    router.add_vrf_route(
        10,
        Ipv6VrfRoute {
            prefix: Ipv6Address::from_str("2001:db8:10::").unwrap(),
            prefix_len: 64,
            next_hop: None,
            out_if: "veth10".to_string(),
        },
    );

    // Route in VRF 20: 2001:db8:20::/64 -> veth20
    router.add_vrf_route(
        20,
        Ipv6VrfRoute {
            prefix: Ipv6Address::from_str("2001:db8:20::").unwrap(),
            prefix_len: 64,
            next_hop: None,
            out_if: "veth20".to_string(),
        },
    );

    // Incoming customer packet for VRF 10
    let mut pkt_vrf10 = vec![0u8; 40];
    pkt_vrf10[0] = 0x60; // IPv6 version
    let dst10 = Ipv6Address::from_str("2001:db8:10::99").unwrap();
    pkt_vrf10[24..40].copy_from_slice(&dst10.0);
    pkt_vrf10.extend_from_slice(b"SecureVRF10Data");

    let srh10 = Srv6Header::build(4, &[sid_vrf10]);
    let res10 = router.process_end_dt6_packet(sid_vrf10, srh10, &pkt_vrf10);

    match res10 {
        EndDt6ForwardVerdict::ForwardCustomer {
            vrf_id,
            dst_ip,
            out_if,
            ..
        } => {
            assert_eq!(vrf_id, 10);
            assert_eq!(dst_ip, dst10);
            assert_eq!(out_if, "veth10");
        }
        other => panic!("Expected ForwardCustomer on VRF 10, got {:?}", other),
    }

    // Packet targeting VRF 20 via SID 10 -> NoRoute in VRF 10 (VRF isolation enforced)
    let mut pkt_vrf20 = vec![0u8; 40];
    pkt_vrf20[0] = 0x60;
    let dst20 = Ipv6Address::from_str("2001:db8:20::88").unwrap();
    pkt_vrf20[24..40].copy_from_slice(&dst20.0);

    let srh_cross = Srv6Header::build(4, &[sid_vrf10]);
    let res_cross = router.process_end_dt6_packet(sid_vrf10, srh_cross, &pkt_vrf20);
    match res_cross {
        EndDt6ForwardVerdict::NoRoute { vrf_id, dst_ip } => {
            assert_eq!(vrf_id, 10);
            assert_eq!(dst_ip, dst20);
        }
        other => panic!("Expected NoRoute due to VRF isolation, got {:?}", other),
    }
}
