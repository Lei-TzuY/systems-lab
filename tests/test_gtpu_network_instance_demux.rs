// tests/test_gtpu_network_instance_demux.rs

use toy_tcpip::gtpu_network_instance_demux::{
    GtpuNetworkInstanceDemuxEngine, NetworkInstanceDemuxVerdict,
};

#[test]
fn test_gtpu_network_instance_demux_integration() {
    let mut engine = GtpuNetworkInstanceDemuxEngine::new();
    engine.register_dnn_profile("private-5g-factory", 50, 5050, 500_000_000);
    engine.bind_teid_to_dnn(0x20001, "private-5g-factory", 82);
    engine.bind_teid_to_dnn(0x10001, "internet", 9);

    // 1. Demux conforming packet to Private 5G Factory VRF
    let v1 = engine.demux_packet(0x20001, 1024, Some("private-5g-factory"));
    assert_eq!(
        v1,
        NetworkInstanceDemuxVerdict::RoutedToTenantVrf {
            teid: 0x20001,
            dnn_name: "private-5g-factory".to_string(),
            vrf_id: 50,
            network_instance_id: 5050,
            qfi: 82,
            payload_bytes: 1024,
        }
    );

    // 2. Demux Internet packet without claimed DNN
    let v2 = engine.demux_packet(0x10001, 1400, None);
    assert_eq!(
        v2,
        NetworkInstanceDemuxVerdict::RoutedToTenantVrf {
            teid: 0x10001,
            dnn_name: "internet".to_string(),
            vrf_id: 1,
            network_instance_id: 1001,
            qfi: 9,
            payload_bytes: 1400,
        }
    );

    // 3. Security violation: tenant mismatch
    let v_sec = engine.demux_packet(0x10001, 500, Some("enterprise-iot"));
    assert_eq!(
        v_sec,
        NetworkInstanceDemuxVerdict::SecurityViolationTenantMismatch {
            teid: 0x10001,
            expected_dnn: "internet".to_string(),
            injected_dnn: "enterprise-iot".to_string(),
        }
    );

    // 4. Unmapped TEID
    let v_unmapped = engine.demux_packet(0xdeadbeef, 800, None);
    assert_eq!(
        v_unmapped,
        NetworkInstanceDemuxVerdict::UnmappedTeidDrop { teid: 0xdeadbeef }
    );

    assert_eq!(engine.total_packets_demuxed, 2);
    assert_eq!(engine.total_bytes_demuxed, 2424);
    assert_eq!(engine.total_security_violations, 1);
    assert_eq!(engine.total_unmapped_teid_drops, 1);
}
