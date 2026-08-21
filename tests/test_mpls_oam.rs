use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::mpls_oam::{
    LspEchoPacket, TargetFecIpv4, FEC_SUBTYPE_IPV4_PREFIX, LSP_MSG_ECHO_REPLY, LSP_MSG_ECHO_REQUEST,
    LSP_PING_UDP_PORT, LSP_REPLY_MODE_UDP, LSP_RET_CODE_EGRESS_FOR_FEC, LSP_TLV_TARGET_FEC_STACK,
};

#[test]
fn test_mpls_lsp_ping_constants_and_echo_request() {
    assert_eq!(LSP_PING_UDP_PORT, 3503);
    assert_eq!(LSP_MSG_ECHO_REQUEST, 1);
    assert_eq!(LSP_MSG_ECHO_REPLY, 2);
    assert_eq!(LSP_REPLY_MODE_UDP, 2);
    assert_eq!(LSP_TLV_TARGET_FEC_STACK, 1);
    assert_eq!(FEC_SUBTYPE_IPV4_PREFIX, 1);

    let fec_ip = Ipv4Address::new(10, 200, 1, 1);
    let req = LspEchoPacket::build_echo_request(0xCAFEBABE, 100, fec_ip, 32, 1700000000, 123456);
    let raw = req.serialize();
    assert!(raw.len() >= 32);

    let parsed = LspEchoPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.msg_type, LSP_MSG_ECHO_REQUEST);
    assert_eq!(parsed.sender_handle, 0xCAFEBABE);
    assert_eq!(parsed.seq_number, 100);
    assert_eq!(
        parsed.target_fec,
        Some(TargetFecIpv4 {
            prefix: fec_ip,
            mask_len: 32,
        })
    );
}

#[test]
fn test_mpls_lsp_echo_reply_generation() {
    let fec_ip = Ipv4Address::new(192, 168, 100, 0);
    let req = LspEchoPacket::build_echo_request(0x1234, 5, fec_ip, 24, 1700000000, 500);

    let reply = LspEchoPacket::build_echo_reply(&req, LSP_RET_CODE_EGRESS_FOR_FEC, 1700000000, 800);
    let raw_reply = reply.serialize();

    let parsed_reply = LspEchoPacket::parse(&raw_reply).unwrap();
    assert_eq!(parsed_reply.msg_type, LSP_MSG_ECHO_REPLY);
    assert_eq!(parsed_reply.return_code, LSP_RET_CODE_EGRESS_FOR_FEC);
    assert_eq!(parsed_reply.sender_handle, 0x1234);
    assert_eq!(parsed_reply.seq_number, 5);
    assert_eq!(parsed_reply.timestamp_recv_frac, 800);
}
