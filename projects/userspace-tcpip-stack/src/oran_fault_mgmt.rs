//! O-RAN WG4 Open Fronthaul Management Plane (M-Plane) Fault Management Engine.
//!
//! Compliant with O-RAN.WG4.MP.0 Section 10 & 12, ITU-T X.733, and `o-ran-fm.yang`.
//!
//! Provides the complete fault lifecycle engine:
//! - Standardized O-RAN Fault IDs (1001-1009) and Perceived Severity levels.
//! - Anti-flapping hysteresis with configurable raise/clear soak timers.
//! - Alarm storm suppression via cascading root-cause fault correlation.
//! - Active alarm filtering and acknowledgment RPCs.
//! - Circular audit trail history ring buffer.
//! - RFC 7950 XML and JSON `<alarm-notif>` serialization.

use std::collections::HashMap;

/// Standardized O-RAN Fault Identifiers per O-RAN.WG4.MP.0 §10.2 and `o-ran-fm.yang`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OranFaultId {
    /// Fault ID 1001: Loss of Signal (LOS) on SFP+ / optical transceiver interface.
    LossOfSignal,
    /// Fault ID 1002: Loss of Frame (LOF) on Ethernet / eCPRI framing layer.
    LossOfFrame,
    /// Fault ID 1003: PTP IEEE 1588 / SyncE Clock Loss of Lock or time error mask exceeded.
    PtpClockLossOfLock,
    /// Fault ID 1004: Thermal sensor threshold exceeded (over-temperature protection).
    OverTemperatureProtection,
    /// Fault ID 1005: Antenna Voltage Standing Wave Ratio (VSWR) threshold exceeded.
    VswrThresholdExceeded,
    /// Fault ID 1006: RF Power Amplifier (PA) transmit power degraded below nominal floor.
    TxPowerDegraded,
    /// Fault ID 1007: Receiver Low Noise Amplifier (LNA) input overdrive saturation.
    RxOverdrive,
    /// Fault ID 1008: eCPRI sequence numbering gap / packet loss threshold exceeded.
    EcPriSequenceFailure,
    /// Fault ID 1009: Fronthaul one-way transport delay jitter exceeds O-RU buffer window.
    DelayJitterExceeded,
    /// Vendor-specific extension fault identifier (2000..9999).
    VendorSpecific(u32),
}

impl OranFaultId {
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::LossOfSignal => 1001,
            Self::LossOfFrame => 1002,
            Self::PtpClockLossOfLock => 1003,
            Self::OverTemperatureProtection => 1004,
            Self::VswrThresholdExceeded => 1005,
            Self::TxPowerDegraded => 1006,
            Self::RxOverdrive => 1007,
            Self::EcPriSequenceFailure => 1008,
            Self::DelayJitterExceeded => 1009,
            Self::VendorSpecific(id) => *id,
        }
    }

    pub fn from_u32(val: u32) -> Self {
        match val {
            1001 => Self::LossOfSignal,
            1002 => Self::LossOfFrame,
            1003 => Self::PtpClockLossOfLock,
            1004 => Self::OverTemperatureProtection,
            1005 => Self::VswrThresholdExceeded,
            1006 => Self::TxPowerDegraded,
            1007 => Self::RxOverdrive,
            1008 => Self::EcPriSequenceFailure,
            1009 => Self::DelayJitterExceeded,
            other => Self::VendorSpecific(other),
        }
    }

    pub fn default_probable_cause(&self) -> &'static str {
        match self {
            Self::LossOfSignal => "lossOfSignal",
            Self::LossOfFrame => "lossOfFrame",
            Self::PtpClockLossOfLock => "synchronizationSourceBroken",
            Self::OverTemperatureProtection => "temperatureUnacceptable",
            Self::VswrThresholdExceeded => "antennaFailure",
            Self::TxPowerDegraded => "transmitterFailure",
            Self::RxOverdrive => "receiverFailure",
            Self::EcPriSequenceFailure => "transmissionError",
            Self::DelayJitterExceeded => "excessiveJitter",
            Self::VendorSpecific(_) => "vendorSpecificFault",
        }
    }
}

/// Alarm Perceived Severity per ITU-T X.733 and O-RAN `o-ran-fm.yang`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OranFaultSeverity {
    Cleared = 0,
    Warning = 1,
    Minor = 2,
    Major = 3,
    Critical = 4,
}

