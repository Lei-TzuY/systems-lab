package logs

import (
	"strings"
	"time"
)

// FilterLogsSince filters log entries by timestamp within a specified duration window.
func FilterLogsSince(logContent string, since time.Duration) []string {
	if since <= 0 {
		return strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	}

	cutoff := time.Now().Add(-since)
	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var result []string

	for _, line := range lines {
		parts := strings.SplitN(line, " ", 2)
		if len(parts) >= 1 {
			if t, err := time.Parse(time.RFC3339, parts[0]); err == nil {
				if t.After(cutoff) {
					result = append(result, line)
				}
				continue
			}
		}
		result = append(result, line)
	}

	return result
}
