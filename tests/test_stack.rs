use std::io::Cursor;
use toy_tcpip::arp::ArpPacket;
use toy_tcpip::ethernet::{EthernetFrame, MacAddress, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use toy_tcpip::icmp::{IcmpPacket, IcmpType};
use toy_tcpip::ipv4::{Ipv4Address, Ipv4Packet, IP_PROTO_ICMP, IP_PROTO_UDP};
use toy_tcpip::pcap::{PcapReader, PcapWriter, LINKTYPE_ETHERNET};
use toy_tcpip::stack::{NetStack, NetStackConfig};
use toy_tcpip::udp::UdpDatagram;

#[test]
fn test_end_to_end_pcap_pipeline() {
    let stack_mac = MacAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let stack_ip = Ipv4Address::new(192, 168, 1, 10);
    let client_mac = MacAddress([0x52, 0x54, 0x00, 0xaa, 0xbb, 0xcc]);
    let client_ip = Ipv4Address::new(192, 168, 1, 50);

    // 1. Synthesize input PCAP with ARP and ICMP frames
    let mut in_pcap_buf = Vec::new();
    {
        let mut writer = PcapWriter::new(&mut in_pcap_buf, 65535, LINKTYPE_ETHERNET).unwrap();

        // Frame 1: ARP Request
        let arp = ArpPacket::build_request(client_mac, client_ip.0, stack_ip.0);
        let f1 = EthernetFrame::serialize(MacAddress::BROADCAST, client_mac, ETHERTYPE_ARP, &arp.serialize());
        writer.write_packet(1, 0, &f1).unwrap();

        // Frame 2: ICMP Ping
        let icmp = IcmpPacket::build_echo_request(0x4242, 1, b"Ping through PCAP");
        let ip = Ipv4Packet::serialize(client_ip, stack_ip, IP_PROTO_ICMP, 1, 64, &icmp);
        let f2 = EthernetFrame::serialize(stack_mac, client_mac, ETHERTYPE_IPV4, &ip);
        writer.write_packet(1, 50, &f2).unwrap();

        // Frame 3: UDP query to port 7
        let udp = UdpDatagram::serialize(client_ip, stack_ip, 40000, 7, b"UDP test");
        let ip_udp = Ipv4Packet::serialize(client_ip, stack_ip, IP_PROTO_UDP, 2, 64, &udp);
        let f3 = EthernetFrame::serialize(stack_mac, client_mac, ETHERTYPE_IPV4, &ip_udp);
        writer.write_packet(1, 100, &f3).unwrap();
    }

    // 2. Feed into NetStack
    let mut stack = NetStack::new(NetStackConfig {
        mac: stack_mac,
        ip: stack_ip,
        ipv6: None,
        subnet_mask: 24,
        gateway: Some(Ipv4Address::new(192, 168, 1, 1)),
    });
    stack.udp_sockets.bind(7, |_src_ip, _src_port, data| Some(data.to_vec()));

    let mut out_pcap_buf = Vec::new();
    let mut out_writer = PcapWriter::new(&mut out_pcap_buf, 65535, LINKTYPE_ETHERNET).unwrap();

    let mut reader = PcapReader::new(Cursor::new(&in_pcap_buf)).unwrap();
    let mut total_responses = 0;

    while let Ok(Some(pkt)) = reader.next_packet() {
        let responses = stack.process_frame(&pkt.data);
        for resp in responses {
            total_responses += 1;
            out_writer.write_packet(pkt.ts_sec, pkt.ts_usec + 1, &resp).unwrap();
        }
    }

    assert_eq!(total_responses, 3);

    // 3. Validate generated output PCAP
    let mut out_reader = PcapReader::new(Cursor::new(&out_pcap_buf)).unwrap();
    let r1 = out_reader.next_packet().unwrap().unwrap();
    let r1_eth = EthernetFrame::parse(&r1.data).unwrap();
    assert_eq!(r1_eth.ethertype, toy_tcpip::ethernet::EtherType::Arp);

    let r2 = out_reader.next_packet().unwrap().unwrap();
    let r2_eth = EthernetFrame::parse(&r2.data).unwrap();
    let r2_ip = Ipv4Packet::parse(r2_eth.payload, true).unwrap();
    let r2_icmp = IcmpPacket::parse(r2_ip.payload, true).unwrap();
    assert_eq!(r2_icmp.icmp_type, IcmpType::EchoReply);

    let r3 = out_reader.next_packet().unwrap().unwrap();
    let r3_eth = EthernetFrame::parse(&r3.data).unwrap();
    let r3_ip = Ipv4Packet::parse(r3_eth.payload, true).unwrap();
    let r3_udp = UdpDatagram::parse(r3_ip.header.src_ip, r3_ip.header.dst_ip, r3_ip.payload, true).unwrap();
    assert_eq!(r3_udp.payload, b"UDP test");
}