impl OranFaultSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cleared => "CLEARED",
            Self::Warning => "WARNING",
            Self::Minor => "MINOR",
            Self::Major => "MAJOR",
            Self::Critical => "CRITICAL",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CLEARED" => Some(Self::Cleared),
            "WARNING" => Some(Self::Warning),
            "MINOR" => Some(Self::Minor),
            "MAJOR" => Some(Self::Major),
            "CRITICAL" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Soak (anti-flapping) timer configuration in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoakConfig {
    /// Duration the fault condition must continuously persist before an alarm is raised.
    pub raise_soak_ms: u64,
    /// Duration the normal condition must continuously persist before an active alarm is cleared.
    pub clear_soak_ms: u64,
}

impl Default for SoakConfig {
    fn default() -> Self {
        Self {
            raise_soak_ms: 100, // 100 ms raise dampening
            clear_soak_ms: 500, // 500 ms clear dampening
        }
    }
}

/// Transient state machine for tracking a specific (FaultId, FaultSource) pair across soak timers.
#[derive(Debug, Clone, PartialEq)]
enum FaultConditionTracker {
    /// No fault present, no alarm raised.
    Normal,
    /// Fault condition detected; soaking until `raise_soak_ms` expires.
    SoakingRaise {
        start_ms: u64,
        severity: OranFaultSeverity,
        fault_text: String,
    },
    /// Fault confirmed and alarm raised in the active alarm table.
    Active {
        alarm_id: u32,
        severity: OranFaultSeverity,
        fault_text: String,
    },
    /// Normal condition detected on an active alarm; soaking until `clear_soak_ms` expires.
    SoakingClear {
        alarm_id: u32,
        severity: OranFaultSeverity,
        fault_text: String,
        clear_start_ms: u64,
    },
}

/// Active alarm record maintained in the O-RU active alarm table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OranActiveAlarm {
    pub alarm_id: u32,
    pub fault_id: OranFaultId,
    pub fault_source: String,
    pub severity: OranFaultSeverity,
    pub is_cleared: bool,
    pub fault_text: String,
    pub probable_cause: &'static str,
    pub event_time_epoch_ms: u64,
    pub is_acknowledged: bool,
    pub acknowledged_by: Option<String>,
    pub acknowledged_time_ms: Option<u64>,
    /// Set if this alarm is suppressed by a root-cause fault (e.g. LOS suppressing LOF).
    pub is_suppressed: bool,
    /// Pointer to the parent alarm ID that caused this alarm to be suppressed.
    pub root_cause_alarm_id: Option<u32>,
}

/// Alarm lifecycle event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEventType {
    Raise,
    Clear,
    SeverityUpdate,
    Acknowledge,
}

/// Complete alarm event notification emitted to the SMO / O-DU via NETCONF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OranFaultNotification {
    pub seq_num: u64,
    pub event_type: NotificationEventType,
    pub alarm: OranActiveAlarm,
}

