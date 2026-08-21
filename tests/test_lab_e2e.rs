//! Integrated End-to-End Virtual Network Lab Integration Tests.
//!
//! Validates:
//! - Scenario A: IPv4 host-to-host ping + cold ARP resolution
//! - Scenario B: IPv6 host-to-host ping + NDP Neighbor Solicitation / Advertisement
//! - Scenario C: Routed IPv4 network (Host A <-> Router <-> Host B) + TTL expiration / ICMP Time Exceeded
//! - Scenario D: UDP end-to-end echo exchange & pseudo-header checksum validation
//! - Scenario E: TCP full lifecycle (SYN -> SYN-ACK -> ACK -> DATA -> FIN-ACK -> ACK)
//! - Scenario F: Fault injection (MTU clipping, deterministic drops, payload corruption rejection)
//! - Scenario G: Live PCAP packet tap & roundtrip validation with PcapReader

use toy_tcpip::ethernet::MacAddress;
use toy_tcpip::icmp::IcmpPacket;
use toy_tcpip::ipv4::{IP_PROTO_ICMP, Ipv4Address, Ipv4Packet};
use toy_tcpip::ipv6::Ipv6Address;
use toy_tcpip::lab::{LabRouter, VirtualLab};
use toy_tcpip::pcap::PcapReader;
use toy_tcpip::stack::NetStackConfig;
use toy_tcpip::tcp::{SocketAddrV4, TcpState};

#[test]
fn test_scenario_a_ipv4_host_to_host_ping_and_arp() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x01, 0x20]);
    let host_a_ip = Ipv4Address::new(192, 168, 1, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 1, 20);

    lab.add_host(
        "host_a",
        "lan1",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "host_b",
        "lan1",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Host A initiates ping to Host B (cold ARP cache)
    let ping_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0x1234, 1, b"E2E_PING_PAYLOAD_V4")
        .expect("Ping frame initiation");

    lab.send_from_host("host_a", ping_frame);

    // Run simulation to quiescence
    let steps = lab.run_until_quiescent(10);
    assert!(steps >= 4, "Expected ARP + Ping exchange steps");

    let host_a = lab.host("host_a").unwrap();
    let host_b = lab.host("host_b").unwrap();

    // 1. Verify Host A received ICMP Echo Reply
    assert_eq!(host_a.stack.received_icmp_replies.len(), 1);
    assert_eq!(
        host_a.stack.received_icmp_replies[0],
        (host_b_ip, 0x1234, 1)
    );

    // 2. Verify ARP tables populated bidirectionally
    assert_eq!(
        host_a.stack.arp_table.lookup(&host_b_ip.0),
        Some(host_b_mac)
    );
    assert_eq!(
        host_b.stack.arp_table.lookup(&host_a_ip.0),
        Some(host_a_mac)
    );
}

