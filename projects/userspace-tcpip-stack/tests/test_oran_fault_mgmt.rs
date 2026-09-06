//! Comprehensive Integration Tests for O-RAN WG4 Open Fronthaul OAM Fault Management Engine.

use toy_tcpip::oran_fault_mgmt::*;

#[test]
fn test_oran_fault_raise_and_clear_lifecycle() {
    let mut fm = OranFaultManager::new(50);
    // Use 0ms soak for immediate lifecycle evaluation
    fm.set_soak_config(
        OranFaultId::OverTemperatureProtection,
        SoakConfig {
            raise_soak_ms: 0,
            clear_soak_ms: 0,
        },
    );

    let t0 = 1750000000000; // Epoch milliseconds

    // 1. Raise fault: Over-temperature on sensor 1
    let raise_notif = fm
        .report_condition(
            OranFaultId::OverTemperatureProtection,
            "ru-sensor[id=1]",
            true,
            OranFaultSeverity::Critical,
            "Internal PA temperature 95C exceeds 85C limit",
            t0,
        )
        .expect("Should immediately raise notification with 0ms soak");

    assert_eq!(raise_notif.seq_num, 1);
    assert_eq!(raise_notif.event_type, NotificationEventType::Raise);
    assert_eq!(raise_notif.alarm.alarm_id, 1);
    assert_eq!(
        raise_notif.alarm.fault_id,
        OranFaultId::OverTemperatureProtection
    );
    assert_eq!(raise_notif.alarm.severity, OranFaultSeverity::Critical);
    assert!(!raise_notif.alarm.is_cleared);
    assert!(!raise_notif.alarm.is_acknowledged);

    // Verify active alarm table
    assert_eq!(fm.active_alarms.len(), 1);
    let active = fm.active_alarms.get(&1).unwrap();
    assert_eq!(active.fault_source, "ru-sensor[id=1]");
    assert_eq!(active.probable_cause, "temperatureUnacceptable");

    // 2. Update severity: temperature cools to 88C (Major)
    let t1 = t0 + 5000;
    let update_notif = fm
        .report_condition(
            OranFaultId::OverTemperatureProtection,
            "ru-sensor[id=1]",
            true,
            OranFaultSeverity::Major,
            "Internal PA temperature 88C exceeds 85C limit",
            t1,
        )
        .expect("Should emit SeverityUpdate notification");

    assert_eq!(update_notif.seq_num, 2);
    assert_eq!(
        update_notif.event_type,
        NotificationEventType::SeverityUpdate
    );
    assert_eq!(update_notif.alarm.severity, OranFaultSeverity::Major);

    // 3. Operator acknowledges alarm
    let t2 = t1 + 2000;
    let ack_notif = fm
        .acknowledge_alarm(1, "smo-operator-01", t2)
        .expect("Acknowledge should succeed");

    assert_eq!(ack_notif.seq_num, 3);
    assert_eq!(ack_notif.event_type, NotificationEventType::Acknowledge);
    assert!(ack_notif.alarm.is_acknowledged);
    assert_eq!(
        ack_notif.alarm.acknowledged_by,
        Some("smo-operator-01".to_string())
    );
    assert_eq!(ack_notif.alarm.acknowledged_time_ms, Some(t2));

    // 4. Fault clears: temperature returns to normal (70C)
    let t3 = t2 + 10000;
    let clear_notif = fm
        .report_condition(
            OranFaultId::OverTemperatureProtection,
            "ru-sensor[id=1]",
            false,
            OranFaultSeverity::Cleared,
            "Temperature restored to normal",
            t3,
        )
        .expect("Should emit Clear notification");

    assert_eq!(clear_notif.seq_num, 4);
    assert_eq!(clear_notif.event_type, NotificationEventType::Clear);
    assert!(clear_notif.alarm.is_cleared);
    assert_eq!(clear_notif.alarm.severity, OranFaultSeverity::Cleared);

    // Active table must now be empty
    assert!(fm.active_alarms.is_empty());

    // History audit log contains all 4 sequential events
    assert_eq!(fm.get_history().len(), 4);
    assert_eq!(fm.get_history()[0].event_type, NotificationEventType::Raise);
    assert_eq!(
        fm.get_history()[1].event_type,
        NotificationEventType::SeverityUpdate
    );
    assert_eq!(
        fm.get_history()[2].event_type,
        NotificationEventType::Acknowledge
    );
    assert_eq!(fm.get_history()[3].event_type, NotificationEventType::Clear);
}

