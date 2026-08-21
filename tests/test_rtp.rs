use toy_tcpip::rtp::{RtcpSenderReport, RtpPacket, RTP_FIXED_HEADER_LEN, RTP_PT_PCMA};

#[test]
fn test_rtp_audio_packet_framing_and_csrc() {
    let payload = b"PCM Audio Frame 20ms";
    let mut pkt = RtpPacket::build_audio(RTP_PT_PCMA, 500, 8000, 0xFEEDBEEF, true, payload);
    pkt.csrc_list.push(0x11112222);

    let raw = pkt.serialize();
    assert_eq!(raw.len(), RTP_FIXED_HEADER_LEN + 4 + payload.len());

    let parsed = RtpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.payload_type, RTP_PT_PCMA);
    assert_eq!(parsed.sequence_number, 500);
    assert_eq!(parsed.timestamp, 8000);
    assert_eq!(parsed.ssrc, 0xFEEDBEEF);
    assert!(parsed.marker);
    assert_eq!(parsed.csrc_list, vec![0x11112222]);
    assert_eq!(parsed.payload, payload);
}

#[test]
fn test_rtcp_sender_report_stats() {
    let sr = RtcpSenderReport::build(0x12345678, 0x1000000020000000, 90000, 250, 40000);
    let raw = sr.serialize();

    let parsed = RtcpSenderReport::parse(&raw).unwrap();
    assert_eq!(parsed.ssrc, 0x12345678);
    assert_eq!(parsed.ntp_timestamp, 0x1000000020000000);
    assert_eq!(parsed.rtp_timestamp, 90000);
    assert_eq!(parsed.packet_count, 250);
    assert_eq!(parsed.octet_count, 40000);
}