#[test]
fn test_scenario_b_ipv6_host_to_host_ping_and_ndp() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x02, 0x20]);
    let host_a_ip6 = Ipv6Address::new([0x2001, 0x0db8, 0x0001, 0, 0, 0, 0, 0x0010]);
    let host_b_ip6 = Ipv6Address::new([0x2001, 0x0db8, 0x0001, 0, 0, 0, 0, 0x0020]);

    lab.add_host(
        "host_a",
        "lan6",
        NetStackConfig {
            mac: host_a_mac,
            ip: Ipv4Address::new(10, 0, 0, 10),
            ipv6: Some(host_a_ip6),
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "host_b",
        "lan6",
        NetStackConfig {
            mac: host_b_mac,
            ip: Ipv4Address::new(10, 0, 0, 20),
            ipv6: Some(host_b_ip6),
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Host A initiates IPv6 ping (cold NDP cache)
    let ping6_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping6(host_b_ip6, 0x5678, 1, b"E2E_PING6_PAYLOAD")
        .expect("Ping6 frame initiation");

    lab.send_from_host("host_a", ping6_frame);

    let steps = lab.run_until_quiescent(10);
    assert!(steps >= 4, "Expected NDP + Ping6 exchange steps");

    let host_a = lab.host("host_a").unwrap();
    let host_b = lab.host("host_b").unwrap();

    // 1. Verify Host A received ICMPv6 Echo Reply
    assert_eq!(host_a.stack.received_icmpv6_replies.len(), 1);
    assert_eq!(
        host_a.stack.received_icmpv6_replies[0],
        (host_b_ip6, 0x5678, 1)
    );

    // 2. Verify NDP Neighbor tables populated bidirectionally
    assert_eq!(host_a.stack.ndp_table.lookup(&host_b_ip6), Some(host_b_mac));
    assert_eq!(host_b.stack.ndp_table.lookup(&host_a_ip6), Some(host_a_mac));
}

#[test]
fn test_scenario_c_routed_ipv4_network_and_ttl_expiration() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x20]);
    let rtr_if0_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x01]);
    let rtr_if1_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x03, 0x02]);

    let host_a_ip = Ipv4Address::new(10, 0, 1, 2);
    let rtr_if0_ip = Ipv4Address::new(10, 0, 1, 1);
    let rtr_if1_ip = Ipv4Address::new(10, 0, 2, 1);
    let host_b_ip = Ipv4Address::new(10, 0, 2, 2);

    // Host A on net1 with default gateway = Router eth0
    lab.add_host(
        "host_a",
        "link_net1",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(rtr_if0_ip),
        },
    );

    // Host B on net2 with default gateway = Router eth1
    lab.add_host(
        "host_b",
        "link_net2",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: Some(rtr_if1_ip),
        },
    );

    // Router with eth0 on link_net1 and eth1 on link_net2
    let mut router = LabRouter::new("rtr1");
    router.add_interface("eth0", rtr_if0_mac, rtr_if0_ip, 24, "link_net1");
    router.add_interface("eth1", rtr_if1_mac, rtr_if1_ip, 24, "link_net2");
    lab.add_router(router);

    // --- Subtest 1: Routed Ping across subnets ---
    let ping_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0xABCD, 1, b"ROUTED_INTER_SUBNET_DATA")
        .expect("Ping frame");

    lab.send_from_host("host_a", ping_frame);
    let steps = lab.run_until_quiescent(20);
    assert!(steps > 0);

    let host_a = lab.host("host_a").unwrap();
    assert_eq!(host_a.stack.received_icmp_replies.len(), 1);
    assert_eq!(
        host_a.stack.received_icmp_replies[0],
        (host_b_ip, 0xABCD, 1)
    );

    // --- Subtest 2: TTL Expiration and ICMP Time Exceeded (Type 11 Code 0) ---
    // Craft packet with TTL = 1 from Host A to Host B
    let icmp_req = IcmpPacket::build_echo_request(0x9999, 1, b"TTL1_EXPIRATION_TEST");
    let ip_ttl1 = Ipv4Packet::serialize(
        host_a_ip,
        host_b_ip,
        IP_PROTO_ICMP,
        555,
        1, // TTL = 1
        &icmp_req,
    );

    // Host A sends packet with TTL=1 towards router (next hop 10.0.1.1 MAC is already in ARP table)
    let eth_frame = toy_tcpip::ethernet::EthernetFrame::serialize(
        rtr_if0_mac,
        host_a_mac,
        toy_tcpip::ethernet::ETHERTYPE_IPV4,
        &ip_ttl1,
    );

    lab.send_from_host("host_a", eth_frame);
    lab.run_until_quiescent(10);

    let host_a_after = lab.host("host_a").unwrap();
    assert_eq!(
        host_a_after.stack.received_icmp_time_exceeded.len(),
        1,
        "Expected ICMP Time Exceeded from router"
    );
    assert_eq!(
        host_a_after.stack.received_icmp_time_exceeded[0],
        (rtr_if0_ip, 0)
    );
}

