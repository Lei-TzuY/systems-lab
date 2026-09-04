//! O-RAN WG4 Open Fronthaul Management Plane (M-Plane) Performance Management (PM) Engine.
//!
//! Compliant with O-RAN.WG4.MP.0 Section 9 ("Performance Management"),
//! `o-ran-performance-management.yang`, and 3GPP TS 32.435 (PM XML format).
//!
//! Provides the complete performance management and measurement lifecycle:
//! - Physical Transceiver (SFP+/QSFP28) optical power, voltage, bias current, and temperature tracking.
//! - C/U-Plane packet timing window analysis (early, on-time, late, and corrupt packet counts).
//! - eCPRI/Ethernet transport metrics (bytes, frames, buffer drops, sequence gaps, delay).
//! - Physical Resource Block (PRB) usage and peak utilization statistics.
//! - Configurable periodic measurement jobs with circular historical interval bins.
//! - Dual-tier Threshold Crossing Alerts (TCA) with anti-flapping hysteresis.
//! - RFC 7950 XML and RFC 7951 JSON `<performance-measurement-data>` serialization.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Data Models & Measurements
// ---------------------------------------------------------------------------

/// Optical Transceiver measurement data (SFP28 / QSFP28).
#[derive(Debug, Clone, PartialEq)]
pub struct TransceiverMeasurement {
    pub port_id: u8,
    /// Optical transmit power in dBm (e.g. -2.5 dBm).
    pub tx_power_dbm: f32,
    /// Optical receive power in dBm (e.g. -4.8 dBm).
    pub rx_power_dbm: f32,
    /// Supply voltage in Volts (e.g. 3.300 V).
    pub supply_voltage_v: f32,
    /// Laser bias current in mA (e.g. 35.5 mA).
    pub tx_bias_current_ma: f32,
    /// Module operating temperature in Celsius (e.g. 42.5 °C).
    pub temperature_c: f32,
}

/// C/U-Plane Symbol Arrival Window timing metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RxWindowMeasurement {
    pub ru_port_id: u8,
    /// Packets arriving prior to the early boundary (buffered).
    pub early_packets: u64,
    /// Packets arriving in the nominal processing window.
    pub on_time_packets: u64,
    /// Packets arriving after the late boundary (dropped/missed deadline).
    pub late_packets: u64,
    /// Header/payload checksum or framing corruptions.
    pub corrupt_packets: u64,
}

/// eCPRI and Fronthaul Ethernet transport performance metrics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EcpriTransportMeasurement {
    pub interface_id: u8,
    pub tx_packets: u64,
    pub rx_packets: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub buffer_overflow_drops: u64,
    pub sequence_errors: u64,
    /// One-way transport latency in nanoseconds.
    pub one_way_delay_ns: u32,
}

/// Physical Resource Block (PRB) utilization per carrier/antenna port.
#[derive(Debug, Clone, PartialEq)]
pub struct TxPrbMeasurement {
    pub carrier_id: u8,
    pub antenna_port_id: u8,
    /// Average PRB usage percentage (0.00% .. 100.00%).
    pub prb_usage_percent: f32,
    /// Peak number of PRBs scheduled in any single slot/symbol.
    pub peak_prb_usage: u16,
    /// Total number of active transmission symbols in the interval.
    pub total_active_symbols: u64,
}

// ---------------------------------------------------------------------------
// Threshold Crossing Alerts (TCA)
// ---------------------------------------------------------------------------

/// Direction of threshold crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcaDirection {
    /// Metric value rises above threshold (e.g. temperature, packet drops).
    Rising,
    /// Metric value falls below threshold (e.g. optical Rx power, voltage).
    Falling,
}

/// Severity level of a Threshold Crossing Alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcaSeverity {
    Warning,
    Alarm,
}

/// Configuration for a metric Threshold Crossing Alert.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdCrossingConfig {
    pub metric_name: String,
    pub warning_threshold: Option<f32>,
    pub alarm_threshold: Option<f32>,
    /// Anti-flapping hysteresis offset.
    pub hysteresis: f32,
    pub direction: TcaDirection,
}

/// Emitted Threshold Crossing Alert event.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdCrossingAlert {
    pub metric_name: String,
    pub current_value: f32,
    pub threshold_value: f32,
    pub direction: TcaDirection,
    pub severity: TcaSeverity,
    pub cleared: bool,
    pub timestamp_seconds: u64,
}

