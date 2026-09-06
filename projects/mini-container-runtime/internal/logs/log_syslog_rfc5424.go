// Package logs provides container log processing utilities.
// This file implements an RFC 5424 Syslog message formatter
// for forwarding container log streams to centralized syslog aggregators.

package logs

import (
	"fmt"
	"strings"
	"time"
)

// SyslogRFC5424Record holds structured fields for an RFC 5424 syslog message.
type SyslogRFC5424Record struct {
	Facility int       // Default 1 (user-level)
	Severity int       // 0=Emergency, 1=Alert, 2=Crit, 3=Err, 4=Warn, 5=Notice, 6=Info, 7=Debug
	Time     time.Time
	Hostname string
	AppName  string
	ProcID   string
	MsgID    string
	Message  string
}

// SyslogRFC5424Formatter formats container log lines into standard RFC 5424 strings.
type SyslogRFC5424Formatter struct {
	Hostname string
	AppName  string
	ProcID   string
}

// NewSyslogRFC5424Formatter creates a SyslogRFC5424Formatter.
func NewSyslogRFC5424Formatter(hostname, appName, procID string) *SyslogRFC5424Formatter {
	if hostname == "" {
		hostname = "minidocker"
	}
	if appName == "" {
		appName = "container"
	}
	if procID == "" {
		procID = "-"
	}
	return &SyslogRFC5424Formatter{
		Hostname: hostname,
		AppName:  appName,
		ProcID:   procID,
	}
}

// InferSeverity parses severity keywords from log text (defaults to 6: Informational).
func InferSeverity(line string) int {
	upper := strings.ToUpper(line)
	if strings.Contains(upper, "PANIC") || strings.Contains(upper, "EMERG") {
		return 0 // Emergency
	}
	if strings.Contains(upper, "ALERT") {
		return 1 // Alert
	}
	if strings.Contains(upper, "CRIT") || strings.Contains(upper, "FATAL") {
		return 2 // Critical
	}
	if strings.Contains(upper, "ERR") || strings.Contains(upper, "ERROR") {
		return 3 // Error
	}
	if strings.Contains(upper, "WARN") {
		return 4 // Warning
	}
	if strings.Contains(upper, "NOTICE") {
		return 5 // Notice
	}
	if strings.Contains(upper, "DEBUG") || strings.Contains(upper, "TRACE") {
		return 7 // Debug
	}
	return 6 // Informational
}

// FormatLine converts a single log line into an RFC 5424 syslog string.
func (f *SyslogRFC5424Formatter) FormatLine(line string, now time.Time) string {
	sev := InferSeverity(line)
	pri := (1 * 8) + sev // Facility=1 (user-level)

	ts, ok := ExtractTimestamp(line)
	if !ok {
		ts = now
	}
	tsStr := ts.Format(time.RFC3339)

	// Format: <PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID [STRUCTURED-DATA] MSG
	return fmt.Sprintf("<%d>1 %s %s %s %s - - %s",
		pri, tsStr, f.Hostname, f.AppName, f.ProcID, strings.TrimSpace(line))
}

// FormatLines converts a slice of log lines into RFC 5424 syslog messages.
func (f *SyslogRFC5424Formatter) FormatLines(lines []string, now time.Time) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = f.FormatLine(line, now)
	}
	return out
}
