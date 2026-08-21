use toy_tcpip::ipv4::{Ipv4Address, Ipv4Packet, IP_PROTO_UDP};
use toy_tcpip::nat::NatTable;
use toy_tcpip::udp::UdpDatagram;

#[test]
fn test_nat_udp_dns_translation_and_reverse() {
    let public_ip = Ipv4Address::new(203, 0, 113, 1);
    let mut nat = NatTable::new(public_ip);

    let client_ip = Ipv4Address::new(192, 168, 1, 50);
    let dns_server_ip = Ipv4Address::new(8, 8, 8, 8);

    // Client sends UDP query to 8.8.8.8:53 from 192.168.1.50:53535
    let udp = UdpDatagram::serialize(client_ip, dns_server_ip, 53535, 53, b"DNS Query");
    let mut ip = Ipv4Packet::serialize(client_ip, dns_server_ip, IP_PROTO_UDP, 100, 64, &udp);

    // SNAT
    let out_ok = nat.translate_outbound(&mut ip);
    assert!(out_ok);

    let parsed_out = Ipv4Packet::parse(&ip, true).unwrap();
    assert_eq!(parsed_out.header.src_ip, public_ip);

    let parsed_udp_out = UdpDatagram::parse(public_ip, dns_server_ip, parsed_out.payload, true).unwrap();
    let nat_port = parsed_udp_out.src_port;
    assert_eq!(nat_port, 40000);

    // Inbound reply from 8.8.8.8:53 to 203.0.113.1:40000
    let reply_udp = UdpDatagram::serialize(dns_server_ip, public_ip, 53, nat_port, b"DNS Response");
    let mut reply_ip = Ipv4Packet::serialize(dns_server_ip, public_ip, IP_PROTO_UDP, 200, 64, &reply_udp);

    // Reverse SNAT
    let in_ok = nat.translate_inbound(&mut reply_ip);
    assert!(in_ok);

    let parsed_in = Ipv4Packet::parse(&reply_ip, true).unwrap();
    assert_eq!(parsed_in.header.dst_ip, client_ip);

    let parsed_udp_in = UdpDatagram::parse(dns_server_ip, client_ip, parsed_in.payload, true).unwrap();
    assert_eq!(parsed_udp_in.dst_port, 53535); // Restored!
}
