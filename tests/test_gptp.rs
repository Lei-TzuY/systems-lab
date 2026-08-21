use toy_tcpip::gptp::{calculate_gptp_peer_delay, GptpPacket, GptpTimestamp, ETHERTYPE_GPTP, GPTP_MSG_PDELAY_REQ, GPTP_MSG_PDELAY_RESP, GPTP_MULTICAST_MAC};

#[test]
fn test_gptp_pdelay_req_resp_and_peer_delay_calculation() {
    let clock_master = [0x00, 0x11, 0x22, 0xFF, 0xFE, 0x33, 0x44, 0x55];
    let clock_slave = [0x00, 0x66, 0x77, 0xFF, 0xFE, 0x88, 0x99, 0xAA];

    let t1 = GptpTimestamp::new(1700000000, 50_000_000);
    let t2 = GptpTimestamp::new(1700000000, 50_000_035); // 35 ns link delay
    let t3 = GptpTimestamp::new(1700000000, 50_002_000); // 2 us responder turn-around
    let t4 = GptpTimestamp::new(1700000000, 50_002_035); // 35 ns link delay

    let req = GptpPacket::build_pdelay_req(clock_slave, 1, 1001, t1);
    let raw_req = req.serialize();
    assert_eq!(raw_req.len(), 54);

    let parsed_req = GptpPacket::parse(&raw_req).unwrap();
    assert_eq!(parsed_req.header.transport_specific, 1); // IEEE 802.1AS
    assert_eq!(parsed_req.header.message_type, GPTP_MSG_PDELAY_REQ);
    assert_eq!(parsed_req.header.clock_identity, clock_slave);
    assert_eq!(parsed_req.origin_timestamp, Some(t1));

    let resp = GptpPacket::build_pdelay_resp(clock_master, 2, clock_slave, 1, 1001, t2);
    let raw_resp = resp.serialize();
    let parsed_resp = GptpPacket::parse(&raw_resp).unwrap();
    assert_eq!(parsed_resp.header.message_type, GPTP_MSG_PDELAY_RESP);
    assert_eq!(parsed_resp.origin_timestamp, Some(t2));

    let delay = calculate_gptp_peer_delay(t1, t2, t3, t4);
    assert_eq!(delay, 35); // Exactly 35 nanoseconds

    assert_eq!(ETHERTYPE_GPTP, 0x88F7);
    assert_eq!(GPTP_MULTICAST_MAC.0, [0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E]);
}
