//! Advanced Virtual Network Lab Integration Tests.
//!
//! Validates:
//! - Scenario 1: DHCPv4 DORA auto-configuration & ping after lease acquisition
//! - Scenario 2: Router NAT SNAT (Masquerade) & DNAT (Connection Tracking)
//! - Scenario 3: RIPv2 Multi-Router dynamic convergence and multi-hop routing
//! - Scenario 4: TCP Out-of-Order segment queuing and sliding window reassembly

use toy_tcpip::dhcp::{DhcpMessageType, DhcpServer};
use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::stack::NetStackConfig;
use toy_tcpip::tcp::{SocketAddrV4, TcpConnection};

#[test]
fn test_scenario_1_dhcp_dora_host_autoconfiguration() {
    let mut lab = VirtualLab::new();

    let server_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x01]);
    let server_ip = Ipv4Address::new(192, 168, 1, 1);
    let client_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x50]);

    // 1. Configure DHCP Server Host
    lab.add_host(
        "dhcp_server",
        "lan_dhcp",
        NetStackConfig {
            mac: server_mac,
            ip: server_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );
    lab.host_mut("dhcp_server").unwrap().stack.dhcp_server = Some(DhcpServer::new(
        server_ip,
        Ipv4Address::new(255, 255, 255, 0),
        server_ip,
        Ipv4Address::new(8, 8, 8, 8),
        Ipv4Address::new(192, 168, 1, 100),
        Ipv4Address::new(192, 168, 1, 200),
        86400,
    ));

    // 2. Configure unconfigured Client Host (0.0.0.0)
    lab.add_host(
        "client",
        "lan_dhcp",
        NetStackConfig {
            mac: client_mac,
            ip: Ipv4Address::UNSPECIFIED,
            ipv6: None,
            subnet_mask: 0,
            gateway: None,
        },
    );

    // 3. Client initiates DHCP Discover
    let xid = 0x12345678;
    let disc_frame = lab.host_mut("client").unwrap().stack.dhcp_discover(xid);

    lab.send_from_host("client", disc_frame);
    lab.run_until_quiescent(10);

    // Client should have received a DHCP Offer
    let client_host = lab.host_mut("client").unwrap();
    assert_eq!(client_host.stack.received_dhcp_offers.len(), 1);
    let offer = client_host.stack.received_dhcp_offers[0].clone();
    assert_eq!(offer.msg_type, DhcpMessageType::Offer);
    assert_eq!(offer.yiaddr, Ipv4Address::new(192, 168, 1, 100));

    // 4. Client sends DHCP Request
    let req_frame = client_host
        .stack
        .dhcp_request(offer.yiaddr, offer.server_id.unwrap(), xid);
    lab.send_from_host("client", req_frame);
    lab.run_until_quiescent(10);

    // Client should have received a DHCP ACK
    let client_host = lab.host_mut("client").unwrap();
    assert_eq!(client_host.stack.received_dhcp_acks.len(), 1);
    let ack = client_host.stack.received_dhcp_acks[0].clone();
    assert_eq!(ack.msg_type, DhcpMessageType::Ack);
    assert_eq!(ack.yiaddr, Ipv4Address::new(192, 168, 1, 100));

    // Apply lease configuration
    client_host.stack.apply_dhcp_ack(&ack);
    assert_eq!(
        client_host.stack.config.ip,
        Ipv4Address::new(192, 168, 1, 100)
    );
    assert_eq!(client_host.stack.config.subnet_mask, 24);
    assert_eq!(client_host.stack.config.gateway, Some(server_ip));

    // 5. Client now pings DHCP Server successfully
    let ping_frame = client_host
        .stack
        .ping4(server_ip, 0x9999, 1, b"DHCP_CLIENT_VERIFY")
        .expect("Ping frame");
    lab.send_from_host("client", ping_frame);
    lab.run_until_quiescent(10);

    assert_eq!(
        lab.host("client")
            .unwrap()
            .stack
            .received_icmp_replies
            .len(),
        1
    );
}

