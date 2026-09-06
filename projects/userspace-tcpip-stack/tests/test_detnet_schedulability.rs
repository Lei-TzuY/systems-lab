//! Integration tests for DetNet Schedulability & Over-Provisioning Analysis (RFC 9024 / IEEE 802.1Qbv)

use toy_tcpip::detnet_schedulability::{
    DetNetAdmissionDecision, DetNetFlowSpec, DetNetNodeCapacity, DetNetSchedulabilityEngine,
};

#[test]
fn test_detnet_multi_hop_schedulability_and_admission_pipeline() {
    let mut engine = DetNetSchedulabilityEngine::new();

    // 5-hop deterministic backbone with mixed 10 Gbps and 25 Gbps links
    engine.add_node(DetNetNodeCapacity {
        node_id: 1,
        link_speed_bps: 10_000_000_000,
        cycle_time_ns: 100_000,
        max_reservable_utilization: 0.75,
        propagation_delay_ns: 10_000,
        processing_delay_ns: 1_500,
    });
    engine.add_node(DetNetNodeCapacity {
        node_id: 2,
        link_speed_bps: 25_000_000_000,
        cycle_time_ns: 100_000,
        max_reservable_utilization: 0.85,
        propagation_delay_ns: 5_000,
        processing_delay_ns: 1_000,
    });
    engine.add_node(DetNetNodeCapacity {
        node_id: 3,
        link_speed_bps: 25_000_000_000,
        cycle_time_ns: 100_000,
        max_reservable_utilization: 0.85,
        propagation_delay_ns: 5_000,
        processing_delay_ns: 1_000,
    });
    engine.add_node(DetNetNodeCapacity {
        node_id: 4,
        link_speed_bps: 10_000_000_000,
        cycle_time_ns: 100_000,
        max_reservable_utilization: 0.75,
        propagation_delay_ns: 10_000,
        processing_delay_ns: 1_500,
    });

    let path = vec![1, 2, 3, 4];

    // Flow A: Ultra-low latency industrial robotic control (500 Mbps, 1 ms max latency)
    let flow_a = DetNetFlowSpec {
        flow_id: 5001,
        traffic_class: 7,
        peak_data_rate_bps: 500_000_000, // 500 Mbps
        max_payload_bytes: 512,
        max_burst_bytes: 1024,
        max_tolerable_latency_ns: 1_500_000, // 1.5 ms
        max_tolerable_jitter_ns: 600_000,    // 600 µs
    };

    let decision_a = engine.request_admission(flow_a, path.clone());
    match decision_a {
        DetNetAdmissionDecision::Admitted {
            flow_id,
            assigned_bandwidth_bps,
            guaranteed_latency_ns,
            guaranteed_jitter_ns,
            over_provisioning_factor,
        } => {
            assert_eq!(flow_id, 5001);
            assert_eq!(assigned_bandwidth_bps, 500_000_000);
            assert!(guaranteed_latency_ns < 1_500_000);
            assert_eq!(guaranteed_jitter_ns, 400_000); // 4 hops * 100 µs
            assert!(over_provisioning_factor >= 1.0);
        }
        DetNetAdmissionDecision::Rejected { cause, .. } => {
            panic!("Flow A should be admitted: {}", cause)
        }
    }

    // Flow B: Strict latency SLA that is too tight for the 4-hop physical pipeline (e.g. 100 us)
    let flow_unrealistic = DetNetFlowSpec {
        flow_id: 5002,
        traffic_class: 7,
        peak_data_rate_bps: 100_000_000,
        max_payload_bytes: 256,
        max_burst_bytes: 512,
        max_tolerable_latency_ns: 100_000, // 100 µs (cannot satisfy 4 * 100 µs cycles)
        max_tolerable_jitter_ns: 500_000,
    };

    let decision_b = engine.request_admission(flow_unrealistic, path.clone());
    match decision_b {
        DetNetAdmissionDecision::Rejected { flow_id, cause, .. } => {
            assert_eq!(flow_id, 5002);
            assert!(cause.contains("exceeds flow tolerance"));
        }
        DetNetAdmissionDecision::Admitted { .. } => panic!("Unrealistic flow must be rejected"),
    }

    // Release flow A and verify node state is cleared
    assert!(engine.release_reservation(5001));
    for node_id in 1..=4 {
        assert_eq!(*engine.node_reserved_bandwidth.get(&node_id).unwrap(), 0);
    }
}
