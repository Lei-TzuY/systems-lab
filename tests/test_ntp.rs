use toy_tcpip::ntp::{
    NTP_MODE_CLIENT, NTP_MODE_SERVER, NTP_VERSION_4, NtpPacket, NtpTimestamp,
    calculate_offset_and_delay,
};

#[test]
fn test_ntp_packet_structure_and_serialization() {
    let t1 = NtpTimestamp::new(3900000000, 123456);
    let req = NtpPacket::build_client_request(t1);
    let raw = req.serialize();

    let parsed = NtpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, NTP_VERSION_4);
    assert_eq!(parsed.mode, NTP_MODE_CLIENT);
    assert_eq!(parsed.transmit_timestamp, t1);

    let t2 = NtpTimestamp::new(3900000000, 200000);
    let t3 = NtpTimestamp::new(3900000000, 205000);
    let resp = NtpPacket::build_server_response(&parsed, t2, t3);
    let parsed_resp = NtpPacket::parse(&resp.serialize()).unwrap();

    assert_eq!(parsed_resp.mode, NTP_MODE_SERVER);
    assert_eq!(parsed_resp.stratum, 1);
    assert_eq!(parsed_resp.origin_timestamp, t1);
    assert_eq!(parsed_resp.receive_timestamp, t2);
    assert_eq!(parsed_resp.transmit_timestamp, t3);
}

#[test]
fn test_ntp_timestamp_fractional_precision() {
    let unix = 1715000000.750; // .75 fraction
    let ts = NtpTimestamp::from_unix_f64(unix);
    let recovered = ts.to_unix_f64();
    assert!((recovered - unix).abs() < 0.0001);

    let (offset, delay) = calculate_offset_and_delay(10.0, 10.05, 10.06, 10.12);
    assert!((delay - 0.11).abs() < 0.0001);
    assert!((offset - (-0.005)).abs() < 0.0001);
}