#[test]
fn test_scenario_2_router_nat_snat_and_dnat() {
    let mut lab = VirtualLab::new();

    let lan_client_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x10]);
    let lan_client_ip = Ipv4Address::new(192, 168, 10, 5);

    let router_lan_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x01]);
    let router_lan_ip = Ipv4Address::new(192, 168, 10, 1);

    let router_wan_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x01]);
    let router_wan_ip = Ipv4Address::new(203, 0, 113, 1);

    let wan_server_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x80]);
    let wan_server_ip = Ipv4Address::new(203, 0, 113, 80);

    // Private Host on LAN
    lab.add_host(
        "private_client",
        "lan_link",
        NetStackConfig {
            mac: lan_client_mac,
            ip: lan_client_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(router_lan_ip),
        },
    );

    // Public Server on WAN
    lab.add_host(
        "wan_server",
        "wan_link",
        NetStackConfig {
            mac: wan_server_mac,
            ip: wan_server_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(router_wan_ip),
        },
    );

    // Setup UDP echo server on WAN host (port 5353)
    lab.host_mut("wan_server").unwrap().stack.udp_sockets.bind(
        5353,
        |_src_ip, _src_port, payload| {
            let mut resp = b"PONG:".to_vec();
            resp.extend_from_slice(payload);
            Some(resp)
        },
    );

    // NAT Gateway Router
    let mut router = LabRouter::new("nat_gateway");
    router.add_interface("eth_lan", router_lan_mac, router_lan_ip, 24, "lan_link");
    router.add_interface("eth_wan", router_wan_mac, router_wan_ip, 24, "wan_link");
    router.enable_nat("eth_lan", "eth_wan", router_wan_ip);
    lab.add_router(router);

    // Private client sends UDP request to WAN server
    let query_frame = lab
        .host_mut("private_client")
        .unwrap()
        .stack
        .send_udp(wan_server_ip, 12345, 5353, b"SECURE_QUERY")
        .expect("Query frame");

    lab.send_from_host("private_client", query_frame);
    lab.run_until_quiescent(20);

    // Verify WAN server received packet from public IP (SNAT translated)
    let wan_host = lab.host("wan_server").unwrap();
    assert_eq!(wan_host.stack.received_udp_payloads.len(), 1);
    let (src_ip, _src_port, dst_port, payload) = &wan_host.stack.received_udp_payloads[0];
    assert_eq!(*src_ip, router_wan_ip); // SNAT rewritten!
    assert_eq!(*dst_port, 5353);
    assert_eq!(payload, b"SECURE_QUERY");

    // Verify Private client received server's response (DNAT de-translated)
    let client_host = lab.host("private_client").unwrap();
    assert_eq!(client_host.stack.received_udp_payloads.len(), 1);
    let (reply_src, reply_src_port, reply_dst_port, reply_payload) =
        &client_host.stack.received_udp_payloads[0];
    assert_eq!(*reply_src, wan_server_ip);
    assert_eq!(*reply_src_port, 5353);
    assert_eq!(*reply_dst_port, 12345);
    assert_eq!(reply_payload, b"PONG:SECURE_QUERY");
}

#[test]
fn test_scenario_3_ripv2_multi_router_dynamic_convergence() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x02]);
    let host_a_ip = Ipv4Address::new(10, 0, 1, 2);

    let r1_lan_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0A, 0x01]);
    let r1_lan_ip = Ipv4Address::new(10, 0, 1, 1);
    let r1_transit_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x11, 0x01]);
    let r1_transit_ip = Ipv4Address::new(172, 16, 0, 1);

    let r2_transit_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x11, 0x02]);
    let r2_transit_ip = Ipv4Address::new(172, 16, 0, 2);
    let r2_lan_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x01]);
    let r2_lan_ip = Ipv4Address::new(10, 0, 2, 1);

    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x0B, 0x02]);
    let host_b_ip = Ipv4Address::new(10, 0, 2, 2);

    // Host A (Subnet 10.0.1.0/24)
    lab.add_host(
        "host_a",
        "link_a",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(r1_lan_ip),
        },
    );

    // Host B (Subnet 10.0.2.0/24)
    lab.add_host(
        "host_b",
        "link_b",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(r2_lan_ip),
        },
    );

    // Router 1
    let mut r1 = LabRouter::new("router_1");
    r1.add_interface("r1_lan", r1_lan_mac, r1_lan_ip, 24, "link_a");
    r1.add_interface("r1_wan", r1_transit_mac, r1_transit_ip, 24, "link_transit");
    r1.enable_rip();
    lab.add_router(r1);

    // Router 2
    let mut r2 = LabRouter::new("router_2");
    r2.add_interface("r2_wan", r2_transit_mac, r2_transit_ip, 24, "link_transit");
    r2.add_interface("r2_lan", r2_lan_mac, r2_lan_ip, 24, "link_b");
    r2.enable_rip();
    lab.add_router(r2);

    // Before RIP exchange: Router 1 has NO route to 10.0.2.0/24
    assert!(
        lab.router("router_1")
            .unwrap()
            .routing_table
            .lookup(host_b_ip)
            .is_none()
    );

    // Trigger RIPv2 routing advertisement updates across all links
    lab.broadcast_rip_advertisements();
    lab.run_until_quiescent(10);

    // After RIP exchange: Router 1 dynamically learned route to 10.0.2.0/24 via 172.16.0.2!
    let r1_route_b = lab
        .router("router_1")
        .unwrap()
        .routing_table
        .lookup(host_b_ip);
    assert!(r1_route_b.is_some());
    assert_eq!(r1_route_b.unwrap().next_hop(host_b_ip), r2_transit_ip);

    // Router 2 dynamically learned route to 10.0.1.0/24 via 172.16.0.1!
    let r2_route_a = lab
        .router("router_2")
        .unwrap()
        .routing_table
        .lookup(host_a_ip);
    assert!(r2_route_a.is_some());
    assert_eq!(r2_route_a.unwrap().next_hop(host_a_ip), r1_transit_ip);

    // End-to-end ping across dynamically converged multi-hop network
    let ping_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0xABCD, 1, b"DYNAMIC_RIP_PING")
        .expect("Ping frame");

    lab.send_from_host("host_a", ping_frame);
    lab.run_until_quiescent(25);

    // Host A should receive ICMP Echo Reply from Host B!
    let host_a = lab.host("host_a").unwrap();
    assert_eq!(host_a.stack.received_icmp_replies.len(), 1);
    assert_eq!(host_a.stack.received_icmp_replies[0].0, host_b_ip);
}

