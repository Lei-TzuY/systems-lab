use toy_tcpip::http2::{
    Http2Frame, HTTP2_FLAG_ACK, HTTP2_FLAG_END_HEADERS, HTTP2_FLAG_END_STREAM, HTTP2_FRAME_DATA,
    HTTP2_FRAME_HEADERS, HTTP2_FRAME_SETTINGS,
};

#[test]
fn test_http2_headers_and_data_multiplexing() {
    let headers = Http2Frame::build_headers(1, false, true, b":status 200 :content-type text/html");
    let raw_headers = headers.serialize();
    let parsed_h = Http2Frame::parse(&raw_headers).unwrap();

    assert_eq!(parsed_h.stream_id, 1);
    assert_eq!(parsed_h.frame_type, HTTP2_FRAME_HEADERS);
    assert_eq!(parsed_h.flags & HTTP2_FLAG_END_HEADERS, HTTP2_FLAG_END_HEADERS);

    let data = Http2Frame::build_data(1, true, b"<html><body>Hello HTTP/2</body></html>");
    let raw_data = data.serialize();
    let parsed_d = Http2Frame::parse(&raw_data).unwrap();

    assert_eq!(parsed_d.stream_id, 1);
    assert_eq!(parsed_d.frame_type, HTTP2_FRAME_DATA);
    assert_eq!(parsed_d.flags & HTTP2_FLAG_END_STREAM, HTTP2_FLAG_END_STREAM);
}

#[test]
fn test_http2_control_frames() {
    let settings_ack = Http2Frame::build_settings(true);
    let parsed_s = Http2Frame::parse(&settings_ack.serialize()).unwrap();
    assert_eq!(parsed_s.frame_type, HTTP2_FRAME_SETTINGS);
    assert_eq!(parsed_s.flags, HTTP2_FLAG_ACK);

    let ping = Http2Frame::build_ping(false, [0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04]);
    let parsed_p = Http2Frame::parse(&ping.serialize()).unwrap();
    assert_eq!(parsed_p.payload.len(), 8);

    let goaway = Http2Frame::build_goaway(1, 0);
    let parsed_g = Http2Frame::parse(&goaway.serialize()).unwrap();
    assert_eq!(parsed_g.payload.len(), 8);
}
