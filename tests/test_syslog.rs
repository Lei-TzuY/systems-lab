use toy_tcpip::syslog::{SyslogCollector, SyslogFacility, SyslogMessage, SyslogSeverity};

#[test]
fn test_syslog_prival_calculation_and_rfc5424_roundtrip() {
    let msg = SyslogMessage::new(
        SyslogFacility::Daemon,
        SyslogSeverity::Error,
        "edge-gw",
        "netstack",
        "Interface link eth0 DOWN: carrier lost",
    );

    // Facility Daemon (3) * 8 + Severity Error (3) = 27
    assert_eq!(msg.pri_val(), 27);

    let raw_str = msg.format_rfc5424();
    assert!(raw_str.starts_with("<27>1"));

    let parsed = SyslogMessage::parse_rfc5424(&raw_str).unwrap();
    assert_eq!(parsed.facility, SyslogFacility::Daemon);
    assert_eq!(parsed.severity, SyslogSeverity::Error);
    assert_eq!(parsed.hostname, "edge-gw");
    assert_eq!(parsed.app_name, "netstack");
    assert_eq!(parsed.message, "Interface link eth0 DOWN: carrier lost");
}

#[test]
fn test_syslog_collector_capacity() {
    let mut collector = SyslogCollector::new(5);
    for i in 0..10 {
        let msg = SyslogMessage::new(
            SyslogFacility::Local0,
            SyslogSeverity::Informational,
            "host",
            "app",
            &format!("Log message {}", i),
        );
        collector.record(msg);
    }

    assert_eq!(collector.logs.len(), 5);
    assert_eq!(collector.logs.last().unwrap().message, "Log message 9");
}
