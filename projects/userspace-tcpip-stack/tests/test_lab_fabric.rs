//! Multi-Layer Overlay Fabric & Transport Integration Tests.
//!
//! Validates:
//! - Scenario 1: VXLAN L2 Overlay over Multi-Hop Underlay IP Network (Leaf-Spine-Leaf Fabric)
//! - Scenario 2: OSPFv2 Link-State Topology & Dijkstra SPF Convergence
//! - Scenario 3: Router Stateful Firewall Policy Filtering & Packet Drop
//! - Scenario 4: MPLS 3-Node Label Switched Path (Push -> Swap -> Pop) End-to-End Data Plane

use std::collections::HashMap;
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::firewall::{Firewall, FirewallAction, FirewallChain, FirewallRule};
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::stack::NetStackConfig;

#[test]
fn test_scenario_1_vxlan_overlay_fabric() {
    let mut lab = VirtualLab::new();

    // Tenant Hosts (L2 Subnet 192.168.100.0/24, VNI 5001)
    let host1_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x10]);
    let host1_ip = Ipv4Address::new(192, 168, 100, 10);

    let host2_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x20]);
    let host2_ip = Ipv4Address::new(192, 168, 100, 20);

    lab.add_host(
        "tenant_host1",
        "access_link_1",
        NetStackConfig {
            mac: host1_mac,
            ip: host1_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "tenant_host2",
        "access_link_2",
        NetStackConfig {
            mac: host2_mac,
            ip: host2_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Leaf 1 VTEP (Underlay IP 10.0.1.1)
    let leaf1_underlay_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]);
    let leaf1_underlay_ip = Ipv4Address::new(10, 0, 1, 1);
    let mut leaf1 = LabRouter::new("leaf1");
    leaf1.add_interface(
        "eth_access",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0xAA]),
        Ipv4Address::new(192, 168, 100, 254),
        24,
        "access_link_1",
    );
    leaf1.add_interface(
        "eth_underlay",
        leaf1_underlay_mac,
        leaf1_underlay_ip,
        24,
        "underlay_link_1",
    );
    leaf1.routing_table.add_route(
        Ipv4Address::new(10, 0, 2, 0),
        24,
        Some(Ipv4Address::new(10, 0, 1, 254)),
        "eth_underlay",
    );
    leaf1.add_vxlan_tunnel(
        "eth_access",
        5001,
        Ipv4Address::new(10, 0, 2, 1),
        "eth_underlay",
    );
    lab.add_router(leaf1);

    // Spine Underlay Router
    let spine_mac1 = MacAddress([0x02, 0x00, 0x00, 0x00, 0x55, 0x01]);
    let spine_mac2 = MacAddress([0x02, 0x00, 0x00, 0x00, 0x55, 0x02]);
    let mut spine = LabRouter::new("spine");
    spine.add_interface(
        "spine_if1",
        spine_mac1,
        Ipv4Address::new(10, 0, 1, 254),
        24,
        "underlay_link_1",
    );
    spine.add_interface(
        "spine_if2",
        spine_mac2,
        Ipv4Address::new(10, 0, 2, 254),
        24,
        "underlay_link_2",
    );
    lab.add_router(spine);

    // Leaf 2 VTEP (Underlay IP 10.0.2.1)
    let leaf2_underlay_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]);
    let leaf2_underlay_ip = Ipv4Address::new(10, 0, 2, 1);
    let mut leaf2 = LabRouter::new("leaf2");
    leaf2.add_interface(
        "eth_underlay",
        leaf2_underlay_mac,
        leaf2_underlay_ip,
        24,
        "underlay_link_2",
    );
    leaf2.add_interface(
        "eth_access",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0xAA]),
        Ipv4Address::new(192, 168, 100, 253),
        24,
        "access_link_2",
    );
    leaf2.routing_table.add_route(
        Ipv4Address::new(10, 0, 1, 0),
        24,
        Some(Ipv4Address::new(10, 0, 2, 254)),
        "eth_underlay",
    );
    leaf2.add_vxlan_tunnel(
        "eth_access",
        5001,
        Ipv4Address::new(10, 0, 1, 1),
        "eth_underlay",
    );
    lab.add_router(leaf2);

    // Pre-populate underlay ARP caches for instantaneous underlay packet delivery
    lab.router_mut("leaf1")
        .unwrap()
        .arp_tables
        .get_mut("eth_underlay")
        .unwrap()
        .insert([10, 0, 1, 254], spine_mac1);
    lab.router_mut("spine")
        .unwrap()
        .arp_tables
        .get_mut("spine_if1")
        .unwrap()
        .insert([10, 0, 1, 1], leaf1_underlay_mac);
    lab.router_mut("spine")
        .unwrap()
        .arp_tables
        .get_mut("spine_if2")
        .unwrap()
        .insert([10, 0, 2, 1], leaf2_underlay_mac);
    lab.router_mut("leaf2")
        .unwrap()
        .arp_tables
        .get_mut("eth_underlay")
        .unwrap()
        .insert([10, 0, 2, 254], spine_mac2);

    // Tenant Host 1 pings Tenant Host 2 across VXLAN L2 Overlay
    let ping_frame = lab
        .host_mut("tenant_host1")
        .unwrap()
        .stack
        .ping4(host2_ip, 0x5566, 1, b"VXLAN_OVERLAY_PING")
        .expect("Ping frame");

    lab.send_from_host("tenant_host1", ping_frame);
    lab.run_until_quiescent(30);

    // Verify Tenant Host 1 received ICMP Echo Reply from Tenant Host 2!
    let host1 = lab.host("tenant_host1").unwrap();
    assert_eq!(host1.stack.received_icmp_replies.len(), 1);
    assert_eq!(host1.stack.received_icmp_replies[0].0, host2_ip);
}

