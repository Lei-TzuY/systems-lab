use toy_tcpip::sip::{SIP_PORT, SipMessage, SipMethod, build_simple_sdp};

#[test]
fn test_sip_invite_sdp_and_ok_response() {
    let sdp = build_simple_sdp("alice", "192.168.1.100", 4000);
    let invite =
        SipMessage::build_invite("alice@example.com", "bob@example.com", "call-abc-123", &sdp);
    let serialized = invite.serialize();

    let parsed = SipMessage::parse(&serialized).unwrap();
    assert!(!parsed.is_response);
    assert_eq!(parsed.method, Some(SipMethod::Invite));
    assert_eq!(parsed.headers.get("Call-ID").unwrap(), "call-abc-123");
    assert!(parsed.body.contains("m=audio 4000 RTP/AVP 0"));

    let server_sdp = build_simple_sdp("bob", "192.168.1.10", 5000);
    let ok = SipMessage::build_200_ok(&parsed, &server_sdp);
    let ok_serialized = ok.serialize();

    let parsed_ok = SipMessage::parse(&ok_serialized).unwrap();
    assert!(parsed_ok.is_response);
    assert_eq!(parsed_ok.status_code, 200);
    assert_eq!(parsed_ok.reason_phrase, "OK");
    assert_eq!(SIP_PORT, 5060);
}