// ---------------------------------------------------------------------------
// Measurement Intervals & Jobs
// ---------------------------------------------------------------------------

/// Record containing aggregated performance metrics for a completed interval.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementIntervalRecord {
    pub interval_id: u64,
    pub start_timestamp_seconds: u64,
    pub duration_seconds: u32,
    pub transceiver_records: Vec<TransceiverMeasurement>,
    pub rx_window_records: Vec<RxWindowMeasurement>,
    pub ecpri_records: Vec<EcpriTransportMeasurement>,
    pub prb_records: Vec<TxPrbMeasurement>,
}

/// Configured O-RAN PM Job collecting metrics on a defined schedule.
#[derive(Debug, Clone)]
pub struct MeasurementJob {
    pub job_id: String,
    pub interval_seconds: u32,
    pub max_history_intervals: usize,
    pub active: bool,
    pub current_interval_seconds_elapsed: u32,
    pub interval_counter: u64,
    pub history: Vec<MeasurementIntervalRecord>,
}

impl MeasurementJob {
    pub fn new(job_id: impl Into<String>, interval_seconds: u32, max_history: usize) -> Self {
        Self {
            job_id: job_id.into(),
            interval_seconds,
            max_history_intervals: max_history,
            active: true,
            current_interval_seconds_elapsed: 0,
            interval_counter: 0,
            history: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// O-RAN Performance Management Engine
// ---------------------------------------------------------------------------

/// O-RAN WG4 Management Plane Performance Management Engine.
#[derive(Debug, Default)]
pub struct OranPmEngine {
    pub jobs: HashMap<String, MeasurementJob>,
    pub tca_configs: Vec<ThresholdCrossingConfig>,
    pub active_tcas: HashMap<String, ThresholdCrossingAlert>,

    // Current interval working accumulators
    pub current_transceivers: HashMap<u8, TransceiverMeasurement>,
    pub current_rx_windows: HashMap<u8, RxWindowMeasurement>,
    pub current_ecpri: HashMap<u8, EcpriTransportMeasurement>,
    pub current_prb: HashMap<(u8, u8), TxPrbMeasurement>,
}

impl OranPmEngine {
    /// Create a new O-RAN PM Engine instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a measurement job.
    pub fn add_job(&mut self, job: MeasurementJob) {
        self.jobs.insert(job.job_id.clone(), job);
    }

    /// Add a Threshold Crossing Alert configuration.
    pub fn add_tca_config(&mut self, config: ThresholdCrossingConfig) {
        self.tca_configs.push(config);
    }

    /// Ingest an optical transceiver measurement and check TCAs.
    pub fn record_transceiver(
        &mut self,
        meas: TransceiverMeasurement,
        timestamp_seconds: u64,
    ) -> Vec<ThresholdCrossingAlert> {
        let mut alerts = Vec::new();

        // Check TCAs for Temperature
        let temp_metric = format!("transceiver.{}.temperature", meas.port_id);
        alerts.extend(self.evaluate_metric_tca(
            &temp_metric,
            meas.temperature_c,
            timestamp_seconds,
        ));

        // Check TCAs for Rx Optical Power
        let rx_pow_metric = format!("transceiver.{}.rx_power", meas.port_id);
        alerts.extend(self.evaluate_metric_tca(
            &rx_pow_metric,
            meas.rx_power_dbm,
            timestamp_seconds,
        ));

        // Store into current interval accumulator
        self.current_transceivers.insert(meas.port_id, meas);

        alerts
    }

    /// Ingest C/U-Plane Rx Window packet timing metrics.
    pub fn record_rx_window(
        &mut self,
        meas: RxWindowMeasurement,
        timestamp_seconds: u64,
    ) -> Vec<ThresholdCrossingAlert> {
        let mut alerts = Vec::new();

        let late_metric = format!("rx_window.{}.late_packets", meas.ru_port_id);
        alerts.extend(self.evaluate_metric_tca(
            &late_metric,
            meas.late_packets as f32,
            timestamp_seconds,
        ));

        self.current_rx_windows.insert(meas.ru_port_id, meas);
        alerts
    }

    /// Ingest eCPRI transport performance metrics.
    pub fn record_ecpri(
        &mut self,
        meas: EcpriTransportMeasurement,
        timestamp_seconds: u64,
    ) -> Vec<ThresholdCrossingAlert> {
        let mut alerts = Vec::new();

        let drop_metric = format!("ecpri.{}.drops", meas.interface_id);
        alerts.extend(self.evaluate_metric_tca(
            &drop_metric,
            meas.buffer_overflow_drops as f32,
            timestamp_seconds,
        ));

        let delay_metric = format!("ecpri.{}.delay_ns", meas.interface_id);
        alerts.extend(self.evaluate_metric_tca(
            &delay_metric,
            meas.one_way_delay_ns as f32,
            timestamp_seconds,
        ));

        self.current_ecpri.insert(meas.interface_id, meas);
        alerts
    }

    /// Ingest PRB utilization metrics.
    pub fn record_tx_prb(
        &mut self,
        meas: TxPrbMeasurement,
        timestamp_seconds: u64,
    ) -> Vec<ThresholdCrossingAlert> {
        let mut alerts = Vec::new();

        let prb_metric = format!("prb.{}.{}.usage", meas.carrier_id, meas.antenna_port_id);
        alerts.extend(self.evaluate_metric_tca(
            &prb_metric,
            meas.prb_usage_percent,
            timestamp_seconds,
        ));

        self.current_prb
            .insert((meas.carrier_id, meas.antenna_port_id), meas);
        alerts
    }

    /// Internal evaluation of metric value against configured TCA thresholds.
    fn evaluate_metric_tca(
        &mut self,
        metric_name: &str,
        value: f32,
        timestamp: u64,
    ) -> Vec<ThresholdCrossingAlert> {
        let mut emitted = Vec::new();

        let matching_cfg = self
            .tca_configs
            .iter()
            .find(|c| c.metric_name == metric_name)
            .cloned();

        let cfg = match matching_cfg {
            Some(c) => c,
            None => return emitted,
        };

        let active_opt = self.active_tcas.get(metric_name).cloned();

        let is_active = active_opt.as_ref().map_or(false, |a| !a.cleared);
        let active_severity = active_opt.as_ref().map(|a| a.severity);

        match cfg.direction {
            TcaDirection::Rising => {
                let alarm_thresh = cfg.alarm_threshold.unwrap_or(f32::MAX);
                let warn_thresh = cfg.warning_threshold.unwrap_or(f32::MAX);

                if is_active && active_severity == Some(TcaSeverity::Alarm) {
                    let clear_thresh = alarm_thresh - cfg.hysteresis;
                    if value <= clear_thresh {
                        let clear_alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: clear_thresh,
                            direction: TcaDirection::Rising,
                            severity: TcaSeverity::Alarm,
                            cleared: true,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), clear_alert.clone());
                        emitted.push(clear_alert);
                    }
                } else if is_active && active_severity == Some(TcaSeverity::Warning) {
                    if value >= alarm_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: alarm_thresh,
                            direction: TcaDirection::Rising,
                            severity: TcaSeverity::Alarm,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    } else {
                        let clear_thresh = warn_thresh - cfg.hysteresis;
                        if value <= clear_thresh {
                            let clear_alert = ThresholdCrossingAlert {
                                metric_name: metric_name.to_string(),
                                current_value: value,
                                threshold_value: clear_thresh,
                                direction: TcaDirection::Rising,
                                severity: TcaSeverity::Warning,
                                cleared: true,
                                timestamp_seconds: timestamp,
                            };
                            self.active_tcas
                                .insert(metric_name.to_string(), clear_alert.clone());
                            emitted.push(clear_alert);
                        }
                    }
                } else {
                    // No active alert (or previous alert was cleared)
                    if value >= alarm_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: alarm_thresh,
                            direction: TcaDirection::Rising,
                            severity: TcaSeverity::Alarm,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    } else if value >= warn_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: warn_thresh,
                            direction: TcaDirection::Rising,
                            severity: TcaSeverity::Warning,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    }
                }
            }
            TcaDirection::Falling => {
                let alarm_thresh = cfg.alarm_threshold.unwrap_or(f32::MIN);
                let warn_thresh = cfg.warning_threshold.unwrap_or(f32::MIN);

                if is_active && active_severity == Some(TcaSeverity::Alarm) {
                    let clear_thresh = alarm_thresh + cfg.hysteresis;
                    if value >= clear_thresh {
                        let clear_alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: clear_thresh,
                            direction: TcaDirection::Falling,
                            severity: TcaSeverity::Alarm,
                            cleared: true,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), clear_alert.clone());
                        emitted.push(clear_alert);
                    }
                } else if is_active && active_severity == Some(TcaSeverity::Warning) {
                    if value <= alarm_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: alarm_thresh,
                            direction: TcaDirection::Falling,
                            severity: TcaSeverity::Alarm,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    } else {
                        let clear_thresh = warn_thresh + cfg.hysteresis;
                        if value >= clear_thresh {
                            let clear_alert = ThresholdCrossingAlert {
                                metric_name: metric_name.to_string(),
                                current_value: value,
                                threshold_value: clear_thresh,
                                direction: TcaDirection::Falling,
                                severity: TcaSeverity::Warning,
                                cleared: true,
                                timestamp_seconds: timestamp,
                            };
                            self.active_tcas
                                .insert(metric_name.to_string(), clear_alert.clone());
                            emitted.push(clear_alert);
                        }
                    }
                } else {
                    // No active alert (or previous alert was cleared)
                    if value <= alarm_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: alarm_thresh,
                            direction: TcaDirection::Falling,
                            severity: TcaSeverity::Alarm,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    } else if value <= warn_thresh {
                        let alert = ThresholdCrossingAlert {
                            metric_name: metric_name.to_string(),
                            current_value: value,
                            threshold_value: warn_thresh,
                            direction: TcaDirection::Falling,
                            severity: TcaSeverity::Warning,
                            cleared: false,
                            timestamp_seconds: timestamp,
                        };
                        self.active_tcas
                            .insert(metric_name.to_string(), alert.clone());
                        emitted.push(alert);
                    }
                }
            }
        }

        emitted
    }

    /// Advance time by 1 second: advances active jobs and handles interval binning.
    ///
    /// Returns any newly completed interval records keyed by job ID.
    pub fn tick_second(
        &mut self,
        current_timestamp_seconds: u64,
    ) -> Vec<(String, MeasurementIntervalRecord)> {
        let mut completed_intervals = Vec::new();

        // Snapshot current metrics
        let transceivers: Vec<_> = self.current_transceivers.values().cloned().collect();
        let rx_windows: Vec<_> = self.current_rx_windows.values().cloned().collect();
        let ecpri: Vec<_> = self.current_ecpri.values().cloned().collect();
        let prb: Vec<_> = self.current_prb.values().cloned().collect();

        for (job_id, job) in &mut self.jobs {
            if !job.active {
                continue;
            }

            job.current_interval_seconds_elapsed += 1;

            if job.current_interval_seconds_elapsed >= job.interval_seconds {
                // Interval Complete!
                job.interval_counter += 1;
                let start_time =
                    current_timestamp_seconds.saturating_sub(job.interval_seconds as u64);

                let record = MeasurementIntervalRecord {
                    interval_id: job.interval_counter,
                    start_timestamp_seconds: start_time,
                    duration_seconds: job.interval_seconds,
                    transceiver_records: transceivers.clone(),
                    rx_window_records: rx_windows.clone(),
                    ecpri_records: ecpri.clone(),
                    prb_records: prb.clone(),
                };

                // Add to job circular history
                if job.history.len() >= job.max_history_intervals {
                    job.history.remove(0);
                }
                job.history.push(record.clone());

                job.current_interval_seconds_elapsed = 0;
                completed_intervals.push((job_id.clone(), record));
            }
        }

        completed_intervals
    }

    // -----------------------------------------------------------------------
    // Standards Serialization (RFC 7950 XML, RFC 7951 JSON, TS 32.435)
    // -----------------------------------------------------------------------

    /// Formats a Measurement Interval Record into RFC 7950 XML `<performance-measurement-data>`.
    pub fn format_xml_pm_data(record: &MeasurementIntervalRecord) -> String {
        let mut xml = String::new();
        xml.push_str("<performance-measurement-data xmlns=\"urn:o-ran:pm:1.0\">\n");
        xml.push_str(&format!(
            "  <interval-id>{}</interval-id>\n",
            record.interval_id
        ));
        xml.push_str(&format!(
            "  <start-time>{}</start-time>\n",
            record.start_timestamp_seconds
        ));
        xml.push_str(&format!(
            "  <duration>{}</duration>\n",
            record.duration_seconds
        ));

        // Transceiver metrics
        if !record.transceiver_records.is_empty() {
            xml.push_str("  <transceiver-measurements>\n");
            for t in &record.transceiver_records {
                xml.push_str(&format!("    <port id=\"{}\">\n", t.port_id));
                xml.push_str(&format!(
                    "      <tx-power-dbm>{:.2}</tx-power-dbm>\n",
                    t.tx_power_dbm
                ));
                xml.push_str(&format!(
                    "      <rx-power-dbm>{:.2}</rx-power-dbm>\n",
                    t.rx_power_dbm
                ));
                xml.push_str(&format!(
                    "      <voltage-v>{:.3}</voltage-v>\n",
                    t.supply_voltage_v
                ));
                xml.push_str(&format!(
                    "      <temperature-c>{:.1}</temperature-c>\n",
                    t.temperature_c
                ));
                xml.push_str("    </port>\n");
            }
            xml.push_str("  </transceiver-measurements>\n");
        }

        // Rx Window metrics
        if !record.rx_window_records.is_empty() {
            xml.push_str("  <rx-window-measurements>\n");
            for w in &record.rx_window_records {
                xml.push_str(&format!("    <ru-port id=\"{}\">\n", w.ru_port_id));
                xml.push_str(&format!("      <on-time>{}</on-time>\n", w.on_time_packets));
                xml.push_str(&format!("      <early>{}</early>\n", w.early_packets));
                xml.push_str(&format!("      <late>{}</late>\n", w.late_packets));
                xml.push_str(&format!("      <corrupt>{}</corrupt>\n", w.corrupt_packets));
                xml.push_str("    </ru-port>\n");
            }
            xml.push_str("  </rx-window-measurements>\n");
        }

        xml.push_str("</performance-measurement-data>");
        xml
    }

    /// Formats a Measurement Interval Record into RFC 7951 JSON format.
    pub fn format_json_pm_data(record: &MeasurementIntervalRecord) -> String {
        let mut json = String::new();
        json.push_str("{\n");
        json.push_str(&format!("  \"interval_id\": {},\n", record.interval_id));
        json.push_str(&format!(
            "  \"start_time\": {},\n",
            record.start_timestamp_seconds
        ));
        json.push_str(&format!("  \"duration\": {},\n", record.duration_seconds));
        json.push_str("  \"transceivers\": [\n");
        for (i, t) in record.transceiver_records.iter().enumerate() {
            json.push_str(&format!(
                "    {{\"port\": {}, \"tx_power_dbm\": {:.2}, \"rx_power_dbm\": {:.2}, \"temp_c\": {:.1}}}{}\n",
                t.port_id,
                t.tx_power_dbm,
                t.rx_power_dbm,
                t.temperature_c,
                if i + 1 < record.transceiver_records.len() { "," } else { "" }
            ));
        }
        json.push_str("  ],\n");
        json.push_str("  \"rx_windows\": [\n");
        for (i, w) in record.rx_window_records.iter().enumerate() {
            json.push_str(&format!(
                "    {{\"ru_port\": {}, \"on_time\": {}, \"late\": {}}}{}\n",
                w.ru_port_id,
                w.on_time_packets,
                w.late_packets,
                if i + 1 < record.rx_window_records.len() {
                    ","
                } else {
                    ""
                }
            ));
        }
        json.push_str("  ]\n");
        json.push_str("}");
        json
    }

    /// Formats a Threshold Crossing Alert notification into RFC 7950 XML.
    pub fn format_xml_tca(alert: &ThresholdCrossingAlert) -> String {
        format!(
            "<threshold-crossing-alert xmlns=\"urn:o-ran:pm:1.0\">\n  <metric>{}</metric>\n  <current-value>{:.2}</current-value>\n  <threshold>{:.2}</threshold>\n  <direction>{:?}</direction>\n  <severity>{:?}</severity>\n  <cleared>{}</cleared>\n  <timestamp>{}</timestamp>\n</threshold-crossing-alert>",
            alert.metric_name,
            alert.current_value,
            alert.threshold_value,
            alert.direction,
            alert.severity,
            alert.cleared,
            alert.timestamp_seconds,
        )
    }
}
