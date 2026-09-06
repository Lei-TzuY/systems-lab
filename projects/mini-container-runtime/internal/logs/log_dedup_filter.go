// Package logs provides container log processing utilities.
// This file implements a deduplication filter that suppresses consecutive
// identical log lines and replaces them with a "repeated N times" summary.

package logs

import (
	"fmt"
)

// DeduplicateResult represents a deduplicated log entry.
type DeduplicateResult struct {
	Line  string
	Count int
}

// DeduplicateLines groups consecutive identical lines, returning unique entries
// with repetition counts.
func DeduplicateLines(lines []string) []DeduplicateResult {
	if len(lines) == 0 {
		return nil
	}

	var results []DeduplicateResult
	current := lines[0]
	count := 1

	for i := 1; i < len(lines); i++ {
		if lines[i] == current {
			count++
		} else {
			results = append(results, DeduplicateResult{Line: current, Count: count})
			current = lines[i]
			count = 1
		}
	}
	results = append(results, DeduplicateResult{Line: current, Count: count})

	return results
}

// FormatDeduplicated converts deduplicated results into human-readable strings.
// Lines that repeated more than once get a suffix like " [repeated 5 times]".
func FormatDeduplicated(results []DeduplicateResult) []string {
	out := make([]string, len(results))
	for i, r := range results {
		if r.Count > 1 {
			out[i] = fmt.Sprintf("%s [repeated %d times]", r.Line, r.Count)
		} else {
			out[i] = r.Line
		}
	}
	return out
}
