use toy_tcpip::twamp::{
    TWAMP_CONTROL_PORT, TWAMP_MODE_AUTHENTICATED, TWAMP_MODE_ENCRYPTED, TWAMP_MODE_UNAUTHENTICATED,
    TWAMP_TEST_PORT, TwampServerGreeting, TwampTestPacket, calculate_twamp_metrics,
};

#[test]
fn test_twamp_constants_and_greeting() {
    assert_eq!(TWAMP_CONTROL_PORT, 862);
    assert_eq!(TWAMP_TEST_PORT, 862);
    assert_eq!(TWAMP_MODE_UNAUTHENTICATED, 1);
    assert_eq!(TWAMP_MODE_AUTHENTICATED, 2);
    assert_eq!(TWAMP_MODE_ENCRYPTED, 4);

    let greeting = TwampServerGreeting::new(TWAMP_MODE_UNAUTHENTICATED | TWAMP_MODE_AUTHENTICATED);
    let raw = greeting.serialize();
    assert_eq!(raw.len(), 64);

    let parsed = TwampServerGreeting::parse(&raw).unwrap();
    assert_eq!(
        parsed.modes,
        TWAMP_MODE_UNAUTHENTICATED | TWAMP_MODE_AUTHENTICATED
    );
    assert_eq!(parsed.count, 1024);
    assert_eq!(parsed.challenge, [0x11; 16]);
    assert_eq!(parsed.salt, [0x22; 16]);
}

#[test]
fn test_twamp_test_packet_and_metrics_calculation() {
    let t1_sec = 1700000000;
    let t1_frac = 100000000; // ~0.02328 s

    let req = TwampTestPacket::build_sender_request(10, t1_sec, t1_frac);
    let raw_req = req.serialize();
    assert!(raw_req.len() >= 14);

    let parsed_req = TwampTestPacket::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.seq_number, 10);
    assert_eq!(parsed_req.timestamp_sec, t1_sec);
    assert_eq!(parsed_req.timestamp_frac, t1_frac);

    let t2_sec = 1700000000;
    let t2_frac = 101000000;
    let t3_sec = 1700000000;
    let t3_frac = 101200000;
    let t4_sec = 1700000000;
    let t4_frac = 102300000;

    let resp = TwampTestPacket::build_reflector_response(
        &parsed_req,
        500,
        t2_sec,
        t2_frac,
        t3_sec,
        t3_frac,
        64,
    );
    let raw_resp = resp.serialize();
    assert!(raw_resp.len() >= 41);

    let parsed_resp = TwampTestPacket::parse(&raw_resp).unwrap();
    assert_eq!(parsed_resp.seq_number, 500);
    assert_eq!(parsed_resp.sender_seq_number, Some(10));
    assert_eq!(parsed_resp.sender_ttl, Some(64));
    assert_eq!(parsed_resp.receive_timestamp_sec, Some(t2_sec));

    let metrics = calculate_twamp_metrics(
        t1_sec, t1_frac, t2_sec, t2_frac, t3_sec, t3_frac, t4_sec, t4_frac,
    );
    assert!(metrics.rtt_us > 0.0);
    assert!(metrics.forward_delay_us > 0.0);
    assert!(metrics.reverse_delay_us > 0.0);
}
