use toy_tcpip::ptp::{calculate_ptp_offset_and_delay, PtpPacket, PtpTimestamp, ETHERTYPE_PTP, PTP_EVENT_PORT, PTP_GENERAL_PORT, PTP_HEADER_LEN, PTP_MSG_FOLLOW_UP, PTP_MSG_SYNC};

#[test]
fn test_ptp_sync_and_follow_up_packets() {
    let clock_id = [0xAA, 0xBB, 0xCC, 0xFF, 0xFE, 0xDD, 0xEE, 0xFF];
    let ts = PtpTimestamp::new(1700000000, 123456789);

    let sync = PtpPacket::build_sync(clock_id, 10, ts);
    let raw_sync = sync.serialize();
    assert_eq!(raw_sync.len(), PTP_HEADER_LEN + 10);

    let parsed_sync = PtpPacket::parse(&raw_sync).unwrap();
    assert_eq!(parsed_sync.header.message_type, PTP_MSG_SYNC);
    assert_eq!(parsed_sync.header.clock_identity, clock_id);
    assert_eq!(parsed_sync.header.sequence_id, 10);
    assert_eq!(PTP_EVENT_PORT, 319);
    assert_eq!(PTP_GENERAL_PORT, 320);
    assert_eq!(ETHERTYPE_PTP, 0x88F7);

    let follow_up = PtpPacket::build_follow_up(clock_id, 10, ts);
    let raw_fu = follow_up.serialize();
    let parsed_fu = PtpPacket::parse(&raw_fu).unwrap();
    assert_eq!(parsed_fu.header.message_type, PTP_MSG_FOLLOW_UP);
}

#[test]
fn test_ptp_nanosecond_timing_calculation() {
    let t1 = PtpTimestamp::new(1000, 0);
    let t2 = PtpTimestamp::new(1000, 50);  // Master to slave takes 50ns
    let t3 = PtpTimestamp::new(1000, 100);
    let t4 = PtpTimestamp::new(1000, 150); // Slave to master takes 50ns

    let (offset, delay) = calculate_ptp_offset_and_delay(t1, t2, t3, t4);
    assert_eq!(offset, 0);
    assert_eq!(delay, 50); // 50ns mean one-way path delay
}