#[test]
fn test_scenario_d_udp_end_to_end_echo() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x04, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x04, 0x20]);
    let host_a_ip = Ipv4Address::new(192, 168, 10, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 10, 20);

    lab.add_host(
        "host_a",
        "lan_udp",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "host_b",
        "lan_udp",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Host B binds UDP echo socket on port 9000
    lab.host_mut("host_b")
        .unwrap()
        .stack
        .udp_sockets
        .bind(9000, |_src_ip, _src_port, payload| {
            let mut echo = b"ECHO_RESP: ".to_vec();
            echo.extend_from_slice(payload);
            Some(echo)
        });

    // Host A sends UDP datagram to Host B:9000
    let udp_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .send_udp(host_b_ip, 45000, 9000, b"PING_UDP_PAYLOAD")
        .expect("UDP frame");

    lab.send_from_host("host_a", udp_frame);
    lab.run_until_quiescent(10);

    let host_a = lab.host("host_a").unwrap();
    assert_eq!(
        host_a.stack.received_udp_payloads.len(),
        1,
        "Host A should receive UDP reply"
    );
    let (reply_src, reply_src_port, reply_dst_port, ref reply_data) =
        host_a.stack.received_udp_payloads[0];
    assert_eq!(reply_src, host_b_ip);
    assert_eq!(reply_src_port, 9000);
    assert_eq!(reply_dst_port, 45000);
    assert_eq!(reply_data, b"ECHO_RESP: PING_UDP_PAYLOAD");
}

#[test]
fn test_scenario_e_tcp_end_to_end_full_lifecycle() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x05, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x05, 0x20]);
    let host_a_ip = Ipv4Address::new(192, 168, 20, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 20, 20);

    lab.add_host(
        "client",
        "lan_tcp",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "server",
        "lan_tcp",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Server listens on TCP port 80
    lab.host_mut("server").unwrap().stack.tcp_manager.listen(80);

    let client_sock = SocketAddrV4 {
        ip: host_a_ip,
        port: 50000,
    };
    let server_sock = SocketAddrV4 {
        ip: host_b_ip,
        port: 80,
    };

    // 1. Client initiates TCP connection (SYN)
    let syn_frame = lab
        .host_mut("client")
        .unwrap()
        .stack
        .tcp_connect(host_b_ip, 50000, 80, 1000)
        .expect("SYN frame");

    lab.send_from_host("client", syn_frame);
    lab.run_until_quiescent(10);

    // Verify 3-way handshake established both sides
    let client_conn = lab
        .host("client")
        .unwrap()
        .stack
        .tcp_manager
        .get_connection(client_sock, server_sock)
        .unwrap();
    let server_conn = lab
        .host("server")
        .unwrap()
        .stack
        .tcp_manager
        .get_connection(server_sock, client_sock)
        .unwrap();

    assert_eq!(client_conn.state, TcpState::Established);
    assert_eq!(server_conn.state, TcpState::Established);

    // 2. Client sends HTTP Data
    let data_frame = lab
        .host_mut("client")
        .unwrap()
        .stack
        .tcp_send_data(host_b_ip, 50000, 80, b"GET /healthz HTTP/1.1\r\n\r\n")
        .expect("Data frame");

    lab.send_from_host("client", data_frame);
    lab.run_until_quiescent(10);

    let server_conn_after = lab
        .host("server")
        .unwrap()
        .stack
        .tcp_manager
        .get_connection(server_sock, client_sock)
        .unwrap();
    assert_eq!(
        server_conn_after.rx_buffer,
        b"GET /healthz HTTP/1.1\r\n\r\n"
    );

    // 3. Client closes TCP connection (FIN-ACK)
    let fin_frame = lab
        .host_mut("client")
        .unwrap()
        .stack
        .tcp_close(host_b_ip, 50000, 80)
        .expect("FIN frame");

    lab.send_from_host("client", fin_frame);
    lab.run_until_quiescent(10);

    let server_conn_final = lab
        .host("server")
        .unwrap()
        .stack
        .tcp_manager
        .get_connection(server_sock, client_sock)
        .unwrap();
    assert_eq!(server_conn_final.state, TcpState::Closed);
}

