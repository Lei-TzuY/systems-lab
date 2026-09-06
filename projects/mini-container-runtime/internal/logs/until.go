package logs

import (
	"strings"
	"time"
)

// FilterLogsUntil filters log entries, keeping entries emitted prior to an upper duration cutoff.
func FilterLogsUntil(logContent string, until time.Duration) []string {
	if until <= 0 {
		return strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	}

	cutoff := time.Now().Add(-until)
	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var result []string

	for _, line := range lines {
		parts := strings.SplitN(line, " ", 2)
		if len(parts) >= 1 {
			if t, err := time.Parse(time.RFC3339, parts[0]); err == nil {
				if t.Before(cutoff) {
					result = append(result, line)
				}
				continue
			}
		}
		result = append(result, line)
	}

	return result
}
