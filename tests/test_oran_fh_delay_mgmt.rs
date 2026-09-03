//! Integration tests for O-RAN WG4 fronthaul delay management windows.

use toy_tcpip::oran_fh_delay_mgmt::{
    DelayMgmtError, FronthaulWindowKind, NetworkDelayBudget, OduTransmissionWindow,
    OranDelayManager, OruReceptionWindow, OruUplinkWindow, OruWindowCapability, WindowVerdict,
    check_window_compatibility, derive_odu_reception_window, derive_reception_window,
    derive_transmission_window, nr_symbol_air_time_ns,
};

/// Typical LLS-C3 style fronthaul: 5 us to 50 us of one-way transport delay.
fn budget() -> NetworkDelayBudget {
    NetworkDelayBudget::symmetric(5_000, 50_000).unwrap()
}

#[test]
fn test_network_delay_budget_variation_and_asymmetry() {
    let symmetric = budget();
    assert_eq!(symmetric.t12_variation_ns(), 45_000);
    assert_eq!(symmetric.t34_variation_ns(), 45_000);
    assert_eq!(symmetric.round_trip_max_ns(), 100_000);
    assert_eq!(symmetric.asymmetry_ns(), 0);

    // A WDM fronthaul with different fibre lengths per direction.
    let asymmetric = NetworkDelayBudget::new(5_000, 50_000, 6_000, 62_000).unwrap();
    assert_eq!(asymmetric.asymmetry_ns(), -12_000);
    assert_eq!(asymmetric.round_trip_max_ns(), 112_000);

    assert_eq!(
        NetworkDelayBudget::new(50_000, 5_000, 0, 0),
        Err(DelayMgmtError::InvertedWindow {
            min_ns: 50_000,
            max_ns: 5_000
        })
    );
}

#[test]
fn test_reception_window_derivation_widens_by_delay_variation() {
    let tx =
        OduTransmissionWindow::new(FronthaulWindowKind::CPlaneDownlink, 258_000, 500_000).unwrap();
    assert_eq!(tx.width_ns(), 242_000);
    assert_eq!(tx.kind.label(), "C-Plane downlink (T1a_cp_dl)");

    let rx = derive_reception_window(&tx, &budget()).unwrap();
    // T2a_min = T1a_min - T12_max, T2a_max = T1a_max - T12_min.
    assert_eq!(rx.t2a_min_ns, 208_000);
    assert_eq!(rx.t2a_max_ns, 495_000);
    // The arrival envelope is wider than the transmit window by the delay variation.
    assert_eq!(rx.width_ns(), tx.width_ns() + 45_000);
    assert_eq!(rx.kind, FronthaulWindowKind::CPlaneDownlink);

    // Deriving back gives the original transmission window.
    let round_trip = derive_transmission_window(&rx, &budget()).unwrap();
    assert_eq!(round_trip.t1a_min_ns, 258_000);
    assert_eq!(round_trip.t1a_max_ns, 500_000);

    // The usual configuration direction: the O-RU publishes what it can buffer.
    let advertised =
        OruReceptionWindow::new(FronthaulWindowKind::UPlaneDownlink, 50_000, 345_000).unwrap();
    let required = derive_transmission_window(&advertised, &budget()).unwrap();
    assert_eq!(required.t1a_min_ns, 100_000);
    assert_eq!(required.t1a_max_ns, 350_000);
    assert_eq!(required.width_ns(), advertised.width_ns() - 45_000);

    // An O-RU window narrower than the network's delay variation cannot be served.
    let narrow =
        OruReceptionWindow::new(FronthaulWindowKind::UPlaneDownlink, 50_000, 80_000).unwrap();
    assert_eq!(
        derive_transmission_window(&narrow, &budget()),
        Err(DelayMgmtError::InvertedWindow {
            min_ns: 100_000,
            max_ns: 85_000
        })
    );

    assert_eq!(
        OruReceptionWindow::new(FronthaulWindowKind::CPlaneUplink, 400_000, 400_000),
        Err(DelayMgmtError::WindowCollapsed {
            t2a_min_ns: 400_000,
            t2a_max_ns: 400_000
        })
    );
}

#[test]
fn test_reception_window_classifies_early_late_and_on_time() {
    let window =
        OruReceptionWindow::new(FronthaulWindowKind::CPlaneDownlink, 208_000, 495_000).unwrap();

    // Comfortably inside: the margin is the distance to the nearer edge.
    assert_eq!(
        window.classify(300_000),
        WindowVerdict::OnTime { margin_ns: 92_000 }
    );
    assert!(window.accepts(300_000));

    // Sent further ahead than the O-RU can buffer.
    assert_eq!(
        window.classify(500_000),
        WindowVerdict::TooEarly { by_ns: 5_000 }
    );
    // Arrived so close to the air time that the symbol can no longer be prepared.
    assert_eq!(
        window.classify(100_000),
        WindowVerdict::TooLate { by_ns: 108_000 }
    );
    assert!(!window.accepts(100_000));

    // The window edges themselves are inside it.
    assert_eq!(
        window.classify(208_000),
        WindowVerdict::OnTime { margin_ns: 0 }
    );
    assert_eq!(
        window.classify(495_000),
        WindowVerdict::OnTime { margin_ns: 0 }
    );
    assert!(WindowVerdict::OnTime { margin_ns: 0 }.is_on_time());
    assert!(!WindowVerdict::TooLate { by_ns: 1 }.is_on_time());
}

