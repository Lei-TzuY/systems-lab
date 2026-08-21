use toy_tcpip::websocket::{
    WebSocketFrame, WS_OPCODE_BINARY, WS_OPCODE_CLOSE, WS_OPCODE_PING, WS_OPCODE_PONG,
    WS_OPCODE_TEXT,
};

#[test]
fn test_websocket_text_and_binary_roundtrips() {
    // 1. Unmasked Text Frame
    let text = "Hello WebSocket Realtime Feed";
    let text_frame = WebSocketFrame::build_text(text, false, None);
    let parsed_text = WebSocketFrame::parse(&text_frame.serialize()).unwrap();
    assert_eq!(parsed_text.opcode, WS_OPCODE_TEXT);
    assert_eq!(String::from_utf8(parsed_text.unmasked_payload()).unwrap(), text);

    // 2. Masked Binary Frame (Client to Server)
    let binary_data = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
    let mask = [0x12, 0x34, 0x56, 0x78];
    let bin_frame = WebSocketFrame::build_binary(&binary_data, true, Some(mask));
    let parsed_bin = WebSocketFrame::parse(&bin_frame.serialize()).unwrap();
    assert_eq!(parsed_bin.opcode, WS_OPCODE_BINARY);
    assert_eq!(parsed_bin.unmasked_payload(), binary_data);
}

#[test]
fn test_websocket_control_frames() {
    let ping = WebSocketFrame::build_ping(b"keepalive");
    let parsed_ping = WebSocketFrame::parse(&ping.serialize()).unwrap();
    assert_eq!(parsed_ping.opcode, WS_OPCODE_PING);
    assert_eq!(parsed_ping.payload, b"keepalive");

    let pong = WebSocketFrame::build_pong(b"keepalive");
    let parsed_pong = WebSocketFrame::parse(&pong.serialize()).unwrap();
    assert_eq!(parsed_pong.opcode, WS_OPCODE_PONG);

    let close = WebSocketFrame::build_close();
    let parsed_close = WebSocketFrame::parse(&close.serialize()).unwrap();
    assert_eq!(parsed_close.opcode, WS_OPCODE_CLOSE);
}
