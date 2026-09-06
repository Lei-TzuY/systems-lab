package logs

import (
	"strings"
)

// AggregateMultilineLogs merges indented stack trace lines into single log events.
func AggregateMultilineLogs(logContent string) []string {
	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var events []string
	var current string

	for _, line := range lines {
		if line == "" {
			continue
		}
		if strings.HasPrefix(line, " ") || strings.HasPrefix(line, "\t") || strings.HasPrefix(line, "at ") || strings.HasPrefix(line, "Caused by:") {
			if current != "" {
				current += "\n" + line
			} else {
				current = line
			}
		} else {
			if current != "" {
				events = append(events, current)
			}
			current = line
		}
	}

	if current != "" {
		events = append(events, current)
	}

	return events
}
