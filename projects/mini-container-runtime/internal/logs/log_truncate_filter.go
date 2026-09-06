// Package logs provides container log processing utilities.
// This file implements a line truncation filter that caps individual log lines
// to a maximum byte length, appending an ellipsis marker for truncated content.

package logs

import (
	"fmt"
)

// TruncateFilter truncates log lines exceeding a maximum byte length.
type TruncateFilter struct {
	MaxBytes int
	Suffix   string
}

// NewTruncateFilter creates a TruncateFilter with configurable max byte length.
func NewTruncateFilter(maxBytes int, suffix string) *TruncateFilter {
	if maxBytes <= 0 {
		maxBytes = 4096
	}
	if suffix == "" {
		suffix = "...[truncated]"
	}
	return &TruncateFilter{
		MaxBytes: maxBytes,
		Suffix:   suffix,
	}
}

// Truncate returns the line truncated to MaxBytes with suffix appended if needed.
func (tf *TruncateFilter) Truncate(line string) string {
	if len(line) <= tf.MaxBytes {
		return line
	}
	cutoff := tf.MaxBytes - len(tf.Suffix)
	if cutoff < 0 {
		cutoff = 0
	}
	return line[:cutoff] + tf.Suffix
}

// FilterLines processes a slice of log lines, truncating any that exceed MaxBytes.
func (tf *TruncateFilter) FilterLines(lines []string) []string {
	result := make([]string, len(lines))
	for i, line := range lines {
		result[i] = tf.Truncate(line)
	}
	return result
}

// FormatTruncateStats returns a summary of how many lines were truncated.
func FormatTruncateStats(lines []string, maxBytes int) string {
	truncated := 0
	for _, line := range lines {
		if len(line) > maxBytes {
			truncated++
		}
	}
	return fmt.Sprintf("Total: %d lines, Truncated: %d (max %d bytes)", len(lines), truncated, maxBytes)
}
