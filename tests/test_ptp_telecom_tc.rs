use toy_tcpip::ptp_telecom_tc::TelecomPeerTransparentClockEngine;

#[test]
fn test_ptp_telecom_p2p_transparent_clock_correction() {
    let mut tc = TelecomPeerTransparentClockEngine::new();

    // Port 1 link delay measurement:
    // t1 = 1000ns, t2 = 1200ns, t3 = 2000ns, t4 = 2400ns
    // Peer Delay = ((2400 - 1000) - (2000 - 1200)) / 2 = (1400 - 800) / 2 = 300ns
    let delay = tc.compute_peer_delay(1000, 1200, 2000, 2400);
    assert_eq!(delay, 300);

    tc.set_port_peer_delay(1, delay);

    // Event packet residence correction:
    // Ingress Port 1 at Tin = 5000ns, Egress Port 2 at Tout = 5600ns (Residence Time = 600ns)
    // Initial correction = 50ns
    // Expected correction = 50 + 600 (residence) + 300 (peer delay) = 950ns
    let updated_corr = tc.correct_event_packet(1, 5000, 5600, 50);
    assert_eq!(updated_corr, 950);
    assert_eq!(tc.corrections_performed, 1);
    assert_eq!(tc.accumulated_correction_ns, 900);
}
