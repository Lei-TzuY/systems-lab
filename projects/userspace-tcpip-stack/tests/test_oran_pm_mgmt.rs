//! Comprehensive Integration Tests for O-RAN WG4 Open Fronthaul Performance Management Engine.

use toy_tcpip::oran_pm_mgmt::*;

#[test]
fn test_oran_pm_transceiver_and_rx_window_metrics() {
    let mut engine = OranPmEngine::new();

    let sfp_meas = TransceiverMeasurement {
        port_id: 0,
        tx_power_dbm: -2.10,
        rx_power_dbm: -5.45,
        supply_voltage_v: 3.298,
        tx_bias_current_ma: 32.10,
        temperature_c: 38.5,
    };

    let alerts = engine.record_transceiver(sfp_meas.clone(), 1000);
    assert!(alerts.is_empty());
    assert_eq!(engine.current_transceivers.len(), 1);
    assert_eq!(engine.current_transceivers.get(&0), Some(&sfp_meas));

    let rx_win = RxWindowMeasurement {
        ru_port_id: 0,
        early_packets: 120,
        on_time_packets: 9500,
        late_packets: 4,
        corrupt_packets: 0,
    };

    let rx_alerts = engine.record_rx_window(rx_win.clone(), 1001);
    assert!(rx_alerts.is_empty());
    assert_eq!(engine.current_rx_windows.len(), 1);
    assert_eq!(engine.current_rx_windows.get(&0), Some(&rx_win));
}

#[test]
fn test_oran_pm_interval_rollover_and_circular_history() {
    let mut engine = OranPmEngine::new();

    // 5-second interval job retaining up to 3 historical bins
    let job = MeasurementJob::new("job-5s", 5, 3);
    engine.add_job(job);

    // Populate current metrics
    engine.record_transceiver(
        TransceiverMeasurement {
            port_id: 1,
            tx_power_dbm: -1.5,
            rx_power_dbm: -4.0,
            supply_voltage_v: 3.3,
            tx_bias_current_ma: 30.0,
            temperature_c: 40.0,
        },
        100,
    );
    engine.record_ecpri(
        EcpriTransportMeasurement {
            interface_id: 1,
            tx_packets: 10000,
            rx_packets: 9998,
            tx_bytes: 14000000,
            rx_bytes: 13997200,
            buffer_overflow_drops: 0,
            sequence_errors: 2,
            one_way_delay_ns: 12500,
        },
        100,
    );
    engine.record_tx_prb(
        TxPrbMeasurement {
            carrier_id: 0,
            antenna_port_id: 0,
            prb_usage_percent: 68.5,
            peak_prb_usage: 273,
            total_active_symbols: 1400,
        },
        100,
    );

    // 1. Tick 4 seconds: not finished yet
    for t in 101..=104 {
        let finished = engine.tick_second(t);
        assert!(finished.is_empty());
    }

    // 2. 5th second: Interval 1 completed
    let finished1 = engine.tick_second(105);
    assert_eq!(finished1.len(), 1);
    assert_eq!(finished1[0].0, "job-5s");
    assert_eq!(finished1[0].1.interval_id, 1);
    assert_eq!(finished1[0].1.duration_seconds, 5);
    assert_eq!(finished1[0].1.start_timestamp_seconds, 100);
    assert_eq!(finished1[0].1.transceiver_records.len(), 1);
    assert_eq!(finished1[0].1.ecpri_records.len(), 1);
    assert_eq!(finished1[0].1.prb_records.len(), 1);

    // 3. Complete Interval 2 (t = 106..110)
    for t in 106..=110 {
        engine.tick_second(t);
    }
    // 4. Complete Interval 3 (t = 111..115)
    for t in 111..=115 {
        engine.tick_second(t);
    }

    let job_ref = engine.jobs.get("job-5s").unwrap();
    assert_eq!(job_ref.history.len(), 3);
    assert_eq!(job_ref.history[0].interval_id, 1);
    assert_eq!(job_ref.history[1].interval_id, 2);
    assert_eq!(job_ref.history[2].interval_id, 3);

    // 5. Complete Interval 4 (t = 116..120): Circular history evicts Interval 1
    for t in 116..=120 {
        engine.tick_second(t);
    }

    let job_ref2 = engine.jobs.get("job-5s").unwrap();
    assert_eq!(job_ref2.history.len(), 3);
    assert_eq!(job_ref2.history[0].interval_id, 2);
    assert_eq!(job_ref2.history[1].interval_id, 3);
    assert_eq!(job_ref2.history[2].interval_id, 4);
}

#[test]
fn test_oran_pm_threshold_crossing_alert_rising() {
    let mut engine = OranPmEngine::new();

    // High temperature rising TCA
    engine.add_tca_config(ThresholdCrossingConfig {
        metric_name: "transceiver.1.temperature".to_string(),
        warning_threshold: Some(60.0),
        alarm_threshold: Some(75.0),
        hysteresis: 5.0,
        direction: TcaDirection::Rising,
    });

    let make_sfp = |temp: f32| TransceiverMeasurement {
        port_id: 1,
        tx_power_dbm: 0.0,
        rx_power_dbm: 0.0,
        supply_voltage_v: 3.3,
        tx_bias_current_ma: 30.0,
        temperature_c: temp,
    };

    // 1. Normal temperature (45.0 C): no alert
    let a1 = engine.record_transceiver(make_sfp(45.0), 100);
    assert!(a1.is_empty());

    // 2. Temp crosses Warning threshold (62.0 C)
    let a2 = engine.record_transceiver(make_sfp(62.0), 101);
    assert_eq!(a2.len(), 1);
    assert_eq!(a2[0].severity, TcaSeverity::Warning);
    assert_eq!(a2[0].direction, TcaDirection::Rising);
    assert!(!a2[0].cleared);
    assert_eq!(a2[0].current_value, 62.0);
    assert_eq!(a2[0].threshold_value, 60.0);

    // 3. Repeated temp at 64.0 C: does not re-emit warning alert
    let a3 = engine.record_transceiver(make_sfp(64.0), 102);
    assert!(a3.is_empty());

    // 4. Temp escalates to Alarm threshold (78.0 C)
    let a4 = engine.record_transceiver(make_sfp(78.0), 103);
    assert_eq!(a4.len(), 1);
    assert_eq!(a4[0].severity, TcaSeverity::Alarm);
    assert!(!a4[0].cleared);
    assert_eq!(a4[0].current_value, 78.0);
    assert_eq!(a4[0].threshold_value, 75.0);
}

