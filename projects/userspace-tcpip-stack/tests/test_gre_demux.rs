use toy_tcpip::gre_demux::{GreDemuxTable, GreVirtualTunnel};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gre_demux_multi_tenant_isolation_and_replay_protection() {
    let mut demux = GreDemuxTable::new();

    let remote_router = Ipv4Address::new(203, 0, 113, 20);

    // Tunnel 1: VRF 100 (Financial Dept) - Strict Sequence Enabled
    demux.register_tunnel(GreVirtualTunnel {
        if_name: "gre100".to_string(),
        vrf_id: 100,
        local_ip: Ipv4Address::new(192, 168, 1, 1),
        remote_ip: remote_router,
        key: 0x00000064,
        strict_sequence: true,
    });

    // Tunnel 2: VRF 200 (Engineering Dept) - Strict Sequence Disabled
    demux.register_tunnel(GreVirtualTunnel {
        if_name: "gre200".to_string(),
        vrf_id: 200,
        local_ip: Ipv4Address::new(192, 168, 1, 1),
        remote_ip: remote_router,
        key: 0x000000C8,
        strict_sequence: false,
    });

    // Packet 1: VRF 100 Seq 1 -> Accept
    let res1 = demux.demux_packet(remote_router, Some(0x64), Some(1), b"Data 1");
    assert!(res1.is_some());
    let (iface, vrf, _) = res1.unwrap();
    assert_eq!(iface, "gre100");
    assert_eq!(vrf, 100);

    // Packet 2: VRF 100 Seq 2 -> Accept
    let res2 = demux.demux_packet(remote_router, Some(0x64), Some(2), b"Data 2");
    assert!(res2.is_some());

    // Packet 3: VRF 100 Seq 1 (Replay Attack) -> Drop
    let res3 = demux.demux_packet(remote_router, Some(0x64), Some(1), b"Replayed Data");
    assert_eq!(res3, None);

    // Packet 4: VRF 200 Seq 1 -> Accept
    let res4 = demux.demux_packet(remote_router, Some(0xC8), Some(1), b"Eng Data");
    assert!(res4.is_some());
    let (iface_eng, vrf_eng, _) = res4.unwrap();
    assert_eq!(iface_eng, "gre200");
    assert_eq!(vrf_eng, 200);

    // Packet 5: Unregistered Key 0x999 -> Drop
    let res5 = demux.demux_packet(remote_router, Some(0x999), Some(1), b"Unknown");
    assert_eq!(res5, None);
}
