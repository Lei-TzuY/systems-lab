package logs

import (
	"strings"
)

// TailLogLines extracts the last tailCount lines from a log content string.
func TailLogLines(logContent string, tailCount int) []string {
	if tailCount <= 0 {
		return nil
	}

	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	if len(lines) <= tailCount {
		return lines
	}

	return lines[len(lines)-tailCount:]
}
