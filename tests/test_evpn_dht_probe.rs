use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_dht_probe::{DhtTickAction, EvpnDhtEngine, HostTrackingState};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_dht_probing_lifecycle() {
    let mut dht = EvpnDhtEngine::new(20, 2); // 20s inactivity, 2 retries

    let mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
    let ip = Ipv4Address::new(10, 1, 1, 100);

    // 1. Host registers at t=100
    dht.touch_host(200, 2, mac, ip, 100);
    assert_eq!(dht.hosts.len(), 1);

    // 2. Tick at t=110 (elapsed 10s < 20s) -> No action
    assert!(dht.tick(110).is_empty());

    // 3. Tick at t=125 (elapsed 25s >= 20s) -> Probing 1st probe
    let a1 = dht.tick(125);
    assert_eq!(a1.len(), 1);
    assert_eq!(
        a1[0],
        DhtTickAction::SendUnicastProbe {
            vni: 200,
            port_id: 2,
            target_mac: mac,
            target_ip: ip,
        }
    );

    // 4. Host responds to probe at t=126
    dht.touch_host(200, 2, mac, ip, 126);
    assert_eq!(dht.hosts[0].state, HostTrackingState::Active);

    // 5. Host becomes silent again at t=150
    let a2 = dht.tick(150);
    assert_eq!(a2.len(), 1); // Probe 1

    let a3 = dht.tick(155);
    assert_eq!(a3.len(), 1); // Probe 2 (retry exhausted)

    let a4 = dht.tick(160);
    assert_eq!(a4.len(), 1); // Host dead -> withdraw
    assert_eq!(
        a4[0],
        DhtTickAction::WithdrawHost {
            vni: 200,
            port_id: 2,
            mac,
            ip,
        }
    );
    assert!(dht.hosts.is_empty());
}
