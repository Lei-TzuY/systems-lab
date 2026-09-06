//! OpenTelemetry OTLP Network Telemetry (gRPC Port 4317 / HTTP Port 4318).
//!
//! Cloud-native network metrics (throughput, drop rate, RTT latency, queue depth) and distributed trace spans exporter.

pub const OTLP_GRPC_PORT: u16 = 4317;
pub const OTLP_HTTP_PORT: u16 = 4318;

#[derive(Debug, Clone, PartialEq)]
pub enum OtlpMetric {
    Gauge {
        name: String,
        description: String,
        unit: String,
        value: f64,
    },
    Sum {
        name: String,
        description: String,
        unit: String,
        count: u64,
        is_monotonic: bool,
    },
    Histogram {
        name: String,
        description: String,
        unit: String,
        count: u64,
        sum: f64,
        buckets: Vec<(f64, u64)>, // (UpperBound, Count)
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtlpSpan {
    pub trace_id: [u8; 16],
    pub span_id: [u8; 8],
    pub parent_span_id: Option<[u8; 8]>,
    pub name: String,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct OtlpExporter {
    pub service_name: String,
    pub metrics: Vec<OtlpMetric>,
    pub spans: Vec<OtlpSpan>,
}

impl OtlpExporter {
    pub fn new(service_name: &str) -> Self {
        OtlpExporter {
            service_name: service_name.to_string(),
            metrics: Vec::new(),
            spans: Vec::new(),
        }
    }

    pub fn record_gauge(&mut self, name: &str, desc: &str, unit: &str, value: f64) {
        self.metrics.push(OtlpMetric::Gauge {
            name: name.to_string(),
            description: desc.to_string(),
            unit: unit.to_string(),
            value,
        });
    }

    pub fn record_counter(&mut self, name: &str, desc: &str, unit: &str, count: u64) {
        self.metrics.push(OtlpMetric::Sum {
            name: name.to_string(),
            description: desc.to_string(),
            unit: unit.to_string(),
            count,
            is_monotonic: true,
        });
    }

    pub fn record_span(&mut self, span: OtlpSpan) {
        self.spans.push(span);
    }

    pub fn export_json(&self) -> String {
        let mut json = String::new();
        json.push_str("{\n");
        json.push_str(&format!(
            "  \"resource\": {{ \"service.name\": \"{}\" }},\n",
            self.service_name
        ));
        json.push_str("  \"metrics\": [\n");

        for (i, m) in self.metrics.iter().enumerate() {
            let comma = if i + 1 < self.metrics.len() { "," } else { "" };
            match m {
                OtlpMetric::Gauge {
                    name,
                    description,
                    unit,
                    value,
                } => {
                    json.push_str(&format!(
                        "    {{ \"name\": \"{}\", \"description\": \"{}\", \"unit\": \"{}\", \"gauge\": {{ \"value\": {} }} }}{}\n",
                        name, description, unit, value, comma
                    ));
                }
                OtlpMetric::Sum {
                    name,
                    description,
                    unit,
                    count,
                    is_monotonic,
                } => {
                    json.push_str(&format!(
                        "    {{ \"name\": \"{}\", \"description\": \"{}\", \"unit\": \"{}\", \"sum\": {{ \"count\": {}, \"isMonotonic\": {} }} }}{}\n",
                        name, description, unit, count, is_monotonic, comma
                    ));
                }
                OtlpMetric::Histogram {
                    name,
                    description,
                    unit,
                    count,
                    sum,
                    ..
                } => {
                    json.push_str(&format!(
                        "    {{ \"name\": \"{}\", \"description\": \"{}\", \"unit\": \"{}\", \"histogram\": {{ \"count\": {}, \"sum\": {} }} }}{}\n",
                        name, description, unit, count, sum, comma
                    ));
                }
            }
        }
        json.push_str("  ],\n");

        json.push_str("  \"spans\": [\n");
        for (i, s) in self.spans.iter().enumerate() {
            let comma = if i + 1 < self.spans.len() { "," } else { "" };
            json.push_str(&format!(
                "    {{ \"name\": \"{}\", \"traceId\": \"{:02x?}\", \"spanId\": \"{:02x?}\", \"durationNs\": {} }}{}\n",
                s.name, s.trace_id, s.span_id, s.end_time_ns.saturating_sub(s.start_time_ns), comma
            ));
        }
        json.push_str("  ]\n");
        json.push('}');
        json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otlp_metrics_and_trace_export() {
        let mut exporter = OtlpExporter::new("toy-netstack-core");

        exporter.record_counter(
            "net.packets.transmitted",
            "Total egress packets",
            "packets",
            15420,
        );
        exporter.record_gauge("net.rtt.latency", "Current smoothed RTT", "ms", 1.45);

        let span = OtlpSpan {
            trace_id: [0x11; 16],
            span_id: [0x22; 8],
            parent_span_id: None,
            name: "tcp.handshake".to_string(),
            start_time_ns: 1700000000000000,
            end_time_ns: 1700000000001500,
            attributes: vec![("peer.ip".to_string(), "192.168.1.10".to_string())],
        };
        exporter.record_span(span);

        let json = exporter.export_json();
        assert!(json.contains("toy-netstack-core"));
        assert!(json.contains("net.packets.transmitted"));
        assert!(json.contains("15420"));
        assert!(json.contains("tcp.handshake"));

        assert_eq!(OTLP_GRPC_PORT, 4317);
        assert_eq!(OTLP_HTTP_PORT, 4318);
    }
}