impl OranFaultNotification {
    /// Format ISO 8601 UTC timestamp string from epoch milliseconds.
    fn format_iso8601(epoch_ms: u64) -> String {
        let total_secs = epoch_ms / 1000;
        let millis = epoch_ms % 1000;

        // Date calculation from Unix epoch (1970-01-01)
        let mut days = total_secs / 86400;
        let day_secs = total_secs % 86400;

        let hours = day_secs / 3600;
        let mins = (day_secs % 3600) / 60;
        let secs = day_secs % 60;

        // Simple leap year Gregorian calendar computation
        let mut year = 1970;
        loop {
            let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            let days_in_year = if leap { 366 } else { 365 };
            if days >= days_in_year {
                days -= days_in_year;
                year += 1;
            } else {
                break;
            }
        }

        let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_per_month = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut month = 1;
        for &dm in &days_per_month {
            if days >= dm {
                days -= dm;
                month += 1;
            } else {
                break;
            }
        }
        let day = days + 1;

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            year, month, day, hours, mins, secs, millis
        )
    }

    /// Serialize into RFC 7950 XML `<alarm-notif>` document.
    pub fn to_xml(&self) -> String {
        let ts = Self::format_iso8601(self.alarm.event_time_epoch_ms);
        let mut xml = String::new();
        xml.push_str("<notification xmlns=\"urn:ietf:params:xml:ns:netconf:notification:1.0\">\n");
        xml.push_str(&format!("  <eventTime>{}</eventTime>\n", ts));
        xml.push_str("  <alarm-notif xmlns=\"urn:o-ran:fm:1.0\">\n");
        xml.push_str(&format!(
            "    <fault-id>{}</fault-id>\n",
            self.alarm.fault_id.as_u32()
        ));
        xml.push_str(&format!(
            "    <fault-source>{}</fault-source>\n",
            self.alarm.fault_source
        ));
        xml.push_str(&format!(
            "    <fault-severity>{}</fault-severity>\n",
            self.alarm.severity.as_str()
        ));
        xml.push_str(&format!(
            "    <is-cleared>{}</is-cleared>\n",
            self.alarm.is_cleared
        ));
        xml.push_str(&format!(
            "    <fault-text>{}</fault-text>\n",
            self.alarm.fault_text
        ));
        xml.push_str(&format!("    <event-time>{}</event-time>\n", ts));
        if self.alarm.is_suppressed {
            if let Some(parent) = self.alarm.root_cause_alarm_id {
                xml.push_str(&format!(
                    "    <root-cause-alarm-id>{}</root-cause-alarm-id>\n",
                    parent
                ));
            }
        }
        xml.push_str("  </alarm-notif>\n");
        xml.push_str("</notification>");
        xml
    }

    /// Serialize into JSON payload for RESTCONF or streaming telemetry.
    pub fn to_json(&self) -> String {
        let ts = Self::format_iso8601(self.alarm.event_time_epoch_ms);
        let root_cause_str = match self.alarm.root_cause_alarm_id {
            Some(rc) => format!("{}", rc),
            None => "null".to_string(),
        };

        format!(
            "{{\"seq_num\":{},\"event_type\":\"{:?}\",\"fault_id\":{},\"fault_source\":\"{}\",\"severity\":\"{}\",\"is_cleared\":{},\"fault_text\":\"{}\",\"event_time\":\"{}\",\"is_suppressed\":{},\"root_cause_alarm_id\":{}}}",
            self.seq_num,
            self.event_type,
            self.alarm.fault_id.as_u32(),
            self.alarm.fault_source,
            self.alarm.severity.as_str(),
            self.alarm.is_cleared,
            self.alarm.fault_text,
            ts,
            self.alarm.is_suppressed,
            root_cause_str
        )
    }
}

/// Filter criteria for querying active alarms via `get-active-alarms` NETCONF RPC.
#[derive(Debug, Clone, Default)]
pub struct AlarmFilter {
    pub min_severity: Option<OranFaultSeverity>,
    pub source_prefix: Option<String>,
    pub acknowledged_only: Option<bool>,
    pub unacknowledged_only: Option<bool>,
    pub include_suppressed: bool,
}

/// O-RAN Open Fronthaul OAM Fault Management (FM) Engine.
#[derive(Debug)]
pub struct OranFaultManager {
    /// Active alarms indexed by unique alarm_id.
    pub active_alarms: HashMap<u32, OranActiveAlarm>,
    /// State tracking for flapping hysteresis per (FaultId, FaultSource).
    condition_trackers: HashMap<(OranFaultId, String), FaultConditionTracker>,
    /// Circular ring buffer for audit logging.
    history_log: Vec<OranFaultNotification>,
    max_history_entries: usize,
    next_alarm_id: u32,
    next_seq_num: u64,
    /// Per-fault soak configuration.
    soak_configs: HashMap<OranFaultId, SoakConfig>,
    default_soak_config: SoakConfig,
}

impl OranFaultManager {
    /// Create a new O-RAN Fault Management Engine.
    pub fn new(max_history_entries: usize) -> Self {
        Self {
            active_alarms: HashMap::new(),
            condition_trackers: HashMap::new(),
            history_log: Vec::with_capacity(max_history_entries.min(1000)),
            max_history_entries,
            next_alarm_id: 1,
            next_seq_num: 1,
            soak_configs: HashMap::new(),
            default_soak_config: SoakConfig::default(),
        }
    }

    /// Configure soak timers for a specific fault identifier.
    pub fn set_soak_config(&mut self, fault_id: OranFaultId, config: SoakConfig) {
        self.soak_configs.insert(fault_id, config);
    }

    /// Retrieve effective soak configuration for a fault ID.
    fn get_soak_config(&self, fault_id: OranFaultId) -> SoakConfig {
        *self
            .soak_configs
            .get(&fault_id)
            .unwrap_or(&self.default_soak_config)
    }

