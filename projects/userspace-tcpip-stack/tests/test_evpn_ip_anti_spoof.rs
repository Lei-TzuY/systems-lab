use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn_ip_anti_spoof::{
    AntiSpoofVerdict, EvpnIpAntiSpoofEngine, IpSourceBinding, PortTrustMode,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_ip_anti_spoof_lifecycle() {
    let mut engine = EvpnIpAntiSpoofEngine::new();

    let vni = 200;
    let access_port = 1;
    let core_port = 2;

    engine.set_port_trust_mode(access_port, PortTrustMode::Untrusted);
    engine.set_port_trust_mode(core_port, PortTrustMode::Trusted);

    let vm1_mac = MacAddress([0x00, 0x50, 0x56, 0xAA, 0xBB, 0x01]);
    let vm1_ip = Ipv4Address::new(172, 16, 1, 10);

    let vm2_mac = MacAddress([0x00, 0x50, 0x56, 0xAA, 0xBB, 0x02]);
    let vm2_ip = Ipv4Address::new(172, 16, 1, 20);

    // Register authorized VM1 binding
    engine.add_binding(IpSourceBinding {
        vni,
        port_id: access_port,
        mac: vm1_mac,
        ip: vm1_ip,
        is_static: true,
    });

    // 1. Authorized packet from VM1
    let v1 = engine.evaluate_ingress(vni, access_port, vm1_mac, vm1_ip);
    assert_eq!(v1, AntiSpoofVerdict::Forward);

    // 2. VM1 attempting to spoof VM2's IP address (IP spoofing)
    let v2 = engine.evaluate_ingress(vni, access_port, vm1_mac, vm2_ip);
    assert_eq!(v2, AntiSpoofVerdict::DropUnbound);

    // Register VM2 on a different port/MAC
    engine.add_binding(IpSourceBinding {
        vni,
        port_id: 3,
        mac: vm2_mac,
        ip: vm2_ip,
        is_static: true,
    });

    // 3. Attacker on access_port attempting to claim VM2's IP (IP spoofing across ports)
    let v3 = engine.evaluate_ingress(vni, access_port, vm1_mac, vm2_ip);
    assert_eq!(v3, AntiSpoofVerdict::DropSpoofedIp);

    // 4. Rogue MAC claiming VM1's IP on access_port (MAC spoofing)
    let rogue_mac = MacAddress([0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01]);
    let v4 = engine.evaluate_ingress(vni, access_port, rogue_mac, vm1_ip);
    assert_eq!(v4, AntiSpoofVerdict::DropSpoofedMac);

    // 5. Ingress on trusted core port (anti-spoof filter bypassed)
    let v5 = engine.evaluate_ingress(vni, core_port, rogue_mac, vm1_ip);
    assert_eq!(v5, AntiSpoofVerdict::Forward);

    // Verify telemetry
    assert_eq!(engine.stats.total_evaluated, 5);
    assert_eq!(engine.stats.total_forwarded, 2);
    assert_eq!(engine.stats.total_spoofed_ip_drops, 1);
    assert_eq!(engine.stats.total_spoofed_mac_drops, 1);
    assert_eq!(engine.stats.total_unbound_drops, 1);
}
