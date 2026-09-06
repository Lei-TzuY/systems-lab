use toy_tcpip::tas::TimeAwareShaper;

#[test]
fn test_tas_gcl_cycle_and_gate_open_checks() {
    let mut tas = TimeAwareShaper::new();
    tas.add_entry(0x80, 200); // 0..200µs: Queue 7 (0b10000000)
    tas.add_entry(0x01, 300); // 200..500µs: Queue 0 (0b00000001)

    assert_eq!(tas.cycle_time_us, 500);

    // Queue 7 checks
    assert!(tas.is_queue_open(7, 100));
    assert!(!tas.is_queue_open(7, 250));

    // Queue 0 checks
    assert!(!tas.is_queue_open(0, 100));
    assert!(tas.is_queue_open(0, 250));
    assert!(tas.is_queue_open(0, 750)); // (750 % 500 = 250)
}

#[test]
fn test_tas_guard_band_and_closed_drops() {
    let mut tas = TimeAwareShaper::new();
    tas.guard_band_us = 20;
    tas.add_entry(0x01, 100); // 0..100µs: Queue 0
    tas.add_entry(0x80, 100); // 100..200µs: Queue 7

    // Queue 7 at t=50µs (Gate closed)
    assert!(!tas.can_transmit(7, 64, 1000, 50));
    assert_eq!(tas.gate_closed_drops, 1);

    // Queue 0 at t=20µs (80µs remaining > 12µs tx + 20µs guard band)
    assert!(tas.can_transmit(0, 1500, 1000, 20));
    assert_eq!(tas.transmitted_frames, 1);

    // Queue 0 at t=80µs (20µs remaining < 12µs tx + 20µs guard band = 32µs needed) -> Guard band drop
    assert!(!tas.can_transmit(0, 1500, 1000, 80));
    assert_eq!(tas.guard_band_drops, 1);
}
