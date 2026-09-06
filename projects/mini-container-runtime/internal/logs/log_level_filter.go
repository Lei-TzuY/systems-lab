// Package logs provides container log processing utilities.
// This file implements a severity level filter that filters log streams based on
// minimum log level thresholds (DEBUG, INFO, WARN, ERROR, FATAL).

package logs

import (
	"encoding/json"
	"regexp"
	"strings"
)

// LogSeverity represents the standardized log severity level rank.
type LogSeverity int

const (
	SeverityDebug LogSeverity = iota
	SeverityInfo
	SeverityWarn
	SeverityError
	SeverityFatal
	SeverityUnknown
)

// ParseSeverity parses a severity level string into LogSeverity.
func ParseSeverity(s string) LogSeverity {
	switch strings.ToLower(strings.TrimSpace(s)) {
	case "debug", "trace", "dbg":
		return SeverityDebug
	case "info", "notice", "inf":
		return SeverityInfo
	case "warn", "warning", "wrn":
		return SeverityWarn
	case "error", "err", "er":
		return SeverityError
	case "fatal", "crit", "critical", "panic", "fatal error":
		return SeverityFatal
	default:
		return SeverityUnknown
	}
}

// LogLevelFilter filters log lines according to a minimum severity threshold.
type LogLevelFilter struct {
	MinLevel LogSeverity
}

// NewLogLevelFilter creates a new LogLevelFilter with the specified minimum severity.
func NewLogLevelFilter(minLevel string) *LogLevelFilter {
	return &LogLevelFilter{
		MinLevel: ParseSeverity(minLevel),
	}
}

var (
	bracketLevelRegex = regexp.MustCompile(`(?i)\[(debug|info|warn|warning|error|fatal|panic|crit|critical)\]`)
	kvLevelRegex      = regexp.MustCompile(`(?i)\b(?:level|lvl|severity)=["']?([a-zA-Z]+)["']?`)
	prefixLevelRegex  = regexp.MustCompile(`(?i)^(?:debug|info|warn|warning|error|fatal|crit):\s*`)
)

// DetectSeverity extracts the severity level from a log line.
func (f *LogLevelFilter) DetectSeverity(line string) LogSeverity {
	trimmed := strings.TrimSpace(line)

	// Check JSON format: {"level": "info", ...}
	if strings.HasPrefix(trimmed, "{") && strings.HasSuffix(trimmed, "}") {
		var raw map[string]interface{}
		if err := json.Unmarshal([]byte(trimmed), &raw); err == nil {
			for _, key := range []string{"level", "lvl", "severity", "log_level"} {
				if val, ok := raw[key]; ok {
					if strVal, isStr := val.(string); isStr {
						sev := ParseSeverity(strVal)
						if sev != SeverityUnknown {
							return sev
						}
					}
				}
			}
		}
	}

	// Check [LEVEL] bracket format
	if m := bracketLevelRegex.FindStringSubmatch(line); len(m) > 1 {
		return ParseSeverity(m[1])
	}

	// Check level=... key-value format
	if m := kvLevelRegex.FindStringSubmatch(line); len(m) > 1 {
		return ParseSeverity(m[1])
	}

	// Check LEVEL: prefix format
	if m := prefixLevelRegex.FindString(line); m != "" {
		clean := strings.TrimSuffix(strings.TrimSpace(m), ":")
		return ParseSeverity(clean)
	}

	return SeverityUnknown
}

// Match returns true if the log line's detected severity meets or exceeds MinLevel.
// Lines with undetected severity are passed through if MinLevel is SeverityDebug or SeverityInfo.
func (f *LogLevelFilter) Match(line string) bool {
	sev := f.DetectSeverity(line)
	if sev == SeverityUnknown {
		return f.MinLevel <= SeverityInfo
	}
	return sev >= f.MinLevel
}

// FilterLines filters a slice of log lines and returns only those meeting the threshold.
func (f *LogLevelFilter) FilterLines(lines []string) []string {
	var matched []string
	for _, line := range lines {
		if f.Match(line) {
			matched = append(matched, line)
		}
	}
	return matched
}
