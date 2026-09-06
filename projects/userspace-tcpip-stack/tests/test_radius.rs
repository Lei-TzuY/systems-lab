use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::radius::{
    RADIUS_ATTR_USER_NAME, RADIUS_CODE_ACCESS_ACCEPT, RADIUS_CODE_ACCESS_REQUEST, RadiusPacket,
};

#[test]
fn test_radius_access_request_and_password_obfuscation() {
    let auth = [0x99; 16];
    let secret = b"mysecretkey";
    let req = RadiusPacket::build_access_request(
        42,
        auth,
        "bob",
        "password123",
        secret,
        Ipv4Address::new(192, 168, 1, 1),
    );
    let raw = req.serialize();

    let parsed = RadiusPacket::parse(&raw).unwrap();
    assert_eq!(parsed.code, RADIUS_CODE_ACCESS_REQUEST);
    assert_eq!(parsed.identifier, 42);
    assert_eq!(parsed.authenticator, auth);
    assert_eq!(parsed.attributes[0].attr_type, RADIUS_ATTR_USER_NAME);
    assert_eq!(parsed.attributes[0].value, b"bob");
}

#[test]
fn test_radius_access_accept_payload() {
    let auth = [0x33; 16];
    let framed_ip = Ipv4Address::new(10, 10, 10, 100);
    let accept = RadiusPacket::build_access_accept(42, auth, framed_ip, "Access Granted");
    let raw = accept.serialize();

    let parsed = RadiusPacket::parse(&raw).unwrap();
    assert_eq!(parsed.code, RADIUS_CODE_ACCESS_ACCEPT);
    assert_eq!(parsed.identifier, 42);
    assert_eq!(parsed.attributes.len(), 2);
}
