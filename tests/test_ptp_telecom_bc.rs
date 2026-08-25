use toy_tcpip::ptp_telecom_bc::{
    TelecomBoundaryClockEngine, TelecomClockQuality, TelecomPortState,
};

#[test]
fn test_ptp_telecom_boundary_clock_alternate_bmca() {
    let mut bc = TelecomBoundaryClockEngine::new();

    // Port 1: Candidate slave, local_priority = 10, not_slave = false
    bc.add_port(1, 10, false);
    // Port 2: Candidate slave, local_priority = 20, not_slave = false
    bc.add_port(2, 20, false);
    // Port 3: Downstream Master-only, not_slave = true
    bc.add_port(3, 128, true);

    // Port 1 receives Class 6 PRTC announce
    bc.update_rx_announce(
        1,
        TelecomClockQuality {
            clock_class: 6,
            clock_accuracy: 0x20,
            offset_scaled_log_variance: 0x4E5D,
        },
        1,
        128,
    );

    // Port 2 receives Class 7 announce
    bc.update_rx_announce(
        2,
        TelecomClockQuality {
            clock_class: 7,
            clock_accuracy: 0x21,
            offset_scaled_log_variance: 0x5A00,
        },
        2,
        128,
    );

    let slave = bc.run_alternate_bmca().expect("elect slave port");
    assert_eq!(slave, 1);
    assert_eq!(bc.port_states.get(&1), Some(&TelecomPortState::Slave));
    assert_eq!(bc.port_states.get(&2), Some(&TelecomPortState::Passive));
    assert_eq!(bc.port_states.get(&3), Some(&TelecomPortState::Master));

    // Phase offset adjustment damping test
    let corr = bc.adjust_phase_offset(40);
    assert_eq!(corr, 20);
    assert_eq!(bc.accumulated_phase_offset_ns, 20);
}