#[test]
fn test_scenario_2_ospf_dijkstra_convergence_and_routing() {
    let mut lab = VirtualLab::new();

    let h_a_ip = Ipv4Address::new(172, 16, 1, 10);
    let h_c_ip = Ipv4Address::new(172, 16, 3, 30);

    lab.add_host(
        "host_a",
        "link_a",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]),
            ip: h_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(172, 16, 1, 1)),
        },
    );

    lab.add_host(
        "host_c",
        "link_c",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x30]),
            ip: h_c_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(172, 16, 3, 1)),
        },
    );

    // Router 1 (ID 1.1.1.1)
    let mut r1 = LabRouter::new("r1");
    r1.add_interface(
        "r1_lan",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
        Ipv4Address::new(172, 16, 1, 1),
        24,
        "link_a",
    );
    r1.add_interface(
        "r1_r2",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x01]),
        Ipv4Address::new(10, 1, 2, 1),
        24,
        "link_r1_r2",
    );
    r1.add_interface(
        "r1_r3",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x13, 0x01]),
        Ipv4Address::new(10, 1, 3, 1),
        24,
        "link_r1_r3",
    );
    r1.enable_ospf();
    // Add links: R1-R2 cost 10, R2-R3 cost 10, R1-R3 cost 50
    r1.add_ospf_link(
        Ipv4Address::new(1, 1, 1, 1),
        Ipv4Address::new(2, 2, 2, 2),
        10,
    );
    r1.add_ospf_link(
        Ipv4Address::new(2, 2, 2, 2),
        Ipv4Address::new(3, 3, 3, 3),
        10,
    );
    r1.add_ospf_link(
        Ipv4Address::new(1, 1, 1, 1),
        Ipv4Address::new(3, 3, 3, 3),
        50,
    );
    lab.add_router(r1);

    // Router 2 (ID 2.2.2.2)
    let mut r2 = LabRouter::new("r2");
    r2.add_interface(
        "r2_r1",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x02]),
        Ipv4Address::new(10, 1, 2, 2),
        24,
        "link_r1_r2",
    );
    r2.add_interface(
        "r2_r3",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x02]),
        Ipv4Address::new(10, 2, 3, 2),
        24,
        "link_r2_r3",
    );
    r2.enable_ospf();
    r2.add_ospf_link(
        Ipv4Address::new(1, 1, 1, 1),
        Ipv4Address::new(2, 2, 2, 2),
        10,
    );
    r2.add_ospf_link(
        Ipv4Address::new(2, 2, 2, 2),
        Ipv4Address::new(3, 3, 3, 3),
        10,
    );
    lab.add_router(r2);

    // Router 3 (ID 3.3.3.3)
    let mut r3 = LabRouter::new("r3");
    r3.add_interface(
        "r3_r2",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x03]),
        Ipv4Address::new(10, 2, 3, 3),
        24,
        "link_r2_r3",
    );
    r3.add_interface(
        "r3_r1",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x13, 0x03]),
        Ipv4Address::new(10, 1, 3, 3),
        24,
        "link_r1_r3",
    );
    r3.add_interface(
        "r3_lan",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x01]),
        Ipv4Address::new(172, 16, 3, 1),
        24,
        "link_c",
    );
    r3.enable_ospf();
    r3.add_ospf_link(
        Ipv4Address::new(2, 2, 2, 2),
        Ipv4Address::new(3, 3, 3, 3),
        10,
    );
    r3.add_ospf_link(
        Ipv4Address::new(1, 1, 1, 1),
        Ipv4Address::new(3, 3, 3, 3),
        50,
    );
    lab.add_router(r3);

    // Run Dijkstra Shortest Path calculation
    let mut neighbor_subnets_r1 = HashMap::new();
    neighbor_subnets_r1.insert(
        Ipv4Address::new(3, 3, 3, 3),
        (
            Ipv4Address::new(172, 16, 3, 0),
            24,
            "r1_r2".to_string(),
            Ipv4Address::new(10, 1, 2, 2),
        ),
    );
    lab.router_mut("r1")
        .unwrap()
        .run_ospf_spf(Ipv4Address::new(1, 1, 1, 1), &neighbor_subnets_r1);

    let mut neighbor_subnets_r2 = HashMap::new();
    neighbor_subnets_r2.insert(
        Ipv4Address::new(3, 3, 3, 3),
        (
            Ipv4Address::new(172, 16, 3, 0),
            24,
            "r2_r3".to_string(),
            Ipv4Address::new(10, 2, 3, 3),
        ),
    );
    neighbor_subnets_r2.insert(
        Ipv4Address::new(1, 1, 1, 1),
        (
            Ipv4Address::new(172, 16, 1, 0),
            24,
            "r2_r1".to_string(),
            Ipv4Address::new(10, 1, 2, 1),
        ),
    );
    lab.router_mut("r2")
        .unwrap()
        .run_ospf_spf(Ipv4Address::new(2, 2, 2, 2), &neighbor_subnets_r2);

    let mut neighbor_subnets_r3 = HashMap::new();
    neighbor_subnets_r3.insert(
        Ipv4Address::new(1, 1, 1, 1),
        (
            Ipv4Address::new(172, 16, 1, 0),
            24,
            "r3_r2".to_string(),
            Ipv4Address::new(10, 2, 3, 2),
        ),
    );
    lab.router_mut("r3")
        .unwrap()
        .run_ospf_spf(Ipv4Address::new(3, 3, 3, 3), &neighbor_subnets_r3);

    // Verify R1 chose R2 (cost 10+10=20) instead of high-cost direct link R3 (cost 50)
    let route_c = lab.router("r1").unwrap().routing_table.lookup(h_c_ip);
    assert!(route_c.is_some());
    assert_eq!(
        route_c.unwrap().next_hop(h_c_ip),
        Ipv4Address::new(10, 1, 2, 2)
    );

    // End-to-end ping across SPF calculated path
    let ping = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(h_c_ip, 0x8899, 1, b"OSPF_SPF_PING")
        .expect("Ping frame");
    lab.send_from_host("host_a", ping);
    lab.run_until_quiescent(30);

    let host_a = lab.host("host_a").unwrap();
    assert_eq!(host_a.stack.received_icmp_replies.len(), 1);
    assert_eq!(host_a.stack.received_icmp_replies[0].0, h_c_ip);
}