#[test]
fn test_oran_fault_soak_timer_flapping_damping() {
    let mut fm = OranFaultManager::new(50);
    let fault = OranFaultId::PtpClockLossOfLock;
    let source = "ptp-profile[domain=24]";

    // Configure 200 ms raise soak and 400 ms clear soak
    fm.set_soak_config(
        fault,
        SoakConfig {
            raise_soak_ms: 200,
            clear_soak_ms: 400,
        },
    );

    let mut t = 1000;

    // --- Phase 1: Transient Glitch (< 200 ms) ---
    // Fault detected at t=1000
    let res1 = fm.report_condition(
        fault,
        source,
        true,
        OranFaultSeverity::Major,
        "Phase drift transient",
        t,
    );
    assert!(res1.is_none()); // Soaking raise

    // Only 60 ms elapsed: glitch clears at t=1060
    t = 1060;
    let res2 = fm.report_condition(
        fault,
        source,
        false,
        OranFaultSeverity::Cleared,
        "Drift normalized",
        t,
    );
    assert!(res2.is_none());

    // Time advances past 200 ms threshold (t=1400)
    t = 1400;
    let timer_events = fm.step_time(t);
    assert!(timer_events.is_empty());
    assert!(fm.active_alarms.is_empty()); // No false alarm generated!

    // --- Phase 2: Persistent Fault (> 200 ms) ---
    t = 2000;
    fm.report_condition(
        fault,
        source,
        true,
        OranFaultSeverity::Critical,
        "Persistent PTP lock loss",
        t,
    );

    // Advance 100 ms (t=2100): still soaking
    let mid_events = fm.step_time(2100);
    assert!(mid_events.is_empty());
    assert!(fm.active_alarms.is_empty());

    // Advance past 200 ms (t=2210): raise timer expires!
    let raise_events = fm.step_time(2210);
    assert_eq!(raise_events.len(), 1);
    assert_eq!(raise_events[0].alarm.alarm_id, 1);
    assert_eq!(raise_events[0].alarm.severity, OranFaultSeverity::Critical);
    assert_eq!(fm.active_alarms.len(), 1);

    // --- Phase 3: Glitch Clear (< 400 ms) ---
    t = 3000;
    fm.report_condition(
        fault,
        source,
        false,
        OranFaultSeverity::Cleared,
        "Temporary lock blip",
        t,
    ); // Starts clear soak

    // Fault reappears after only 150 ms (t=3150 < 400 ms)
    t = 3150;
    fm.report_condition(
        fault,
        source,
        true,
        OranFaultSeverity::Critical,
        "Lock lost again",
        t,
    );

    // Step time forward: alarm must remain active (clear soak was cancelled)
    let abort_clear_events = fm.step_time(3600);
    assert!(abort_clear_events.is_empty());
    assert_eq!(fm.active_alarms.len(), 1);

    // --- Phase 4: Persistent Recovery (> 400 ms) ---
    t = 4000;
    fm.report_condition(
        fault,
        source,
        false,
        OranFaultSeverity::Cleared,
        "Full recovery",
        t,
    );

    // Advance past 400 ms (t=4450)
    let clear_events = fm.step_time(4450);
    assert_eq!(clear_events.len(), 1);
    assert_eq!(clear_events[0].event_type, NotificationEventType::Clear);
    assert!(fm.active_alarms.is_empty());
}

