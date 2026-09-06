use toy_tcpip::gtp::{
    GTP_MSG_ECHO_REQUEST, GTP_MSG_ECHO_RESPONSE, GTP_MSG_GPDU, GTP_U_UDP_PORT, GtpPacket,
    GtpTunnelTable,
};
use toy_tcpip::ipv4::Ipv4Address;

#[test]
fn test_gtp_gpdu_encapsulation_and_tunnel_table() {
    let teid = 0xDEADBEEF;
    let payload = b"Subscribed 5G User Plane IP Packet";
    let gpdu = GtpPacket::build_gpdu(teid, payload);
    let raw = gpdu.serialize();

    let parsed = GtpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.version, 1);
    assert_eq!(parsed.header.msg_type, GTP_MSG_GPDU);
    assert_eq!(parsed.header.teid, teid);
    assert_eq!(parsed.payload, payload);
    assert_eq!(GTP_U_UDP_PORT, 2152);

    let mut table = GtpTunnelTable::new();
    let sub_ip = Ipv4Address::new(10, 45, 0, 100);
    let upf_ip = Ipv4Address::new(10, 0, 0, 1);
    table.insert_session(teid, sub_ip, upf_ip);

    let s = table.sessions.get(&teid).unwrap();
    assert_eq!(s.subscriber_ip, sub_ip);
    assert_eq!(s.gnb_upf_ip, upf_ip);
}

#[test]
fn test_gtp_echo_request_response() {
    let req = GtpPacket::build_echo_request(0, 500);
    let raw_req = req.serialize();
    let parsed_req = GtpPacket::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.header.msg_type, GTP_MSG_ECHO_REQUEST);
    assert_eq!(parsed_req.header.seq_num, Some(500));

    let resp = GtpPacket::build_echo_response(0, 500);
    let raw_resp = resp.serialize();
    let parsed_resp = GtpPacket::parse(&raw_resp).unwrap();
    assert_eq!(parsed_resp.header.msg_type, GTP_MSG_ECHO_RESPONSE);
    assert_eq!(parsed_resp.header.seq_num, Some(500));
}
