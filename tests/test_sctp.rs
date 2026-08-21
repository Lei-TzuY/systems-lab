use toy_tcpip::sctp::{SctpPacket, IP_PROTO_SCTP, SCTP_CHUNK_DATA, SCTP_CHUNK_INIT};

#[test]
fn test_sctp_init_chunk_structure() {
    let init = SctpPacket::build_init(6000, 7000, 0x55667788, 32768, 5, 5, 2000);
    let raw = init.serialize();

    let parsed = SctpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.src_port, 6000);
    assert_eq!(parsed.header.dst_port, 7000);
    assert_eq!(parsed.header.verification_tag, 0);
    assert_eq!(parsed.chunks.len(), 1);
    assert_eq!(parsed.chunks[0].chunk_type, SCTP_CHUNK_INIT);
    assert_eq!(IP_PROTO_SCTP, 132);
}

#[test]
fn test_sctp_data_chunk_streaming() {
    let data = SctpPacket::build_data(6000, 7000, 0x55667788, 10, 1, 0, 0, b"Multi-stream Payload");
    let raw = data.serialize();

    let parsed = SctpPacket::parse(&raw).unwrap();
    assert_eq!(parsed.header.verification_tag, 0x55667788);
    assert_eq!(parsed.chunks[0].chunk_type, SCTP_CHUNK_DATA);
    assert!(parsed.chunks[0].value.ends_with(b"Multi-stream Payload"));
}
