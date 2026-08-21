use toy_tcpip::ptp_tc::{HopMeasurement, TransparentClockEngine, TransparentClockMode};

#[test]
fn test_ptp_tc_e2e_residence_time_accumulation() {
    let mut tc = TransparentClockEngine::new(TransparentClockMode::EndToEnd);
    let hop1 = HopMeasurement {
        ingress_timestamp_ns: 1000,
        egress_timestamp_ns: 1250, // 250ns residence
    };
    let hop2 = HopMeasurement {
        ingress_timestamp_ns: 5000,
        egress_timestamp_ns: 5180, // 180ns residence
    };

    let corr1 = tc.update_correction_field(0, &hop1);
    assert_eq!(corr1, 250);

    let corr2 = tc.update_correction_field(corr1, &hop2);
    assert_eq!(corr2, 430);

    assert_eq!(tc.corrected_packets_count, 2);
    assert_eq!(tc.total_residence_time_ns, 430);
}

#[test]
fn test_ptp_tc_p2p_peer_delay_and_scaled_ns() {
    let mut tc = TransparentClockEngine::new(TransparentClockMode::PeerToPeer);
    let pdelay = tc.calculate_peer_delay(100, 200, 250, 450);
    // round_trip = 450 - 100 = 350; turnaround = 250 - 200 = 50; delay = (350 - 50) / 2 = 150ns
    assert_eq!(pdelay, 150);

    let hop = HopMeasurement {
        ingress_timestamp_ns: 10_000,
        egress_timestamp_ns: 10_100, // 100ns residence
    };

    let updated = tc.update_correction_field(20, &hop);
    assert_eq!(updated, 20 + 100 + 150); // 270ns

    let scaled = TransparentClockEngine::to_scaled_nanoseconds(updated);
    assert_eq!(TransparentClockEngine::from_scaled_nanoseconds(scaled), 270);
}
