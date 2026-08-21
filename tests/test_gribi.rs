use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::gribi::{
    GribiAftTable, GribiIpv4Entry, GribiNextHop, GribiNextHopGroup, GRIBI_PORT, GRIBI_VERSION,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gribi_sdn_aft_operations_and_resolution() {
    let mut aft = GribiAftTable::new();

    let nh1 = GribiNextHop {
        id: 101,
        ip: Ipv4Address::new(10, 0, 0, 1),
        mac: MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x01]),
        weight: 50,
    };

    let nhg = GribiNextHopGroup {
        id: 501,
        next_hop_ids: vec![101],
    };

    let entry = GribiIpv4Entry {
        prefix: Ipv4Address::new(172, 16, 0, 0),
        prefix_len: 12,
        next_hop_group_id: 501,
    };

    aft.set_next_hop(nh1);
    aft.set_next_hop_group(nhg);
    aft.set_ipv4_entry(entry);

    let res = aft.resolve_fib(Ipv4Address::new(172, 20, 5, 6)).unwrap();
    assert_eq!(res.id, 101);
    assert_eq!(res.ip, Ipv4Address::new(10, 0, 0, 1));
}

#[test]
fn test_gribi_constants() {
    assert_eq!(GRIBI_PORT, 9340);
    assert_eq!(GRIBI_VERSION, "0.1.0");
}
