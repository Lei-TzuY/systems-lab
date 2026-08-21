use toy_tcpip::ipv4::Ipv4Address;
use toy_tcpip::wireguard::{
    WG_MSG_DATA, WG_MSG_INITIATION, WG_MSG_RESPONSE, WIREGUARD_PORT, WireguardMessage,
    WireguardPeer,
};

#[test]
fn test_wireguard_handshake_flow_and_data_packet() {
    let ephem_a = [0x11; 32];
    let ephem_b = [0x22; 32];

    let init = WireguardMessage::build_initiation(0x1000, ephem_a);
    let raw_init = init.serialize();
    assert_eq!(raw_init.len(), 148);

    let parsed_init = WireguardMessage::parse(&raw_init).unwrap();
    if let WireguardMessage::HandshakeInitiation { sender_index, .. } = parsed_init {
        assert_eq!(sender_index, 0x1000);
    } else {
        panic!("Expected HandshakeInitiation");
    }

    let resp = WireguardMessage::build_response(0x2000, 0x1000, ephem_b);
    let raw_resp = resp.serialize();
    assert_eq!(raw_resp.len(), 92);

    let parsed_resp = WireguardMessage::parse(&raw_resp).unwrap();
    if let WireguardMessage::HandshakeResponse {
        sender_index,
        receiver_index,
        ..
    } = parsed_resp
    {
        assert_eq!(sender_index, 0x2000);
        assert_eq!(receiver_index, 0x1000);
    } else {
        panic!("Expected HandshakeResponse");
    }

    let mut peer = WireguardPeer::new(
        [0x33; 32],
        Ipv4Address::new(192, 168, 1, 10),
        WIREGUARD_PORT,
        Ipv4Address::new(10, 99, 0, 2),
    );
    peer.local_index = 0x1000;
    peer.handle_response(0x2000, 0x1000);
    assert!(peer.is_established);

    let data_bytes = peer.encapsulate_packet(b"GET / HTTP/1.1\r\n\r\n").unwrap();
    let parsed_data = WireguardMessage::parse(&data_bytes).unwrap();
    if let WireguardMessage::Data {
        receiver_index,
        counter,
        encrypted_payload,
    } = parsed_data
    {
        assert_eq!(receiver_index, 0x2000);
        assert_eq!(counter, 0);
        assert_eq!(&encrypted_payload[..18], b"GET / HTTP/1.1\r\n\r\n");
    } else {
        panic!("Expected Data message");
    }

    assert_eq!(WIREGUARD_PORT, 51820);
    assert_eq!(WG_MSG_INITIATION, 1);
    assert_eq!(WG_MSG_RESPONSE, 2);
    assert_eq!(WG_MSG_DATA, 4);
}
