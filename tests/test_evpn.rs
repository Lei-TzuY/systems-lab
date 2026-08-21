use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::evpn::{EvpnMacTable, EvpnNlri, RouteDistinguisher, BGP_AFI_L2VPN, BGP_SAFI_EVPN, EVPN_TYPE_INCLUSIVE_MULTICAST, EVPN_TYPE_MAC_IP_ADV};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_evpn_route_type_2_mac_ip_advertisement() {
    let rd = RouteDistinguisher::new(Ipv4Address::new(172, 16, 0, 1), 10);
    let mac = MacAddress([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);
    let ip = Ipv4Address::new(10, 100, 1, 25);
    let vni = 10001;

    let nlri = EvpnNlri::build_mac_ip(rd.clone(), mac, Some(ip), vni);
    let raw = nlri.serialize();

    let parsed = EvpnNlri::parse(&raw).unwrap();
    if let EvpnNlri::MacIpAdv(adv) = parsed {
        assert_eq!(adv.rd.to_string(), "172.16.0.1:10");
        assert_eq!(adv.mac, mac);
        assert_eq!(adv.ip, Some(ip));
        assert_eq!(adv.vni, 10001);

        let mut table = EvpnMacTable::new();
        let vtep = Ipv4Address::new(172, 16, 0, 1);
        table.learn_route(&adv, vtep);

        let lookup_res = table.lookup(10001, &mac).unwrap();
        assert_eq!(lookup_res.0, vtep);
        assert_eq!(lookup_res.1, Some(ip));
    } else {
        panic!("Expected MAC/IP advertisement");
    }

    assert_eq!(BGP_AFI_L2VPN, 25);
    assert_eq!(BGP_SAFI_EVPN, 70);
    assert_eq!(EVPN_TYPE_MAC_IP_ADV, 2);
}

#[test]
fn test_evpn_route_type_3_inclusive_multicast() {
    let rd = RouteDistinguisher::new(Ipv4Address::new(172, 16, 0, 2), 20);
    let orig_ip = Ipv4Address::new(172, 16, 0, 2);

    let nlri = EvpnNlri::build_inclusive_multicast(rd.clone(), orig_ip);
    let raw = nlri.serialize();

    let parsed = EvpnNlri::parse(&raw).unwrap();
    if let EvpnNlri::InclusiveMulticast(im) = parsed {
        assert_eq!(im.rd.to_string(), "172.16.0.2:20");
        assert_eq!(im.originating_router_ip, orig_ip);
    } else {
        panic!("Expected Inclusive Multicast");
    }

    assert_eq!(EVPN_TYPE_INCLUSIVE_MULTICAST, 3);
}
