use toy_tcpip::otlp::{OtlpExporter, OtlpMetric, OtlpSpan, OTLP_GRPC_PORT, OTLP_HTTP_PORT};

#[test]
fn test_otlp_exporter_metrics_and_spans() {
    assert_eq!(OTLP_GRPC_PORT, 4317);
    assert_eq!(OTLP_HTTP_PORT, 4318);

    let mut exporter = OtlpExporter::new("datacenter-switch-01");

    exporter.record_counter("system.network.in_bytes", "Total ingress octets", "bytes", 102400000);
    exporter.record_counter("system.network.out_bytes", "Total egress octets", "bytes", 85400000);
    exporter.record_gauge("system.network.queue_depth", "Current egress buffer queue depth", "packets", 12.0);

    let span = OtlpSpan {
        trace_id: [0x01; 16],
        span_id: [0x02; 8],
        parent_span_id: None,
        name: "bgp.session.open".to_string(),
        start_time_ns: 1700000000000000,
        end_time_ns: 1700000000005000,
        attributes: vec![
            ("peer.as".to_string(), "65002".to_string()),
            ("peer.ip".to_string(), "192.168.1.10".to_string()),
        ],
    };
    exporter.record_span(span);

    let json = exporter.export_json();
    assert_eq!(json.contains("datacenter-switch-01"), true);
    assert_eq!(json.contains("system.network.in_bytes"), true);
    assert_eq!(json.contains("system.network.queue_depth"), true);
    assert_eq!(json.contains("bgp.session.open"), true);
    assert_eq!(json.contains("102400000"), true);
}

#[test]
fn test_otlp_metric_histogram_and_formatting() {
    let hist = OtlpMetric::Histogram {
        name: "net.tcp.rtt".to_string(),
        description: "TCP RTT latency distribution".to_string(),
        unit: "ms".to_string(),
        count: 50,
        sum: 125.5,
        buckets: vec![(1.0, 10), (5.0, 35), (10.0, 50)],
    };

    let mut exporter = OtlpExporter::new("service-mesh-gateway");
    exporter.metrics.push(hist);

    let json = exporter.export_json();
    assert_eq!(json.contains("net.tcp.rtt"), true);
    assert_eq!(json.contains("125.5"), true);
}