#[test]
fn test_scenario_3_router_stateful_firewall_filtering() {
    let mut lab = VirtualLab::new();

    let client_ip = Ipv4Address::new(10, 0, 1, 5);
    let server_ip = Ipv4Address::new(10, 0, 2, 80);

    lab.add_host(
        "client",
        "link_lan",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x05]),
            ip: client_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(10, 0, 1, 1)),
        },
    );

    lab.add_host(
        "server",
        "link_wan",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x80]),
            ip: server_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(10, 0, 2, 1)),
        },
    );

    // Setup UDP listeners on server: Port 80 (HTTP) and Port 23 (Telnet)
    lab.host_mut("server")
        .unwrap()
        .stack
        .udp_sockets
        .bind(80, |_src, _port, data| {
            let mut resp = b"HTTP_OK:".to_vec();
            resp.extend_from_slice(data);
            Some(resp)
        });

    lab.host_mut("server")
        .unwrap()
        .stack
        .udp_sockets
        .bind(23, |_src, _port, data| {
            let mut resp = b"TELNET_OK:".to_vec();
            resp.extend_from_slice(data);
            Some(resp)
        });

    // Router with Firewall
    let mut router = LabRouter::new("firewall_router");
    router.add_interface(
        "lan",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
        Ipv4Address::new(10, 0, 1, 1),
        24,
        "link_lan",
    );
    router.add_interface(
        "wan",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
        Ipv4Address::new(10, 0, 2, 1),
        24,
        "link_wan",
    );

    // Configure Firewall Rule: DROP any forwarding packet destined to port 23
    let mut fw = Firewall::new();
    fw.add_rule(
        FirewallChain::Forward,
        FirewallRule {
            description: "Block Telnet Port 23".to_string(),
            src_cidr: None,
            dst_cidr: None,
            protocol: Some(toy_tcpip::ipv4::IP_PROTO_UDP),
            src_port_range: None,
            dst_port_range: Some((23, 23)),
            action: FirewallAction::Drop,
        },
    );
    router.set_firewall(fw);
    lab.add_router(router);

    // 1. Client sends UDP query to Port 80 -> Must be PERMITTED
    let query_80 = lab
        .host_mut("client")
        .unwrap()
        .stack
        .send_udp(server_ip, 40000, 80, b"WEB_REQ")
        .unwrap();
    lab.send_from_host("client", query_80);
    lab.run_until_quiescent(20);

    let client = lab.host("client").unwrap();
    assert_eq!(client.stack.received_udp_payloads.len(), 1);
    assert_eq!(client.stack.received_udp_payloads[0].3, b"HTTP_OK:WEB_REQ");

    // 2. Client sends UDP query to Port 23 -> Must be DROPPED by firewall
    let query_23 = lab
        .host_mut("client")
        .unwrap()
        .stack
        .send_udp(server_ip, 40001, 23, b"TELNET_REQ")
        .unwrap();
    lab.send_from_host("client", query_23);
    lab.run_until_quiescent(20);

    let server = lab.host("server").unwrap();
    // Server must NOT have received any payload on port 23
    assert!(
        !server
            .stack
            .received_udp_payloads
            .iter()
            .any(|(_, _, dst_port, _)| *dst_port == 23)
    );
}