    /// Check if a fault should be suppressed by an active root-cause fault on the same source.
    ///
    /// O-RAN Hierarchy Correlation Rules:
    /// - `LossOfSignal` on `interface[name='X']` suppresses:
    ///   - `LossOfFrame` on `interface[name='X']`
    ///   - `PtpClockLossOfLock` on `interface[name='X']`
    ///   - `EcPriSequenceFailure` on `interface[name='X']`
    ///   - `DelayJitterExceeded` on `interface[name='X']`
    fn find_root_cause_parent(&self, fault_id: OranFaultId, source: &str) -> Option<u32> {
        match fault_id {
            OranFaultId::LossOfFrame
            | OranFaultId::PtpClockLossOfLock
            | OranFaultId::EcPriSequenceFailure
            | OranFaultId::DelayJitterExceeded => {
                // Look for an active LossOfSignal on the same source
                for alarm in self.active_alarms.values() {
                    if alarm.fault_id == OranFaultId::LossOfSignal
                        && alarm.fault_source == source
                        && !alarm.is_cleared
                    {
                        return Some(alarm.alarm_id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Record notification into history audit log (enforcing circular buffer capacity).
    fn log_notification(&mut self, notif: OranFaultNotification) {
        if self.history_log.len() >= self.max_history_entries {
            self.history_log.remove(0);
        }
        self.history_log.push(notif);
    }

    /// Report physical or protocol fault condition observation from a hardware/protocol sensor.
    ///
    /// Parameters:
    /// - `fault_id`: O-RAN standard fault ID
    /// - `source`: Managed Object path (e.g. `interface[name='eth0']`)
    /// - `is_present`: `true` if fault condition is detected; `false` if condition is normal/cleared
    /// - `severity`: Severity level if fault is present
    /// - `fault_text`: Human readable diagnostic explanation
    /// - `current_time_ms`: Current epoch timestamp in milliseconds
    ///
    /// Returns immediate `OranFaultNotification` if soak timer is zero or condition immediately transitioned.
    pub fn report_condition(
        &mut self,
        fault_id: OranFaultId,
        source: &str,
        is_present: bool,
        severity: OranFaultSeverity,
        fault_text: &str,
        current_time_ms: u64,
    ) -> Option<OranFaultNotification> {
        let key = (fault_id, source.to_string());
        let soak = self.get_soak_config(fault_id);
        let current_tracker = self
            .condition_trackers
            .get(&key)
            .cloned()
            .unwrap_or(FaultConditionTracker::Normal);

        if is_present {
            match current_tracker {
                FaultConditionTracker::Normal => {
                    if soak.raise_soak_ms == 0 {
                        // Immediate raise
                        let notif = self.raise_active_alarm(
                            fault_id,
                            source,
                            severity,
                            fault_text,
                            current_time_ms,
                        );
                        self.condition_trackers.insert(
                            key,
                            FaultConditionTracker::Active {
                                alarm_id: notif.alarm.alarm_id,
                                severity,
                                fault_text: fault_text.to_string(),
                            },
                        );
                        Some(notif)
                    } else {
                        // Start soaking raise
                        self.condition_trackers.insert(
                            key,
                            FaultConditionTracker::SoakingRaise {
                                start_ms: current_time_ms,
                                severity,
                                fault_text: fault_text.to_string(),
                            },
                        );
                        None
                    }
                }
                FaultConditionTracker::SoakingRaise {
                    start_ms,
                    severity: old_sev,
                    ..
                } => {
                    // Check if raise soak timer elapsed
                    if current_time_ms.saturating_sub(start_ms) >= soak.raise_soak_ms {
                        let eff_sev = if severity != OranFaultSeverity::Cleared {
                            severity
                        } else {
                            old_sev
                        };
                        let notif = self.raise_active_alarm(
                            fault_id,
                            source,
                            eff_sev,
                            fault_text,
                            current_time_ms,
                        );
                        self.condition_trackers.insert(
                            key,
                            FaultConditionTracker::Active {
                                alarm_id: notif.alarm.alarm_id,
                                severity: eff_sev,
                                fault_text: fault_text.to_string(),
                            },
                        );
                        Some(notif)
                    } else {
                        None
                    }
                }
                FaultConditionTracker::Active {
                    alarm_id,
                    severity: active_sev,
                    ..
                } => {
                    // Fault is already active. Check if severity changed!
                    if severity != active_sev && severity > OranFaultSeverity::Cleared {
                        if let Some(active) = self.active_alarms.get_mut(&alarm_id) {
                            active.severity = severity;
                            active.fault_text = fault_text.to_string();
                            active.event_time_epoch_ms = current_time_ms;

                            let seq = self.next_seq_num;
                            self.next_seq_num += 1;
                            let notif = OranFaultNotification {
                                seq_num: seq,
                                event_type: NotificationEventType::SeverityUpdate,
                                alarm: active.clone(),
                            };
                            self.log_notification(notif.clone());
                            self.condition_trackers.insert(
                                key,
                                FaultConditionTracker::Active {
                                    alarm_id,
                                    severity,
                                    fault_text: fault_text.to_string(),
                                },
                            );
                            return Some(notif);
                        }
                    }
                    None
                }
                FaultConditionTracker::SoakingClear {
                    alarm_id,
                    severity: active_sev,
                    fault_text: prev_text,
                    ..
                } => {
                    // Fault re-occurred while soaking clear -> abort clear soaking, stay active!
                    self.condition_trackers.insert(
                        key,
                        FaultConditionTracker::Active {
                            alarm_id,
                            severity: active_sev,
                            fault_text: prev_text,
                        },
                    );
                    None
                }
            }
        } else {
            // is_present is false -> condition is normal
            match current_tracker {
                FaultConditionTracker::Normal => None,
                FaultConditionTracker::SoakingRaise { .. } => {
                    // Transient glitch ceased before raise soak expired -> cancel raise!
                    self.condition_trackers
                        .insert(key, FaultConditionTracker::Normal);
                    None
                }
                FaultConditionTracker::Active {
                    alarm_id,
                    severity: active_sev,
                    fault_text: prev_text,
                } => {
                    if soak.clear_soak_ms == 0 {
                        // Immediate clear
                        let notif = self.clear_active_alarm(alarm_id, current_time_ms);
                        self.condition_trackers
                            .insert(key, FaultConditionTracker::Normal);
                        notif
                    } else {
                        // Start soaking clear
                        self.condition_trackers.insert(
                            key,
                            FaultConditionTracker::SoakingClear {
                                alarm_id,
                                severity: active_sev,
                                fault_text: prev_text,
                                clear_start_ms: current_time_ms,
                            },
                        );
                        None
                    }
                }
                FaultConditionTracker::SoakingClear {
                    alarm_id,
                    clear_start_ms,
                    ..
                } => {
                    // Check if clear soak timer elapsed
                    if current_time_ms.saturating_sub(clear_start_ms) >= soak.clear_soak_ms {
                        let notif = self.clear_active_alarm(alarm_id, current_time_ms);
                        self.condition_trackers
                            .insert(key, FaultConditionTracker::Normal);
                        notif
                    } else {
                        None
                    }
                }
            }
        }
    }

    /// Step time forward and evaluate all in-flight soak timers.
    ///
    /// Returns any notifications generated by timers expiring.
    pub fn step_time(&mut self, current_time_ms: u64) -> Vec<OranFaultNotification> {
        let mut notifications = Vec::new();
        let keys: Vec<(OranFaultId, String)> = self.condition_trackers.keys().cloned().collect();

        for key in keys {
            let (fault_id, source) = &key;
            let soak = self.get_soak_config(*fault_id);
            let tracker = match self.condition_trackers.get(&key) {
                Some(t) => t.clone(),
                None => continue,
            };

            match tracker {
                FaultConditionTracker::SoakingRaise {
                    start_ms,
                    severity,
                    fault_text,
                } => {
                    if current_time_ms.saturating_sub(start_ms) >= soak.raise_soak_ms {
                        let notif = self.raise_active_alarm(
                            *fault_id,
                            source,
                            severity,
                            &fault_text,
                            current_time_ms,
                        );
                        self.condition_trackers.insert(
                            key.clone(),
                            FaultConditionTracker::Active {
                                alarm_id: notif.alarm.alarm_id,
                                severity,
                                fault_text,
                            },
                        );
                        notifications.push(notif);
                    }
                }
                FaultConditionTracker::SoakingClear {
                    alarm_id,
                    clear_start_ms,
                    ..
                } => {
                    if current_time_ms.saturating_sub(clear_start_ms) >= soak.clear_soak_ms {
                        if let Some(notif) = self.clear_active_alarm(alarm_id, current_time_ms) {
                            self.condition_trackers
                                .insert(key.clone(), FaultConditionTracker::Normal);
                            notifications.push(notif);
                        }
                    }
                }
                _ => {}
            }
        }

        notifications
    }

    /// Internal: instantiate and record an active alarm.
    fn raise_active_alarm(
        &mut self,
        fault_id: OranFaultId,
        source: &str,
        severity: OranFaultSeverity,
        fault_text: &str,
        event_time_epoch_ms: u64,
    ) -> OranFaultNotification {
        let alarm_id = self.next_alarm_id;
        self.next_alarm_id += 1;

        let root_cause_id = self.find_root_cause_parent(fault_id, source);
        let is_suppressed = root_cause_id.is_some();

        let alarm = OranActiveAlarm {
            alarm_id,
            fault_id,
            fault_source: source.to_string(),
            severity,
            is_cleared: false,
            fault_text: fault_text.to_string(),
            probable_cause: fault_id.default_probable_cause(),
            event_time_epoch_ms,
            is_acknowledged: false,
            acknowledged_by: None,
            acknowledged_time_ms: None,
            is_suppressed,
            root_cause_alarm_id: root_cause_id,
        };

        self.active_alarms.insert(alarm_id, alarm.clone());

        let seq_num = self.next_seq_num;
        self.next_seq_num += 1;

        let notif = OranFaultNotification {
            seq_num,
            event_type: NotificationEventType::Raise,
            alarm,
        };
        self.log_notification(notif.clone());
        notif
    }

    /// Internal: clear an active alarm and remove from active alarm table.
    fn clear_active_alarm(
        &mut self,
        alarm_id: u32,
        event_time_epoch_ms: u64,
    ) -> Option<OranFaultNotification> {
        if let Some(mut active) = self.active_alarms.remove(&alarm_id) {
            active.is_cleared = true;
            active.severity = OranFaultSeverity::Cleared;
            active.event_time_epoch_ms = event_time_epoch_ms;

            let seq_num = self.next_seq_num;
            self.next_seq_num += 1;

            let notif = OranFaultNotification {
                seq_num,
                event_type: NotificationEventType::Clear,
                alarm: active,
            };
            self.log_notification(notif.clone());
            Some(notif)
        } else {
            None
        }
    }

    /// Acknowledge an active alarm via `acknowledge-alarm` NETCONF RPC.
    pub fn acknowledge_alarm(
        &mut self,
        alarm_id: u32,
        user: &str,
        ack_time_ms: u64,
    ) -> Result<OranFaultNotification, &'static str> {
        let active = self
            .active_alarms
            .get_mut(&alarm_id)
            .ok_or("Alarm ID not found in active alarm table")?;

        active.is_acknowledged = true;
        active.acknowledged_by = Some(user.to_string());
        active.acknowledged_time_ms = Some(ack_time_ms);

        let seq_num = self.next_seq_num;
        self.next_seq_num += 1;

        let notif = OranFaultNotification {
            seq_num,
            event_type: NotificationEventType::Acknowledge,
            alarm: active.clone(),
        };
        self.log_notification(notif.clone());
        Ok(notif)
    }

    /// Query active alarms with multi-criterion filtering (`get-active-alarms` RPC).
    pub fn get_active_alarms(&self, filter: &AlarmFilter) -> Vec<OranActiveAlarm> {
        let mut results = Vec::new();

        for alarm in self.active_alarms.values() {
            // Severity filter
            if let Some(min_sev) = filter.min_severity {
                if alarm.severity < min_sev {
                    continue;
                }
            }

            // Source prefix filter
            if let Some(ref prefix) = filter.source_prefix {
                if !alarm.fault_source.starts_with(prefix) {
                    continue;
                }
            }

            // Acknowledged filter
            if let Some(true) = filter.acknowledged_only {
                if !alarm.is_acknowledged {
                    continue;
                }
            }
            if let Some(true) = filter.unacknowledged_only {
                if alarm.is_acknowledged {
                    continue;
                }
            }

            // Suppressed filter
            if alarm.is_suppressed && !filter.include_suppressed {
                continue;
            }

            results.push(alarm.clone());
        }

        // Sort by alarm_id for deterministic order
        results.sort_by_key(|a| a.alarm_id);
        results
    }

    /// Retrieve historical audit log.
    pub fn get_history(&self) -> &[OranFaultNotification] {
        &self.history_log
    }
}
