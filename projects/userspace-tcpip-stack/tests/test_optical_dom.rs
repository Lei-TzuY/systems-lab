use toy_tcpip::optical_dom::{OpticalDiagnostics, TransceiverFormFactor};

#[test]
fn test_optical_dom_telemetry_and_unit_conversions() {
    let dom = OpticalDiagnostics::new(
        "FortyGigE0/1/0/1",
        TransceiverFormFactor::Qsfp28_100G,
        35.0,
        3.30,
        40.0,
        0.0,   // 0 dBm = 1.0 mW
        -10.0, // -10 dBm = 0.1 mW
    );

    let tx_mw = OpticalDiagnostics::dbm_to_mw(dom.tx_power_dbm);
    assert!((tx_mw - 1.0).abs() < 0.01);

    let rx_mw = OpticalDiagnostics::dbm_to_mw(dom.rx_power_dbm);
    assert!((rx_mw - 0.1).abs() < 0.01);

    assert_eq!(dom.link_attenuation_db(), 10.0);
    assert_eq!(dom.rx_optical_margin_db(), 8.0); // -10.0 - (-18.0) = 8.0 dB
}

#[test]
fn test_optical_dom_high_temperature_alarm() {
    let dom = OpticalDiagnostics::new(
        "TenGigE0/0/0/8",
        TransceiverFormFactor::SfpPlus10G,
        82.5, // Exceeds 75.0 C high alarm
        3.15,
        25.0,
        -3.0,
        -12.0,
    );

    let alarms = dom.evaluate_alarms();
    assert!(alarms.temp_alarm);
    assert!(!alarms.rx_los);
    assert!(!alarms.tx_fault);
}