#[test]
fn test_uplink_ta3_ta4_window_runs_forward_from_air_time() {
    let ta3 = OruUplinkWindow {
        ta3_min_ns: 70_000,
        ta3_max_ns: 150_000,
    };
    let ta4 = derive_odu_reception_window(&ta3, &budget()).unwrap();

    // Uplink windows add both bounds: Ta4 = Ta3 + T34.
    assert_eq!(ta4.ta4_min_ns, 75_000);
    assert_eq!(ta4.ta4_max_ns, 200_000);
    assert_eq!(ta4.width_ns(), 125_000);

    assert_eq!(
        ta4.classify(100_000),
        WindowVerdict::OnTime { margin_ns: 25_000 }
    );
    // Uplink data cannot reach the O-DU before the O-RU has captured the symbol.
    assert_eq!(
        ta4.classify(60_000),
        WindowVerdict::TooEarly { by_ns: 15_000 }
    );
    assert_eq!(
        ta4.classify(210_000),
        WindowVerdict::TooLate { by_ns: 10_000 }
    );

    let inverted = OruUplinkWindow {
        ta3_min_ns: 150_000,
        ta3_max_ns: 70_000,
    };
    assert_eq!(
        derive_odu_reception_window(&inverted, &budget()),
        Err(DelayMgmtError::InvertedWindow {
            min_ns: 150_000,
            max_ns: 70_000
        })
    );
}

#[test]
fn test_window_capability_intersection() {
    let derived =
        OruReceptionWindow::new(FronthaulWindowKind::UPlaneDownlink, 208_000, 495_000).unwrap();

    let capable = OruWindowCapability {
        supported_min_ns: 100_000,
        supported_max_ns: 400_000,
    };
    let overlap = check_window_compatibility(&derived, &capable).unwrap();
    assert_eq!(overlap.usable_min_ns, 208_000);
    assert_eq!(overlap.usable_max_ns, 400_000);
    assert_eq!(overlap.usable_width_ns, 192_000);

    // An O-RU that only buffers far ahead of the air time cannot serve this O-DU.
    let mismatched = OruWindowCapability {
        supported_min_ns: 600_000,
        supported_max_ns: 700_000,
    };
    assert_eq!(
        check_window_compatibility(&derived, &mismatched),
        Err(DelayMgmtError::IncompatibleWindows {
            derived_min_ns: 208_000,
            derived_max_ns: 495_000,
            supported_min_ns: 600_000,
            supported_max_ns: 700_000,
        })
    );
}

#[test]
fn test_nr_symbol_air_time_scales_with_numerology() {
    // mu = 0: 1 ms slots, 14 symbols each.
    assert_eq!(nr_symbol_air_time_ns(0, 0, 0, 0, 0), 0);
    assert_eq!(nr_symbol_air_time_ns(0, 0, 0, 1, 0), 71_428);
    assert_eq!(nr_symbol_air_time_ns(0, 1, 0, 0, 0), 1_000_000);

    // mu = 1: 0.5 ms slots, so slot 1 sits half a subframe in.
    assert_eq!(nr_symbol_air_time_ns(0, 0, 1, 0, 1), 500_000);
    assert_eq!(nr_symbol_air_time_ns(1, 2, 1, 3, 1), 12_607_142);

    // A radio frame is 10 ms regardless of numerology.
    assert_eq!(nr_symbol_air_time_ns(1, 0, 0, 0, 3), 10_000_000);
    // mu = 3: 125 us slots.
    assert_eq!(nr_symbol_air_time_ns(0, 0, 7, 0, 3), 875_000);
}

#[test]
fn test_delay_manager_tracks_window_violations_and_headroom() {
    let window =
        OruReceptionWindow::new(FronthaulWindowKind::UPlaneDownlink, 208_000, 495_000).unwrap();
    let mut manager = OranDelayManager::new(window);
    assert_eq!(manager.on_time_ratio(), 0.0);
    assert_eq!(manager.observed_variation_ns(), None);

    let air_time = nr_symbol_air_time_ns(0, 0, 0, 0, 1) + 1_000_000;

    // 300 us ahead of the air time: inside the window.
    assert!(manager.observe(air_time, air_time - 300_000).is_on_time());
    // 100 us ahead: too late for the O-RU to prepare the symbol.
    assert_eq!(
        manager.observe(air_time, air_time - 100_000),
        WindowVerdict::TooLate { by_ns: 108_000 }
    );
    // 600 us ahead: earlier than the O-RU can buffer.
    assert_eq!(
        manager.observe(air_time, air_time - 600_000),
        WindowVerdict::TooEarly { by_ns: 105_000 }
    );

    assert_eq!(manager.observed(), 3);
    assert_eq!(manager.on_time, 1);
    assert_eq!(manager.too_late, 1);
    assert_eq!(manager.too_early, 1);
    assert!((manager.on_time_ratio() - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(manager.worst_margin_ns(), Some(92_000));

    // The traffic actually spanned 100 us to 600 us of advance.
    assert_eq!(manager.required_window(), Some((100_000, 600_000)));
    assert_eq!(manager.observed_variation_ns(), Some(500_000));
    // Negative headroom: the configured window does not cover what arrived.
    assert_eq!(manager.headroom_ns(), Some(-108_000));

    manager.reset();
    assert_eq!(manager.observed(), 0);
    assert_eq!(manager.required_window(), None);
    assert_eq!(manager.worst_margin_ns(), None);

    // A well behaved flow keeps positive headroom on both edges.
    for advance in [250_000, 300_000, 400_000, 450_000] {
        assert!(manager.observe_advance(advance).is_on_time());
    }
    assert_eq!(manager.on_time_ratio(), 1.0);
    assert_eq!(manager.headroom_ns(), Some(42_000));
    assert_eq!(manager.worst_margin_ns(), Some(42_000));
}