#[test]
fn test_scenario_4_tcp_out_of_order_stream_reassembly() {
    let local = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 1),
        port: 80,
    };
    let remote = SocketAddrV4 {
        ip: Ipv4Address::new(10, 0, 0, 2),
        port: 50000,
    };

    let mut conn = TcpConnection::new_server(local, remote, 1000);
    conn.state = toy_tcpip::tcp::TcpState::Established;
    conn.rcv_nxt = 5000;

    // Simulate receiving 3 segments out-of-order:
    // Seg 3: Seq 5022..5033 (11 bytes: "CHUNK_THREE")
    // Seg 2: Seq 5011..5022 (11 bytes: "CHUNK_TWO__")
    // Seg 1: Seq 5000..5011 (11 bytes: "CHUNK_ONE__")

    let seg3 = toy_tcpip::tcp::TcpSegment {
        src_port: 50000,
        dst_port: 80,
        seq_num: 5022,
        ack_num: 1000,
        data_offset: 5,
        flags: toy_tcpip::tcp::TcpFlags::ack(),
        window_size: 65535,
        checksum: 0,
        urgent_ptr: 0,
        options: Vec::new(),
        payload: b"CHUNK_THREE",
    };

    let seg2 = toy_tcpip::tcp::TcpSegment {
        src_port: 50000,
        dst_port: 80,
        seq_num: 5011,
        ack_num: 1000,
        data_offset: 5,
        flags: toy_tcpip::tcp::TcpFlags::ack(),
        window_size: 65535,
        checksum: 0,
        urgent_ptr: 0,
        options: Vec::new(),
        payload: b"CHUNK_TWO__",
    };

    let seg1 = toy_tcpip::tcp::TcpSegment {
        src_port: 50000,
        dst_port: 80,
        seq_num: 5000,
        ack_num: 1000,
        data_offset: 5,
        flags: toy_tcpip::tcp::TcpFlags::ack(),
        window_size: 65535,
        checksum: 0,
        urgent_ptr: 0,
        options: Vec::new(),
        payload: b"CHUNK_ONE__",
    };

    // Process Seg 3 -> Buffered into OOO queue
    conn.handle_segment(&seg3);
    assert_eq!(conn.rcv_nxt, 5000);
    assert_eq!(conn.ooo_queue.len(), 1);
    assert!(conn.rx_buffer.is_empty());

    // Process Seg 2 -> Buffered into OOO queue
    conn.handle_segment(&seg2);
    assert_eq!(conn.rcv_nxt, 5000);
    assert_eq!(conn.ooo_queue.len(), 2);
    assert!(conn.rx_buffer.is_empty());

    // Process Seg 1 (In-order arrival) -> Triggers cascading reassembly of Seg 1, 2, and 3!
    conn.handle_segment(&seg1);
    assert_eq!(conn.rcv_nxt, 5033);
    assert_eq!(conn.ooo_queue.len(), 0);
    assert_eq!(conn.rx_buffer, b"CHUNK_ONE__CHUNK_TWO__CHUNK_THREE");
}
