// Package logs provides container log processing utilities.
// This file implements an RFC 4180 compliant CSV/TSV table exporter
// for structured container log streams.

package logs

import (
	"bytes"
	"encoding/csv"
	"fmt"
	"strings"
	"time"
)

// CSVLogFormatter formats log lines into RFC 4180 CSV/TSV records.
type CSVLogFormatter struct {
	Comma       rune
	IncludeHead bool
	Headers     []string
}

// NewCSVLogFormatter creates a new CSV/TSV log formatter.
func NewCSVLogFormatter(comma rune, includeHead bool, headers []string) *CSVLogFormatter {
	if comma == 0 {
		comma = ','
	}
	if len(headers) == 0 {
		headers = []string{"timestamp", "severity", "message"}
	}
	return &CSVLogFormatter{
		Comma:       comma,
		IncludeHead: includeHead,
		Headers:     headers,
	}
}

// FormatLine parses a raw log line into a CSV row (timestamp, level, message).
func (f *CSVLogFormatter) FormatLine(line string) []string {
	t, hasTime := ExtractTimestamp(line)
	var timeStr string
	if hasTime {
		timeStr = t.Format(time.RFC3339)
	}

	filter := NewLogLevelFilter("debug")
	sev := filter.DetectSeverity(line)
	var sevStr string
	switch sev {
	case SeverityDebug:
		sevStr = "DEBUG"
	case SeverityInfo:
		sevStr = "INFO"
	case SeverityWarn:
		sevStr = "WARN"
	case SeverityError:
		sevStr = "ERROR"
	case SeverityFatal:
		sevStr = "FATAL"
	default:
		sevStr = "UNKNOWN"
	}

	// Clean up message portion
	msg := strings.TrimSpace(line)

	return []string{timeStr, sevStr, msg}
}

// FormatLines converts a slice of log lines into a formatted CSV string.
func (f *CSVLogFormatter) FormatLines(lines []string) (string, error) {
	var buf bytes.Buffer
	writer := csv.NewWriter(&buf)
	writer.Comma = f.Comma

	if f.IncludeHead && len(f.Headers) > 0 {
		if err := writer.Write(f.Headers); err != nil {
			return "", fmt.Errorf("write csv header: %w", err)
		}
	}

	for _, line := range lines {
		record := f.FormatLine(line)
		if err := writer.Write(record); err != nil {
			return "", fmt.Errorf("write csv record: %w", err)
		}
	}

	writer.Flush()
	if err := writer.Error(); err != nil {
		return "", fmt.Errorf("flush csv: %w", err)
	}

	return buf.String(), nil
}