#[test]
fn test_scenario_4_mpls_3node_lsp_data_plane() {
    let mut lab = VirtualLab::new();

    let host_a_ip = Ipv4Address::new(192, 168, 1, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 2, 20);

    lab.add_host(
        "host_a",
        "link_customer_a",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]),
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
        },
    );

    lab.add_host(
        "host_b",
        "link_customer_b",
        NetStackConfig {
            mac: MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x20]),
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(Ipv4Address::new(192, 168, 2, 1)),
        },
    );

    // Ingress LSR (R1): Pushes Label 100 on packets to Host B
    let mut r1 = LabRouter::new("r1_ingress");
    r1.add_interface(
        "r1_cust",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]),
        Ipv4Address::new(192, 168, 1, 1),
        24,
        "link_customer_a",
    );
    r1.add_interface(
        "r1_core",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x01]),
        Ipv4Address::new(10, 0, 12, 1),
        24,
        "core_link_12",
    );
    r1.enable_mpls();
    r1.add_mpls_push_route(host_b_ip, 100, "r1_core");
    lab.add_router(r1);

    // Core LSR (R2): Swaps Label 100 -> 200
    let mut r2 = LabRouter::new("r2_core");
    r2.add_interface(
        "r2_in",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x12, 0x02]),
        Ipv4Address::new(10, 0, 12, 2),
        24,
        "core_link_12",
    );
    r2.add_interface(
        "r2_out",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x02]),
        Ipv4Address::new(10, 0, 23, 2),
        24,
        "core_link_23",
    );
    r2.enable_mpls();
    r2.add_mpls_swap_route(100, 200, "r2_out");
    lab.add_router(r2);

    // Egress LSR (R3): Pops Label 200 (PHP / Disposition)
    let mut r3 = LabRouter::new("r3_egress");
    r3.add_interface(
        "r3_core",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x23, 0x03]),
        Ipv4Address::new(10, 0, 23, 3),
        24,
        "core_link_23",
    );
    r3.add_interface(
        "r3_cust",
        MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]),
        Ipv4Address::new(192, 168, 2, 1),
        24,
        "link_customer_b",
    );
    r3.enable_mpls();
    r3.add_mpls_pop_route(200);
    lab.add_router(r3);

    // Host A sends UDP packet to Host B through MPLS LSP (R1 PUSH 100 -> R2 SWAP 200 -> R3 POP)
    let data_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .send_udp(host_b_ip, 30000, 9000, b"MPLS_LSP_DATA_PLANE")
        .unwrap();

    lab.send_from_host("host_a", data_frame);
    lab.run_until_quiescent(25);

    // Verify Host B received the transparently transported customer packet
    let host_b = lab.host("host_b").unwrap();
    assert_eq!(host_b.stack.received_udp_payloads.len(), 1);
    assert_eq!(
        host_b.stack.received_udp_payloads[0].3,
        b"MPLS_LSP_DATA_PLANE"
    );
}
