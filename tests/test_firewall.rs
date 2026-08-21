use toy_tcpip::firewall::{Firewall, FirewallAction, FirewallChain, FirewallRule, IpCidr};
use toy_tcpip::ipv4::{Ipv4Address, Ipv4Packet, IP_PROTO_TCP};
use toy_tcpip::tcp::{TcpFlags, TcpSegment};

#[test]
fn test_firewall_port_and_cidr_filtering() {
    let mut fw = Firewall::new();

    // Rule 1: Drop all traffic from 198.51.100.0/24
    fw.add_rule(
        FirewallChain::Input,
        FirewallRule {
            description: "Block untrusted subnet".to_string(),
            src_cidr: Some(IpCidr::new(Ipv4Address::new(198, 51, 100, 0), 24)),
            action: FirewallAction::Drop,
            ..Default::default()
        },
    );

    // Rule 2: Allow TCP to port 443 (HTTPS)
    fw.add_rule(
        FirewallChain::Input,
        FirewallRule {
            description: "Allow HTTPS".to_string(),
            protocol: Some(IP_PROTO_TCP),
            dst_port_range: Some((443, 443)),
            action: FirewallAction::Accept,
            ..Default::default()
        },
    );

    // Set default input policy to Drop
    fw.default_input_policy = FirewallAction::Drop;

    let target_ip = Ipv4Address::new(10, 0, 0, 1);

    // 1. Untrusted subnet -> DROP
    let tcp_seg1 = TcpSegment::serialize(
        Ipv4Address::new(198, 51, 100, 42),
        target_ip,
        12345,
        443,
        1,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let ip1 = Ipv4Packet::serialize(
        Ipv4Address::new(198, 51, 100, 42),
        target_ip,
        IP_PROTO_TCP,
        1,
        64,
        &tcp_seg1,
    );
    let pkt1 = Ipv4Packet::parse(&ip1, false).unwrap();
    assert_eq!(fw.evaluate(FirewallChain::Input, &pkt1), FirewallAction::Drop);

    // 2. Trusted subnet to port 443 -> ACCEPT
    let tcp_seg2 = TcpSegment::serialize(
        Ipv4Address::new(172, 16, 0, 5),
        target_ip,
        12345,
        443,
        1,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let ip2 = Ipv4Packet::serialize(
        Ipv4Address::new(172, 16, 0, 5),
        target_ip,
        IP_PROTO_TCP,
        2,
        64,
        &tcp_seg2,
    );
    let pkt2 = Ipv4Packet::parse(&ip2, false).unwrap();
    assert_eq!(fw.evaluate(FirewallChain::Input, &pkt2), FirewallAction::Accept);

    // 3. Trusted subnet to unallowed port 22 (SSH) -> Default DROP
    let tcp_seg3 = TcpSegment::serialize(
        Ipv4Address::new(172, 16, 0, 5),
        target_ip,
        12345,
        22,
        1,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let ip3 = Ipv4Packet::serialize(
        Ipv4Address::new(172, 16, 0, 5),
        target_ip,
        IP_PROTO_TCP,
        3,
        64,
        &tcp_seg3,
    );
    let pkt3 = Ipv4Packet::parse(&ip3, false).unwrap();
    assert_eq!(fw.evaluate(FirewallChain::Input, &pkt3), FirewallAction::Drop);
}
