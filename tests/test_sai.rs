use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::sai::{SAI_STATUS_SUCCESS, SaiFdbEntry, SaiRouteEntry, SaiSwitchAdapter};

#[test]
fn test_sai_adapter_l2_and_l3_forwarding() {
    let mut adapter = SaiSwitchAdapter::new(10);
    let mac = MacAddress([0x00, 0x1A, 0x2B, 0x3C, 0x4D, 0x5E]);

    // Create FDB Entry
    let status = adapter.create_fdb_entry(mac, 20, 8);
    assert_eq!(status, SAI_STATUS_SUCCESS);
    assert_eq!(adapter.lookup_fdb(mac, 20), Some(8));
    assert_eq!(adapter.lookup_fdb(mac, 30), None);

    // Create NextHop and Route
    let nh_id = adapter.create_next_hop(
        Ipv4Address::new(172, 16, 1, 254),
        MacAddress([0x00, 0x50, 0x56, 0x00, 0x00, 0x01]),
        4,
    );
    adapter.create_route_entry(0, Ipv4Address::new(172, 16, 0, 0), 16, nh_id);

    // Lookup Route
    let matched = adapter
        .lookup_route(0, Ipv4Address::new(172, 16, 99, 100))
        .unwrap();
    assert_eq!(matched.id, nh_id);
    assert_eq!(matched.port_id, 4);
    assert_eq!(matched.ip, Ipv4Address::new(172, 16, 1, 254));
}

#[test]
fn test_sai_entry_equality_and_hash() {
    let entry1 = SaiFdbEntry {
        switch_id: 1,
        mac_address: MacAddress([0x11; 6]),
        bv_id: 10,
    };
    let entry2 = SaiFdbEntry {
        switch_id: 1,
        mac_address: MacAddress([0x11; 6]),
        bv_id: 10,
    };
    assert_eq!(entry1, entry2);

    let route1 = SaiRouteEntry {
        switch_id: 1,
        vr_id: 0,
        destination: Ipv4Address::new(10, 0, 0, 0),
        prefix_len: 24,
    };
    let route2 = SaiRouteEntry {
        switch_id: 1,
        vr_id: 0,
        destination: Ipv4Address::new(10, 0, 0, 0),
        prefix_len: 24,
    };
    assert_eq!(route1, route2);
}
