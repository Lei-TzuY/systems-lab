// Package logs provides container log processing utilities.
// This file implements a timestamp window filter that filters log lines
// between specified start (Since) and end (Until) time boundaries.

package logs

import (
	"fmt"
	"regexp"
	"strconv"
	"time"
)

// TimeRangeFilter filters log lines according to timestamp boundaries.
type TimeRangeFilter struct {
	Since *time.Time
	Until *time.Time
}

// NewTimeRangeFilter creates a new time boundary filter.
func NewTimeRangeFilter(since, until *time.Time) *TimeRangeFilter {
	return &TimeRangeFilter{
		Since: since,
		Until: until,
	}
}

var (
	// ISO8601 / RFC3339: 2026-08-20T12:34:56Z or 2026-08-20T12:34:56.789Z
	rfc3339Regex = regexp.MustCompile(`\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})\b`)
	// Standard space-separated date time: 2026-08-20 12:34:56
	dateTimeRegex = regexp.MustCompile(`\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\b`)
	// Key-value timestamp: ts=1724155200 or timestamp=...
	tsKVRegex = regexp.MustCompile(`\b(?:ts|timestamp|time)=["']?(\d{10}(?:\.\d+)?|\d{13})["']?`)
)

// ExtractTimestamp parses the timestamp found on a log line, if any.
func ExtractTimestamp(line string) (time.Time, bool) {
	// Try RFC3339 / ISO8601
	if m := rfc3339Regex.FindString(line); m != "" {
		if t, err := time.Parse(time.RFC3339Nano, m); err == nil {
			return t, true
		}
		if t, err := time.Parse(time.RFC3339, m); err == nil {
			return t, true
		}
	}

	// Try standard space-separated
	if m := dateTimeRegex.FindString(line); m != "" {
		if t, err := time.Parse("2006-01-02 15:04:05", m); err == nil {
			return t, true
		}
	}

	// Try ts= epoch seconds or millis
	if m := tsKVRegex.FindStringSubmatch(line); len(m) > 1 {
		raw := m[1]
		if len(raw) == 13 { // epoch millis
			if ms, err := strconv.ParseInt(raw, 10, 64); err == nil {
				return time.UnixMilli(ms), true
			}
		}
		if sec, err := strconv.ParseFloat(raw, 64); err == nil {
			s := int64(sec)
			ns := int64((sec - float64(s)) * 1e9)
			return time.Unix(s, ns), true
		}
	}

	return time.Time{}, false
}

// Match returns true if the line's timestamp falls within [Since, Until].
// If a line has no identifiable timestamp, it is included by default.
func (f *TimeRangeFilter) Match(line string) bool {
	t, found := ExtractTimestamp(line)
	if !found {
		return true // pass through lines with no timestamp
	}

	if f.Since != nil && t.Before(*f.Since) {
		return false
	}
	if f.Until != nil && t.After(*f.Until) {
		return false
	}
	return true
}

// FilterLines filters a slice of log lines based on the time range.
func (f *TimeRangeFilter) FilterLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		if f.Match(line) {
			result = append(result, line)
		}
	}
	return result
}

// FormatTimeRange returns a human-readable description of the active time window.
func (f *TimeRangeFilter) FormatTimeRange() string {
	var sinceStr, untilStr string
	if f.Since != nil {
		sinceStr = f.Since.Format(time.RFC3339)
	} else {
		sinceStr = "(beginning)"
	}
	if f.Until != nil {
		untilStr = f.Until.Format(time.RFC3339)
	} else {
		untilStr = "(now)"
	}
	return fmt.Sprintf("Time Range: %s -> %s", sinceStr, untilStr)
}
