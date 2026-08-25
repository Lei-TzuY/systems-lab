use toy_tcpip::ptp_time_error::{PtpTimeErrorEngine, TelecomClockClass};

#[test]
fn test_ptp_time_error_cte_and_peak_to_peak() {
    let mut engine = PtpTimeErrorEngine::new(5);

    // Add 5 samples: 10, 20, 15, 25, 30 ns
    engine.add_sample(10);
    engine.add_sample(20);
    engine.add_sample(15);
    engine.add_sample(25);
    engine.add_sample(30);

    let cte = engine.calculate_cte();
    assert_eq!(cte, 20.0);

    let p2p = engine.calculate_peak_to_peak_te();
    assert_eq!(p2p, 20); // 30 - 10 = 20 ns

    // Meets Class C (|cTE| <= 30ns, |TE| <= 55ns)
    assert!(engine.verify_compliance(TelecomClockClass::ClassC));
    // Fails Class D (|cTE| <= 15ns)
    assert!(!engine.verify_compliance(TelecomClockClass::ClassD));

    // Ring buffer overflow test
    engine.add_sample(5); // Window becomes [20, 15, 25, 30, 5]
    assert_eq!(engine.samples.len(), 5);
    assert_eq!(engine.calculate_cte(), 19.0);
}