#[test]
fn test_oran_fault_root_cause_alarm_storm_suppression() {
    let mut fm = OranFaultManager::new(50);
    let sfp_source = "interface[name='eth0']";

    // Immediate soak for this test
    let no_soak = SoakConfig {
        raise_soak_ms: 0,
        clear_soak_ms: 0,
    };
    fm.set_soak_config(OranFaultId::LossOfSignal, no_soak);
    fm.set_soak_config(OranFaultId::LossOfFrame, no_soak);
    fm.set_soak_config(OranFaultId::PtpClockLossOfLock, no_soak);
    fm.set_soak_config(OranFaultId::EcPriSequenceFailure, no_soak);
    fm.set_soak_config(OranFaultId::DelayJitterExceeded, no_soak);
    fm.set_soak_config(OranFaultId::OverTemperatureProtection, no_soak);

    let t = 10000;

    // 1. Optical link failure: LossOfSignal on eth0
    let los_notif = fm
        .report_condition(
            OranFaultId::LossOfSignal,
            sfp_source,
            true,
            OranFaultSeverity::Critical,
            "Optical Rx power below receiver sensitivity floor",
            t,
        )
        .unwrap();

    let root_alarm_id = los_notif.alarm.alarm_id;
    assert_eq!(root_alarm_id, 1);
    assert!(!los_notif.alarm.is_suppressed);
    assert_eq!(los_notif.alarm.root_cause_alarm_id, None);

    // 2. Cascading dependent faults on the same interface eth0
    let lof_notif = fm
        .report_condition(
            OranFaultId::LossOfFrame,
            sfp_source,
            true,
            OranFaultSeverity::Critical,
            "eCPRI framing delimiter synchronization lost",
            t + 1,
        )
        .unwrap();

    let ptp_notif = fm
        .report_condition(
            OranFaultId::PtpClockLossOfLock,
            sfp_source,
            true,
            OranFaultSeverity::Critical,
            "SyncE frequency lock lost",
            t + 2,
        )
        .unwrap();

    let seq_notif = fm
        .report_condition(
            OranFaultId::EcPriSequenceFailure,
            sfp_source,
            true,
            OranFaultSeverity::Major,
            "eCPRI message sequence gap detected",
            t + 3,
        )
        .unwrap();

    // Verify all cascading faults were suppressed and linked to root cause alarm 1
    assert!(lof_notif.alarm.is_suppressed);
    assert_eq!(lof_notif.alarm.root_cause_alarm_id, Some(root_alarm_id));

    assert!(ptp_notif.alarm.is_suppressed);
    assert_eq!(ptp_notif.alarm.root_cause_alarm_id, Some(root_alarm_id));

    assert!(seq_notif.alarm.is_suppressed);
    assert_eq!(seq_notif.alarm.root_cause_alarm_id, Some(root_alarm_id));

    // 3. Unrelated fault on another source (temperature sensor) is NOT suppressed
    let temp_notif = fm
        .report_condition(
            OranFaultId::OverTemperatureProtection,
            "ru-sensor[id=1]",
            true,
            OranFaultSeverity::Major,
            "Baseband temperature elevated",
            t + 4,
        )
        .unwrap();

    assert!(!temp_notif.alarm.is_suppressed);
    assert_eq!(temp_notif.alarm.root_cause_alarm_id, None);

    // 4. Query active alarms with suppression filter:
    // Without suppressed alarms: only root cause (LOS) and independent temp alarm are visible to SMO!
    let standard_filter = AlarmFilter {
        include_suppressed: false,
        ..Default::default()
    };
    let smo_visible = fm.get_active_alarms(&standard_filter);
    assert_eq!(smo_visible.len(), 2);
    assert_eq!(smo_visible[0].fault_id, OranFaultId::LossOfSignal);
    assert_eq!(
        smo_visible[1].fault_id,
        OranFaultId::OverTemperatureProtection
    );

    // Query with suppressed included returns all 5 active alarms
    let full_filter = AlarmFilter {
        include_suppressed: true,
        ..Default::default()
    };
    let all_active = fm.get_active_alarms(&full_filter);
    assert_eq!(all_active.len(), 5);
}