#[test]
fn test_oran_pm_tca_hysteresis_and_clear() {
    let mut engine = OranPmEngine::new();

    // Falling optical Rx power alert
    engine.add_tca_config(ThresholdCrossingConfig {
        metric_name: "transceiver.0.rx_power".to_string(),
        warning_threshold: Some(-10.0),
        alarm_threshold: Some(-15.0),
        hysteresis: 2.0,
        direction: TcaDirection::Falling,
    });

    let make_rx = |rx_pow: f32| TransceiverMeasurement {
        port_id: 0,
        tx_power_dbm: 0.0,
        rx_power_dbm: rx_pow,
        supply_voltage_v: 3.3,
        tx_bias_current_ma: 30.0,
        temperature_c: 35.0,
    };

    // 1. Drop into Warning (-11.0 dBm <= -10.0 dBm)
    let a1 = engine.record_transceiver(make_rx(-11.0), 200);
    assert_eq!(a1.len(), 1);
    assert_eq!(a1[0].severity, TcaSeverity::Warning);

    // 2. Drop into Alarm (-16.0 dBm <= -15.0 dBm)
    let a2 = engine.record_transceiver(make_rx(-16.0), 201);
    assert_eq!(a2.len(), 1);
    assert_eq!(a2[0].severity, TcaSeverity::Alarm);

    // 3. Power recovers to -14.0 dBm:
    // With hysteresis = 2.0 dB, clear threshold is -15.0 + 2.0 = -13.0 dBm.
    // -14.0 dBm is still below -13.0 dBm -> alert stays active!
    let a3 = engine.record_transceiver(make_rx(-14.0), 202);
    assert!(a3.is_empty());

    // 4. Power recovers to -12.5 dBm (>= -13.0 dBm):
    // Alarm cleared!
    let a4 = engine.record_transceiver(make_rx(-12.5), 203);
    assert_eq!(a4.len(), 1);
    assert_eq!(a4[0].severity, TcaSeverity::Alarm);
    assert!(a4[0].cleared);
    assert_eq!(a4[0].threshold_value, -13.0);
}

#[test]
fn test_oran_pm_xml_and_json_serialization() {
    let record = MeasurementIntervalRecord {
        interval_id: 42,
        start_timestamp_seconds: 1600000000,
        duration_seconds: 900,
        transceiver_records: vec![TransceiverMeasurement {
            port_id: 0,
            tx_power_dbm: -1.25,
            rx_power_dbm: -4.80,
            supply_voltage_v: 3.305,
            tx_bias_current_ma: 34.2,
            temperature_c: 41.2,
        }],
        rx_window_records: vec![RxWindowMeasurement {
            ru_port_id: 0,
            early_packets: 10,
            on_time_packets: 50000,
            late_packets: 2,
            corrupt_packets: 0,
        }],
        ecpri_records: Vec::new(),
        prb_records: Vec::new(),
    };

    // 1. XML serialization
    let xml = OranPmEngine::format_xml_pm_data(&record);
    assert!(xml.contains("<performance-measurement-data xmlns=\"urn:o-ran:pm:1.0\">"));
    assert!(xml.contains("<interval-id>42</interval-id>"));
    assert!(xml.contains("<duration>900</duration>"));
    assert!(xml.contains("<tx-power-dbm>-1.25</tx-power-dbm>"));
    assert!(xml.contains("<rx-power-dbm>-4.80</rx-power-dbm>"));
    assert!(xml.contains("<on-time>50000</on-time>"));
    assert!(xml.contains("<late>2</late>"));

    // 2. JSON serialization
    let json = OranPmEngine::format_json_pm_data(&record);
    assert!(json.contains("\"interval_id\": 42"));
    assert!(json.contains("\"duration\": 900"));
    assert!(json.contains("\"port\": 0"));
    assert!(json.contains("\"tx_power_dbm\": -1.25"));
    assert!(json.contains("\"on_time\": 50000"));

    // 3. TCA XML serialization
    let alert = ThresholdCrossingAlert {
        metric_name: "transceiver.0.temperature".to_string(),
        current_value: 78.5,
        threshold_value: 75.0,
        direction: TcaDirection::Rising,
        severity: TcaSeverity::Alarm,
        cleared: false,
        timestamp_seconds: 1600000900,
    };
    let tca_xml = OranPmEngine::format_xml_tca(&alert);
    assert!(tca_xml.contains("<threshold-crossing-alert xmlns=\"urn:o-ran:pm:1.0\">"));
    assert!(tca_xml.contains("<metric>transceiver.0.temperature</metric>"));
    assert!(tca_xml.contains("<severity>Alarm</severity>"));
    assert!(tca_xml.contains("<cleared>false</cleared>"));
}
