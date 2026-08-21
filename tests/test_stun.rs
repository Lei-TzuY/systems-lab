use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::stun::{
    STUN_ATTR_XOR_MAPPED_ADDRESS, STUN_BINDING_REQUEST, STUN_BINDING_RESPONSE, STUN_MAGIC_COOKIE,
    STUN_PORT, StunPacket,
};

#[test]
fn test_stun_packet_structure_and_constants() {
    let tid = [0x55; 12];
    let req = StunPacket::build_binding_request(tid);
    let raw = req.serialize();

    let parsed = StunPacket::parse(&raw).unwrap();
    assert_eq!(parsed.msg_type, STUN_BINDING_REQUEST);
    assert_eq!(parsed.magic_cookie, STUN_MAGIC_COOKIE);
    assert_eq!(parsed.transaction_id, tid);
    assert_eq!(STUN_PORT, 3478);
    assert_eq!(STUN_MAGIC_COOKIE, 0x2112A442);
}

#[test]
fn test_stun_xor_mapped_address_translation() {
    let req = StunPacket::build_binding_request([0x77; 12]);
    let public_ip = Ipv4Address::new(198, 51, 100, 25);
    let public_port = 61234;

    let resp = StunPacket::build_binding_response(&req, public_ip, public_port);
    let raw_resp = resp.serialize();

    let parsed_resp = StunPacket::parse(&raw_resp).unwrap();
    assert_eq!(parsed_resp.msg_type, STUN_BINDING_RESPONSE);
    assert!(
        parsed_resp
            .attributes
            .iter()
            .any(|a| a.attr_type == STUN_ATTR_XOR_MAPPED_ADDRESS)
    );

    let (ip, port) = parsed_resp.get_xor_mapped_address().unwrap();
    assert_eq!(ip, public_ip);
    assert_eq!(port, public_port);
}
