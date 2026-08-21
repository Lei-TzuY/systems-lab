use toy_tcpip::http3::{Http3Frame, HTTP3_FRAME_DATA, HTTP3_FRAME_HEADERS, HTTP3_FRAME_SETTINGS};

#[test]
fn test_http3_vint_framing_roundtrip() {
    let settings_frame = Http3Frame::build_settings(&[(0x06, 65535), (0x01, 1024)]);
    let raw = settings_frame.serialize();

    let (parsed, len) = Http3Frame::parse(&raw).unwrap();
    assert_eq!(len, raw.len());
    assert_eq!(parsed.frame_type, HTTP3_FRAME_SETTINGS);
}

#[test]
fn test_http3_multiplexed_headers_and_payload() {
    let headers = vec![(":method", "POST"), (":path", "/graphql"), (":scheme", "https")];
    let hdr_frame = Http3Frame::build_headers(&headers);
    let hdr_raw = hdr_frame.serialize();

    let (parsed_hdr, _) = Http3Frame::parse(&hdr_raw).unwrap();
    assert_eq!(parsed_hdr.frame_type, HTTP3_FRAME_HEADERS);

    let body = b"{\"query\": \"{ user(id: 1) { name } }\"}";
    let data_frame = Http3Frame::build_data(body);
    let data_raw = data_frame.serialize();

    let (parsed_data, _) = Http3Frame::parse(&data_raw).unwrap();
    assert_eq!(parsed_data.frame_type, HTTP3_FRAME_DATA);
    assert_eq!(parsed_data.payload, body);
}
