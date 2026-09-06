use toy_tcpip::bus::VirtualNetworkBus;
use toy_tcpip::ethernet::{ETHERTYPE_IPV4, EthernetFrame, MacAddress};
use toy_tcpip::icmp::IcmpPacket;
use toy_tcpip::ipv4::{IP_PROTO_ICMP, Ipv4Address, Ipv4Packet};
use toy_tcpip::stack::{NetStack, NetStackConfig};

#[test]
fn test_virtual_bus_multi_node_communication() {
    let mut bus = VirtualNetworkBus::new();

    let node1_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x01]);
    let node1_ip = Ipv4Address::new(192, 168, 1, 1);
    bus.add_node(
        "node1",
        NetStack::new(NetStackConfig {
            mac: node1_mac,
            ip: node1_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        }),
    );

    let node2_mac = MacAddress([0x00, 0x11, 0x22, 0x33, 0x44, 0x02]);
    let node2_ip = Ipv4Address::new(192, 168, 1, 2);
    bus.add_node(
        "node2",
        NetStack::new(NetStackConfig {
            mac: node2_mac,
            ip: node2_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        }),
    );

    // Send Ping from Node 1 to Node 2
    let icmp_req = IcmpPacket::build_echo_request(0x1000, 1, b"Bus Ping Test");
    let ip_req = Ipv4Packet::serialize(node1_ip, node2_ip, IP_PROTO_ICMP, 1, 64, &icmp_req);
    let eth_req = EthernetFrame::serialize(node2_mac, node1_mac, ETHERTYPE_IPV4, &ip_req);

    let count = bus.send_frame("node1", eth_req);
    assert_eq!(count, 2);
}
