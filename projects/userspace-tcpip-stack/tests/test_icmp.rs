use toy_tcpip::icmp::{IcmpPacket, IcmpType};

#[test]
fn test_icmp_echo_request_and_reply() {
    let payload = b"Timestamp and ping sequence payload 0123456789";
    let req_bytes = IcmpPacket::build_echo_request(0x5678, 42, payload);

    let req = IcmpPacket::parse(&req_bytes, true).expect("Valid ICMP Request");
    assert_eq!(req.icmp_type, IcmpType::EchoRequest);
    assert_eq!(req.code, 0);
    assert_eq!(req.identifier, 0x5678);
    assert_eq!(req.sequence_number, 42);
    assert_eq!(req.payload, payload);

    let reply_bytes = IcmpPacket::build_echo_reply(&req);
    let reply = IcmpPacket::parse(&reply_bytes, true).expect("Valid ICMP Reply");
    assert_eq!(reply.icmp_type, IcmpType::EchoReply);
    assert_eq!(reply.code, 0);
    assert_eq!(reply.identifier, 0x5678);
    assert_eq!(reply.sequence_number, 42);
    assert_eq!(reply.payload, payload);
}
