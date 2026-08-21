use toy_tcpip::gre_udp::{GRE_IN_UDP_PORT, GreUdpPacket};

#[test]
fn test_gre_in_udp_constants_and_framing() {
    assert_eq!(GRE_IN_UDP_PORT, 4754);

    let entropy_sport = 60123;
    let key = 0xDEADBEEF;
    let seq = 100;
    let payload = b"GRE-in-UDP Multipath Payload";

    let pkt = GreUdpPacket::new(entropy_sport, 0x0800, Some(key), Some(seq), payload);
    let raw = pkt.serialize();

    let parsed = GreUdpPacket::parse(entropy_sport, GRE_IN_UDP_PORT, &raw).unwrap();
    assert_eq!(parsed.src_port, entropy_sport);
    assert_eq!(parsed.dst_port, GRE_IN_UDP_PORT);
    assert_eq!(parsed.header.key, Some(key));
    assert_eq!(parsed.header.sequence_number, Some(seq));
    assert_eq!(parsed.header.protocol_type, 0x0800);
    assert_eq!(parsed.payload, payload);
}
