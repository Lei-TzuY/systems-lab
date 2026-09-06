//! Syslog Protocol & Event Telemetry (RFC 5424 / RFC 3164).
//!
//! Standard network logging and telemetry event collector over UDP port 514.

use std::fmt;

pub const SYSLOG_UDP_PORT: u16 = 514;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyslogFacility {
    Kern = 0,
    User = 1,
    Mail = 2,
    Daemon = 3,
    Auth = 4,
    Syslog = 5,
    Local0 = 16,
    Local1 = 17,
    Local7 = 23,
}

impl SyslogFacility {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(SyslogFacility::Kern),
            1 => Some(SyslogFacility::User),
            2 => Some(SyslogFacility::Mail),
            3 => Some(SyslogFacility::Daemon),
            4 => Some(SyslogFacility::Auth),
            5 => Some(SyslogFacility::Syslog),
            16 => Some(SyslogFacility::Local0),
            17 => Some(SyslogFacility::Local1),
            23 => Some(SyslogFacility::Local7),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyslogSeverity {
    Emergency = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Informational = 6,
    Debug = 7,
}

impl SyslogSeverity {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(SyslogSeverity::Emergency),
            1 => Some(SyslogSeverity::Alert),
            2 => Some(SyslogSeverity::Critical),
            3 => Some(SyslogSeverity::Error),
            4 => Some(SyslogSeverity::Warning),
            5 => Some(SyslogSeverity::Notice),
            6 => Some(SyslogSeverity::Informational),
            7 => Some(SyslogSeverity::Debug),
            _ => None,
        }
    }
}

impl fmt::Display for SyslogSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyslogSeverity::Emergency => write!(f, "EMERG"),
            SyslogSeverity::Alert => write!(f, "ALERT"),
            SyslogSeverity::Critical => write!(f, "CRIT"),
            SyslogSeverity::Error => write!(f, "ERROR"),
            SyslogSeverity::Warning => write!(f, "WARN"),
            SyslogSeverity::Notice => write!(f, "NOTICE"),
            SyslogSeverity::Informational => write!(f, "INFO"),
            SyslogSeverity::Debug => write!(f, "DEBUG"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyslogMessage {
    pub facility: SyslogFacility,
    pub severity: SyslogSeverity,
    pub hostname: String,
    pub app_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyslogError {
    InvalidFormat,
    InvalidPriority(u8),
}

impl fmt::Display for SyslogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyslogError::InvalidFormat => write!(f, "Invalid Syslog framing: expected <PRI>..."),
            SyslogError::InvalidPriority(p) => write!(f, "Invalid Syslog PRI value: {}", p),
        }
    }
}

impl std::error::Error for SyslogError {}

impl SyslogMessage {
    pub fn new(
        facility: SyslogFacility,
        severity: SyslogSeverity,
        hostname: &str,
        app: &str,
        msg: &str,
    ) -> Self {
        SyslogMessage {
            facility,
            severity,
            hostname: hostname.to_string(),
            app_name: app.to_string(),
            message: msg.to_string(),
        }
    }

    pub fn pri_val(&self) -> u8 {
        (self.facility as u8) * 8 + (self.severity as u8)
    }

    pub fn format_rfc5424(&self) -> String {
        format!(
            "<{}>1 - {} {} - - - {}",
            self.pri_val(),
            self.hostname,
            self.app_name,
            self.message
        )
    }

    pub fn parse_rfc5424(s: &str) -> Result<Self, SyslogError> {
        if !s.starts_with('<') {
            return Err(SyslogError::InvalidFormat);
        }

        let end_pri = s.find('>').ok_or(SyslogError::InvalidFormat)?;
        let pri_str = &s[1..end_pri];
        let pri = pri_str
            .parse::<u8>()
            .map_err(|_| SyslogError::InvalidFormat)?;

        let fac_u8 = pri / 8;
        let sev_u8 = pri % 8;

        let facility = SyslogFacility::from_u8(fac_u8).unwrap_or(SyslogFacility::Local0);
        let severity = SyslogSeverity::from_u8(sev_u8).ok_or(SyslogError::InvalidPriority(pri))?;

        let remainder = s[end_pri + 1..].trim();
        let parts: Vec<&str> = remainder.split_whitespace().collect();

        let (hostname, app_name, msg) = if parts.len() >= 8 {
            let host = parts[2];
            let app = parts[3];
            let msg_idx = s.find(parts[7]).unwrap_or(s.len());
            (host, app, s[msg_idx..].trim())
        } else if parts.len() >= 4 {
            let host = parts[2];
            let app = parts[3];
            (host, app, remainder)
        } else {
            ("-", "-", remainder)
        };

        Ok(SyslogMessage {
            facility,
            severity,
            hostname: hostname.to_string(),
            app_name: app_name.to_string(),
            message: msg.to_string(),
        })
    }
}

/// Syslog In-Memory Event Ring Buffer
pub struct SyslogCollector {
    pub logs: Vec<SyslogMessage>,
    pub max_capacity: usize,
}

impl Default for SyslogCollector {
    fn default() -> Self {
        Self::new(100)
    }
}

impl SyslogCollector {
    pub fn new(capacity: usize) -> Self {
        let mut collector = SyslogCollector {
            logs: Vec::new(),
            max_capacity: capacity,
        };
        // Add initial system events
        collector.record(SyslogMessage::new(
            SyslogFacility::Daemon,
            SyslogSeverity::Informational,
            "toy-stack-host",
            "netstack",
            "Network interface eth0 link UP, MTU 1500",
        ));
        collector.record(SyslogMessage::new(
            SyslogFacility::Auth,
            SyslogSeverity::Notice,
            "toy-stack-host",
            "sshd",
            "Accepted publickey for root from 192.168.1.100 port 54321",
        ));
        collector
    }

    pub fn record(&mut self, msg: SyslogMessage) {
        if self.logs.len() >= self.max_capacity {
            self.logs.remove(0);
        }
        self.logs.push(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syslog_prival_and_formatting() {
        let msg = SyslogMessage::new(
            SyslogFacility::Auth,
            SyslogSeverity::Warning,
            "core-router",
            "bgpd",
            "BGP neighbor 192.168.1.10 Down: hold timer expired",
        );

        // Facility Auth (4) * 8 + Severity Warning (4) = 36
        assert_eq!(msg.pri_val(), 36);

        let formatted = msg.format_rfc5424();
        assert!(formatted.starts_with("<36>1 - core-router bgpd"));

        let parsed = SyslogMessage::parse_rfc5424(&formatted).unwrap();
        assert_eq!(parsed.facility, SyslogFacility::Auth);
        assert_eq!(parsed.severity, SyslogSeverity::Warning);
        assert_eq!(parsed.hostname, "core-router");
        assert_eq!(parsed.app_name, "bgpd");
    }

    #[test]
    fn test_syslog_collector_ring_buffer() {
        let mut collector = SyslogCollector::new(2);
        let m1 = SyslogMessage::new(
            SyslogFacility::Local0,
            SyslogSeverity::Informational,
            "h",
            "app",
            "1",
        );
        let m2 = SyslogMessage::new(
            SyslogFacility::Local0,
            SyslogSeverity::Informational,
            "h",
            "app",
            "2",
        );
        let m3 = SyslogMessage::new(
            SyslogFacility::Local0,
            SyslogSeverity::Informational,
            "h",
            "app",
            "3",
        );

        collector.record(m1);
        collector.record(m2);
        collector.record(m3);

        assert_eq!(collector.logs.len(), 2);
        assert_eq!(collector.logs[0].message, "2");
        assert_eq!(collector.logs[1].message, "3");
    }
}
