use std::str::FromStr;
use toy_tcpip::gtp::{GtpPacket, GTP_U_UDP_PORT};
use toy_tcpip::ipv4::{IpProtocol, Ipv4Address, Ipv4Packet};
use toy_tcpip::ipv6::{Ipv6Address, Ipv6Packet};
use toy_tcpip::srv6_mup::{Srv6MupEngine, Srv6MupSession};
use toy_tcpip::udp::UdpDatagram;

#[test]
fn test_srv6_mup_5g_uplink_and_downlink_translation() {
    let mut engine = Srv6MupEngine::new();

    let gnb_ipv4 = Ipv4Address::new(172, 20, 1, 100);
    let upf_ipv4 = Ipv4Address::new(10, 100, 0, 1);
    let teid = 0x11223344;
    let srv6_sid = Ipv6Address::from_str("2001:db8:50:1::1").unwrap();
    let edge_router_v6 = Ipv6Address::from_str("2001:db8:ed:1::1").unwrap();

    engine.register_session(Srv6MupSession {
        gnb_ipv4,
        upf_ipv4,
        teid,
        srv6_sid,
        qfi: 5,
    });

    let pdu_payload = b"5G PDU Session HTTP/3 Request Body";

    // 1. Ingress Uplink GTP-U Packet from gNodeB -> Translated to SRv6
    let srv6_packet = engine.process_uplink_gtp_to_srv6(
        gnb_ipv4,
        teid,
        pdu_payload,
        edge_router_v6,
    ).unwrap();

    let parsed_v6 = Ipv6Packet::parse(&srv6_packet).unwrap();
    assert_eq!(parsed_v6.header.src_ip, edge_router_v6);
    assert_eq!(parsed_v6.header.dst_ip, srv6_sid);

    // 2. Ingress Downlink SRv6 Packet -> Translated back to GTP-U/UDP/IPv4
    let gtp_ip_packet = engine.process_downlink_srv6_to_gtp(
        srv6_sid,
        pdu_payload,
        upf_ipv4,
    ).unwrap();

    let parsed_v4 = Ipv4Packet::parse(&gtp_ip_packet, true).unwrap();
    assert_eq!(parsed_v4.header.src_ip, upf_ipv4);
    assert_eq!(parsed_v4.header.dst_ip, gnb_ipv4);
    assert_eq!(parsed_v4.header.protocol, IpProtocol::Udp);

    let parsed_udp = UdpDatagram::parse(
        parsed_v4.header.src_ip,
        parsed_v4.header.dst_ip,
        parsed_v4.payload,
        true,
    ).unwrap();
    assert_eq!(parsed_udp.dst_port, GTP_U_UDP_PORT);

    let parsed_gtp = GtpPacket::parse(parsed_udp.payload).unwrap();
    assert_eq!(parsed_gtp.header.teid, teid);
}