#[test]
fn test_oran_fault_active_alarm_filtering() {
    let mut fm = OranFaultManager::new(50);
    let no_soak = SoakConfig {
        raise_soak_ms: 0,
        clear_soak_ms: 0,
    };
    fm.set_soak_config(OranFaultId::LossOfSignal, no_soak);
    fm.set_soak_config(OranFaultId::TxPowerDegraded, no_soak);
    fm.set_soak_config(OranFaultId::VswrThresholdExceeded, no_soak);
    fm.set_soak_config(OranFaultId::RxOverdrive, no_soak);

    let t = 1000;
    // Alarm 1: Critical on interface eth0
    fm.report_condition(
        OranFaultId::LossOfSignal,
        "interface[name='eth0']",
        true,
        OranFaultSeverity::Critical,
        "LOS",
        t,
    );
    // Alarm 2: Major on interface eth1
    fm.report_condition(
        OranFaultId::TxPowerDegraded,
        "interface[name='eth1']",
        true,
        OranFaultSeverity::Major,
        "Tx Degraded",
        t,
    );
    // Alarm 3: Minor on antenna 1 (acknowledged)
    fm.report_condition(
        OranFaultId::VswrThresholdExceeded,
        "antenna[id=1]",
        true,
        OranFaultSeverity::Minor,
        "VSWR elevated",
        t,
    );
    fm.acknowledge_alarm(3, "rf-engineer", t + 10).unwrap();

    // Alarm 4: Warning on antenna 2
    fm.report_condition(
        OranFaultId::RxOverdrive,
        "antenna[id=2]",
        true,
        OranFaultSeverity::Warning,
        "LNA Warning",
        t,
    );

    // 1. Severity filter: Major and above
    let filter_sev = AlarmFilter {
        min_severity: Some(OranFaultSeverity::Major),
        include_suppressed: true,
        ..Default::default()
    };
    let sev_results = fm.get_active_alarms(&filter_sev);
    assert_eq!(sev_results.len(), 2);
    assert_eq!(sev_results[0].alarm_id, 1);
    assert_eq!(sev_results[1].alarm_id, 2);

    // 2. Source prefix filter: "antenna"
    let filter_antenna = AlarmFilter {
        source_prefix: Some("antenna".to_string()),
        include_suppressed: true,
        ..Default::default()
    };
    let antenna_results = fm.get_active_alarms(&filter_antenna);
    assert_eq!(antenna_results.len(), 2);
    assert_eq!(antenna_results[0].alarm_id, 3);
    assert_eq!(antenna_results[1].alarm_id, 4);

    // 3. Acknowledged only
    let filter_ack = AlarmFilter {
        acknowledged_only: Some(true),
        include_suppressed: true,
        ..Default::default()
    };
    let ack_results = fm.get_active_alarms(&filter_ack);
    assert_eq!(ack_results.len(), 1);
    assert_eq!(ack_results[0].alarm_id, 3);

    // 4. Unacknowledged only
    let filter_unack = AlarmFilter {
        unacknowledged_only: Some(true),
        include_suppressed: true,
        ..Default::default()
    };
    let unack_results = fm.get_active_alarms(&filter_unack);
    assert_eq!(unack_results.len(), 3);
    assert_eq!(unack_results[0].alarm_id, 1);
    assert_eq!(unack_results[1].alarm_id, 2);
    assert_eq!(unack_results[2].alarm_id, 4);
}

#[test]
fn test_oran_fault_netconf_yang_serialization() {
    let mut fm = OranFaultManager::new(50);
    fm.set_soak_config(
        OranFaultId::LossOfSignal,
        SoakConfig {
            raise_soak_ms: 0,
            clear_soak_ms: 0,
        },
    );

    let t = 1750000000000;
    let notif = fm
        .report_condition(
            OranFaultId::LossOfSignal,
            "interface[name='eth0']",
            true,
            OranFaultSeverity::Critical,
            "Optical signal loss",
            t,
        )
        .unwrap();

    // Verify RFC 7950 XML serialization
    let xml = notif.to_xml();
    assert!(xml.contains("xmlns=\"urn:ietf:params:xml:ns:netconf:notification:1.0\""));
    assert!(xml.contains("xmlns=\"urn:o-ran:fm:1.0\""));
    assert!(xml.contains("<fault-id>1001</fault-id>"));
    assert!(xml.contains("<fault-source>interface[name='eth0']</fault-source>"));
    assert!(xml.contains("<fault-severity>CRITICAL</fault-severity>"));
    assert!(xml.contains("<is-cleared>false</is-cleared>"));
    assert!(xml.contains("<fault-text>Optical signal loss</fault-text>"));
    assert!(xml.contains("T"));
    assert!(xml.contains("Z"));

    // Verify JSON serialization
    let json = notif.to_json();
    assert!(json.contains("\"fault_id\":1001"));
    assert!(json.contains("\"fault_source\":\"interface[name='eth0']\""));
    assert!(json.contains("\"severity\":\"CRITICAL\""));
    assert!(json.contains("\"is_cleared\":false"));
    assert!(json.contains("\"is_suppressed\":false"));
    assert!(json.contains("\"root_cause_alarm_id\":null"));
}
