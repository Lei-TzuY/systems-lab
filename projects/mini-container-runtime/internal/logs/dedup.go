package logs

import (
	"fmt"
)

// DeduplicateLogs merges consecutive identical log lines into summary lines.
func DeduplicateLogs(lines []string) []string {
	if len(lines) == 0 {
		return nil
	}

	var result []string
	prev := lines[0]
	repeatCount := 1

	for i := 1; i < len(lines); i++ {
		if lines[i] == prev {
			repeatCount++
		} else {
			result = append(result, prev)
			if repeatCount > 1 {
				result = append(result, fmt.Sprintf("[Last message repeated %d times]", repeatCount-1))
			}
			prev = lines[i]
			repeatCount = 1
		}
	}

	result = append(result, prev)
	if repeatCount > 1 {
		result = append(result, fmt.Sprintf("[Last message repeated %d times]", repeatCount-1))
	}

	return result
}
