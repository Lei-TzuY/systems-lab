use toy_tcpip::tls::{
    TlsRecord, TLS_CONTENT_APPLICATION_DATA, TLS_CONTENT_HANDSHAKE, TLS_HANDSHAKE_CLIENT_HELLO,
    TLS_HANDSHAKE_SERVER_HELLO,
};

#[test]
fn test_tls_record_layer_framing() {
    let payload = b"Hello TLS 1.3 World!";
    let rec = TlsRecord::build_application_data(payload);
    let serialized = rec.serialize();

    assert_eq!(serialized[0], TLS_CONTENT_APPLICATION_DATA);
    let parsed = TlsRecord::parse(&serialized).unwrap();
    assert_eq!(parsed.payload, payload);
}

#[test]
fn test_tls_handshake_messages() {
    let client_random = [0x11; 32];
    let server_random = [0x22; 32];

    let ch = TlsRecord::build_client_hello("secure.example.com", client_random);
    let parsed_ch = TlsRecord::parse(&ch.serialize()).unwrap();
    assert_eq!(parsed_ch.content_type, TLS_CONTENT_HANDSHAKE);
    assert_eq!(parsed_ch.payload[0], TLS_HANDSHAKE_CLIENT_HELLO);

    let sh = TlsRecord::build_server_hello(server_random);
    let parsed_sh = TlsRecord::parse(&sh.serialize()).unwrap();
    assert_eq!(parsed_sh.content_type, TLS_CONTENT_HANDSHAKE);
    assert_eq!(parsed_sh.payload[0], TLS_HANDSHAKE_SERVER_HELLO);
}
