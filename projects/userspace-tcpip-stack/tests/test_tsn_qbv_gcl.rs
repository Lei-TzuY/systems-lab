use toy_tcpip::tsn_qbv_gcl::{GclEntry, TsnQbvGclEngine};

#[test]
fn test_tsn_qbv_gcl_cyclic_schedule_and_guard_band() {
    let mut tas = TsnQbvGclEngine::new(0, 10_000); // 10 Gbps (1500 bytes takes 1200 ns)

    // Slot 0: TC 7 open only - duration 100_000 ns (100 us)
    tas.add_entry(GclEntry::new(
        [false, false, false, false, false, false, false, true],
        100_000,
    ));
    // Slot 1: TC 0..6 open - duration 400_000 ns (400 us)
    tas.add_entry(GclEntry::new(
        [true, true, true, true, true, true, true, false],
        400_000,
    ));

    assert_eq!(tas.cycle_time_ns, 500_000);

    // At t = 50_000 ns (inside Slot 0):
    // TC 7 is open
    assert!(tas.evaluate_transmission(7, 1500, 50_000));
    // TC 0 is closed in Slot 0
    assert!(!tas.evaluate_transmission(0, 1500, 50_000));

    // At t = 200_000 ns (inside Slot 1, offset = 200_000 ns, Slot 1 ends at 500_000 ns, remaining 300_000 ns):
    // 1500 bytes tx time is 1200 ns <= 300_000 ns -> allowed
    assert!(tas.evaluate_transmission(0, 1500, 200_000));

    // At t = 499_500 ns (inside Slot 1, remaining 500 ns):
    // 1500 bytes tx time is 1200 ns > 500 ns -> BLOCKED by Guard Band!
    assert!(!tas.evaluate_transmission(0, 1500, 499_500));
    assert_eq!(tas.guard_band_blocked_tx, 1);
}
