use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lacp::{
    LacpPacket, LacpPortInfo, LinkAggregationGroup, LACP_STATE_ACTIVITY,
    LACP_STATE_AGGREGATION, LACP_STATE_COLLECTING, LACP_STATE_DISTRIBUTING,
    LACP_STATE_SYNCHRONIZATION,
};

#[test]
fn test_lacp_actor_partner_tlv_codec() {
    let actor = LacpPortInfo {
        system_priority: 32768,
        system_mac: MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
        key: 10,
        port_priority: 128,
        port_number: 1,
        state: LACP_STATE_ACTIVITY | LACP_STATE_AGGREGATION | LACP_STATE_SYNCHRONIZATION | LACP_STATE_COLLECTING | LACP_STATE_DISTRIBUTING,
    };

    let partner = LacpPortInfo {
        system_priority: 32768,
        system_mac: MacAddress([0x00, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
        key: 10,
        port_priority: 128,
        port_number: 2,
        state: LACP_STATE_ACTIVITY | LACP_STATE_AGGREGATION | LACP_STATE_SYNCHRONIZATION | LACP_STATE_COLLECTING | LACP_STATE_DISTRIBUTING,
    };

    let pkt = LacpPacket::build(actor.clone(), partner.clone());
    let raw = pkt.serialize();
    assert_eq!(raw.len(), 110);

    let parsed = LacpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.actor, actor);
    assert_eq!(parsed.partner, partner);
}

#[test]
fn test_lacp_5tuple_load_balancing() {
    let lag = LinkAggregationGroup::new(
        "bond0",
        vec!["eth0".to_string(), "eth1".to_string(), "eth2".to_string(), "eth3".to_string()],
        10,
    );

    let mut port_hits = std::collections::HashSet::new();
    for port in 1000..1050 {
        let slave = lag.select_slave_port(
            Ipv4Address::new(192, 168, 1, 100),
            Ipv4Address::new(192, 168, 1, 10),
            port,
            80,
        );
        port_hits.insert(slave.to_string());
    }

    // Load balancer should distribute traffic across multiple slave ports
    assert!(port_hits.len() > 1);
}