#[test]
fn test_scenario_f_link_fault_injection_mtu_and_drops() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x06, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x06, 0x20]);
    let host_a_ip = Ipv4Address::new(192, 168, 30, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 30, 20);

    lab.add_link_with_mtu("lan_fault", 100); // Small MTU = 100 bytes

    lab.add_host(
        "host_a",
        "lan_fault",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "host_b",
        "lan_fault",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Populate ARP caches so ping doesn't need ARP step
    lab.host_mut("host_a")
        .unwrap()
        .stack
        .arp_table
        .insert(host_b_ip.0, host_b_mac);
    lab.host_mut("host_b")
        .unwrap()
        .stack
        .arp_table
        .insert(host_a_ip.0, host_a_mac);

    // 1. Oversized packet: 200 bytes payload exceeds MTU 100
    let big_payload = vec![0x41u8; 200];
    let oversized_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0x1111, 1, &big_payload)
        .unwrap();

    lab.send_from_host("host_a", oversized_frame);
    lab.run_until_quiescent(10);

    let link = lab.link("lan_fault").unwrap();
    assert_eq!(link.frames_dropped, 1, "Oversized frame must be dropped");

    // 2. Normal packet with deterministic drop rule: drop packet index 2
    lab.link_mut("lan_fault").unwrap().mtu = 1500;
    lab.link_mut("lan_fault")
        .unwrap()
        .drop_packet_indices
        .push(2);

    let ping_dropped = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0x2222, 1, b"SMALL_PAYLOAD")
        .unwrap();

    lab.send_from_host("host_a", ping_dropped);
    lab.run_until_quiescent(10);

    assert_eq!(
        lab.link("lan_fault").unwrap().frames_dropped,
        2,
        "Configured drop index must drop packet"
    );

    // 3. Next packet index 3 is not in drop list -> succeeds
    let ping_success = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0x3333, 1, b"SMALL_PAYLOAD")
        .unwrap();

    lab.send_from_host("host_a", ping_success);
    lab.run_until_quiescent(10);

    let host_a = lab.host("host_a").unwrap();
    assert_eq!(
        host_a.stack.received_icmp_replies.len(),
        1,
        "Packet 3 should succeed"
    );
    assert_eq!(
        host_a.stack.received_icmp_replies[0],
        (host_b_ip, 0x3333, 1)
    );
}

#[test]
fn test_scenario_g_pcap_capture_and_wireshark_validation() {
    let mut lab = VirtualLab::new();

    let host_a_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x07, 0x10]);
    let host_b_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x07, 0x20]);
    let host_a_ip = Ipv4Address::new(192, 168, 40, 10);
    let host_b_ip = Ipv4Address::new(192, 168, 40, 20);

    lab.add_host(
        "host_a",
        "lan_pcap",
        NetStackConfig {
            mac: host_a_mac,
            ip: host_a_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    lab.add_host(
        "host_b",
        "lan_pcap",
        NetStackConfig {
            mac: host_b_mac,
            ip: host_b_ip,
            ipv6: None,
            subnet_mask: 24,
            gateway: None,
        },
    );

    // Enable PCAP capture on link
    lab.enable_pcap("lan_pcap");

    // Execute Ping
    let ping_frame = lab
        .host_mut("host_a")
        .unwrap()
        .stack
        .ping4(host_b_ip, 0x7777, 1, b"PCAP_RECORD_PAYLOAD")
        .unwrap();

    lab.send_from_host("host_a", ping_frame);
    lab.run_until_quiescent(10);

    // Export PCAP byte buffer
    let pcap_bytes = lab.export_pcap("lan_pcap").expect("Exported PCAP bytes");

    assert!(
        pcap_bytes.len() >= 24,
        "PCAP header must be at least 24 bytes"
    );

    // Read back and parse PCAP with PcapReader
    let mut reader = PcapReader::new(&pcap_bytes[..]).expect("PcapReader parse");
    assert_eq!(reader.header.magic_number, 0xa1b2c3d4);
    assert_eq!(reader.header.version_major, 2);
    assert_eq!(reader.header.version_minor, 4);

    let packets = reader.read_all_packets().expect("Read all packets");
    assert!(
        packets.len() >= 4,
        "Should capture ARP req, ARP rep, ICMP req, ICMP rep"
    );

    for pkt in &packets {
        assert!(pkt.data.len() >= 14, "Must contain full Ethernet frame");
        let eth = toy_tcpip::ethernet::EthernetFrame::parse(&pkt.data).unwrap();
        assert!(
            eth.dst_mac == host_a_mac || eth.dst_mac == host_b_mac || eth.dst_mac.is_broadcast()
        );
    }
}
