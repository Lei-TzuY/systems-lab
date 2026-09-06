use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_dai_inspection::{DaiBinding, DaiVerdict, EvpnDaiEngine};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_dai_inspection_lifecycle() {
    let mut dai = EvpnDaiEngine::new(5); // 5 pps

    let vni = 200;
    let access_port = 3;
    let core_uplink = 12;
    let valid_mac = MacAddress([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
    let valid_ip = Ipv4Address::new(172, 16, 20, 100);

    dai.set_port_trusted(core_uplink, true);
    dai.add_binding(DaiBinding {
        vni,
        port_id: access_port,
        mac: valid_mac,
        ip: valid_ip,
    });

    // 1. Valid ARP on untrusted access port -> Permit
    let v1 = dai.inspect_arp(vni, access_port, valid_mac, valid_mac, valid_ip, 1000);
    assert_eq!(v1, DaiVerdict::Permit);

    // 2. ARP packet arriving on trusted core uplink -> Bypass
    let v2 = dai.inspect_arp(vni, core_uplink, valid_mac, valid_mac, valid_ip, 1000);
    assert_eq!(v2, DaiVerdict::BypassTrusted);

    // 3. Sender MAC mismatch against Ethernet header -> DropMacMismatch
    let spoof_eth_mac = MacAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let v3 = dai.inspect_arp(vni, access_port, spoof_eth_mac, valid_mac, valid_ip, 1000);
    assert_eq!(v3, DaiVerdict::DropMacMismatch);

    // 4. Spoofed IP address -> DropBindingNotFound
    let unassigned_ip = Ipv4Address::new(172, 16, 20, 254);
    let v4 = dai.inspect_arp(vni, access_port, valid_mac, valid_mac, unassigned_ip, 1000);
    assert_eq!(v4, DaiVerdict::DropBindingNotFound);

    // 5. Rate limit burst exhaustion
    for _ in 0..10 {
        let _ = dai.inspect_arp(vni, access_port, valid_mac, valid_mac, valid_ip, 1000);
    }
    let v_burst = dai.inspect_arp(vni, access_port, valid_mac, valid_mac, valid_ip, 1000);
    assert_eq!(v_burst, DaiVerdict::DropRateLimitExceeded);
}
