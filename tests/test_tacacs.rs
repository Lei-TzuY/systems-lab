use toy_tcpip::tacacs::{TacacsPacket, TacacsServer, TACACS_AUTHEN_STATUS_FAIL, TACACS_AUTHEN_STATUS_PASS, TACACS_PORT, TACACS_TYPE_AUTHEN};

#[test]
fn test_tacacs_header_and_packet_serialization() {
    let pkt = TacacsPacket::build_authen_start(0xDEADBEEF, "netadmin", "vty0", "secretpass");
    let raw = pkt.serialize();

    let parsed = TacacsPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.session_id, 0xDEADBEEF);
    assert_eq!(parsed.header.packet_type, TACACS_TYPE_AUTHEN);
    assert_eq!(parsed.header.seq_no, 1);
    assert_eq!(TACACS_PORT, 49);
}

#[test]
fn test_tacacs_server_authentication() {
    let mut server = TacacsServer::new();
    server.users.insert("alice".to_string(), "wonderland".to_string());

    let req_ok = TacacsPacket::build_authen_start(1, "alice", "tty1", "wonderland");
    let resp_ok = server.authenticate(&req_ok);
    assert_eq!(resp_ok.header.session_id, 1);
    assert_eq!(resp_ok.header.seq_no, 2);
    assert_eq!(resp_ok.body[0], TACACS_AUTHEN_STATUS_PASS);

    let req_bad = TacacsPacket::build_authen_start(2, "alice", "tty1", "wrong");
    let resp_bad = server.authenticate(&req_bad);
    assert_eq!(resp_bad.body[0], TACACS_AUTHEN_STATUS_FAIL);
}
