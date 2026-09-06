use std::fs::{self, File};
use std::path::Path;
use toy_tcpip::arp::ArpPacket;
use toy_tcpip::ethernet::{ETHERTYPE_ARP, ETHERTYPE_IPV4, EthernetFrame, MacAddress};
use toy_tcpip::icmp::IcmpPacket;
use toy_tcpip::ipv4::{IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP, Ipv4Address, Ipv4Packet};
use toy_tcpip::pcap::{LINKTYPE_ETHERNET, PcapWriter};
use toy_tcpip::tcp::{TcpFlags, TcpSegment};
use toy_tcpip::udp::UdpDatagram;

fn main() {
    let out_dir = Path::new("samples");
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).expect("Failed to create samples directory");
    }

    let pcap_path = out_dir.join("sample.pcap");
    let file = File::create(&pcap_path).expect("Failed to create sample.pcap");
    let mut writer = PcapWriter::new(file, 65535, LINKTYPE_ETHERNET).expect("PcapWriter init");

    let client_mac = MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    let server_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x10]);
    let client_ip = Ipv4Address::new(192, 168, 1, 100);
    let server_ip = Ipv4Address::new(192, 168, 1, 10);

    let ts_sec = 1700000000;
    let mut ts_usec = 100000;

    // 1. ARP Request
    let arp_req = ArpPacket::build_request(client_mac, client_ip.0, server_ip.0);
    let f1 = EthernetFrame::serialize(
        MacAddress::BROADCAST,
        client_mac,
        ETHERTYPE_ARP,
        &arp_req.serialize(),
    );
    writer.write_packet(ts_sec, ts_usec, &f1).unwrap();
    ts_usec += 1000;

    // 2. ICMP Echo Request
    let icmp_req = IcmpPacket::build_echo_request(0x1234, 1, b"PING sample test payload 12345678");
    let ip_icmp = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_ICMP, 1001, 64, &icmp_req);
    let f2 = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_icmp);
    writer.write_packet(ts_sec, ts_usec, &f2).unwrap();
    ts_usec += 2000;

    // 3. UDP Datagram to Port 7 (Echo)
    let udp = UdpDatagram::serialize(client_ip, server_ip, 54321, 7, b"Hello UDP Echo Server");
    let ip_udp = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_UDP, 1002, 64, &udp);
    let f3 = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_udp);
    writer.write_packet(ts_sec, ts_usec, &f3).unwrap();
    ts_usec += 3000;

    // 4. TCP SYN to Port 80 (HTTP)
    let tcp_syn = TcpSegment::serialize(
        client_ip,
        server_ip,
        50000,
        80,
        1000,
        0,
        TcpFlags::syn(),
        65535,
        &[],
    );
    let ip_syn = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_TCP, 1003, 64, &tcp_syn);
    let f4 = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_syn);
    writer.write_packet(ts_sec, ts_usec, &f4).unwrap();
    ts_usec += 4000;

    // 5. TCP ACK (completing handshake simulation)
    let tcp_ack = TcpSegment::serialize(
        client_ip,
        server_ip,
        50000,
        80,
        1001,
        1001,
        TcpFlags::ack(),
        65535,
        &[],
    );
    let ip_ack = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_TCP, 1004, 64, &tcp_ack);
    let f5 = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_ack);
    writer.write_packet(ts_sec, ts_usec, &f5).unwrap();
    ts_usec += 5000;

    // 6. TCP Data (HTTP GET)
    let http_payload = b"GET / HTTP/1.1\r\nHost: 192.168.1.10\r\nUser-Agent: ToyTCP\r\n\r\n";
    let tcp_data = TcpSegment::serialize(
        client_ip,
        server_ip,
        50000,
        80,
        1001,
        1001,
        TcpFlags {
            psh: true,
            ack: true,
            ..Default::default()
        },
        65535,
        http_payload,
    );
    let ip_data = Ipv4Packet::serialize(client_ip, server_ip, IP_PROTO_TCP, 1005, 64, &tcp_data);
    let f6 = EthernetFrame::serialize(server_mac, client_mac, ETHERTYPE_IPV4, &ip_data);
    writer.write_packet(ts_sec, ts_usec, &f6).unwrap();

    println!(
        "✅ Generated '{}' with 6 sample multi-protocol frames.",
        pcap_path.display()
    );
}
