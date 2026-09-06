// Package logs provides container log processing utilities.
// This file implements a grep filter with before/after/around context lines (similar to grep -A -B -C).

package logs

import (
	"fmt"
	"regexp"
)

// LogGrepContextFilter searches for log lines matching a pattern with surrounding context lines.
type LogGrepContextFilter struct {
	pattern       *regexp.Regexp
	BeforeContext int
	AfterContext  int
	Separator     string
}

// NewLogGrepContextFilter creates a LogGrepContextFilter.
func NewLogGrepContextFilter(regexPattern string, before, after int, separator string) (*LogGrepContextFilter, error) {
	if regexPattern == "" {
		return nil, fmt.Errorf("grep regex pattern cannot be empty")
	}
	re, err := regexp.Compile(regexPattern)
	if err != nil {
		return nil, fmt.Errorf("compile regex %q: %w", regexPattern, err)
	}
	if before < 0 {
		before = 0
	}
	if after < 0 {
		after = 0
	}
	if separator == "" {
		separator = "--"
	}
	return &LogGrepContextFilter{
		pattern:       re,
		BeforeContext: before,
		AfterContext:  after,
		Separator:     separator,
	}, nil
}

// FilterWithContext processes a slice of log lines, returning matching lines
// along with surrounding context and separator markers.
func (f *LogGrepContextFilter) FilterWithContext(lines []string) []string {
	n := len(lines)
	if n == 0 {
		return nil
	}

	// Identify match indices
	matched := make([]bool, n)
	hasAny := false
	for i, line := range lines {
		if f.pattern.MatchString(line) {
			matched[i] = true
			hasAny = true
		}
	}
	if !hasAny {
		return nil
	}

	// Compute boolean mask of lines to include
	include := make([]bool, n)
	for i := 0; i < n; i++ {
		if matched[i] {
			start := i - f.BeforeContext
			if start < 0 {
				start = 0
			}
			end := i + f.AfterContext
			if end >= n {
				end = n - 1
			}
			for j := start; j <= end; j++ {
				include[j] = true
			}
		}
	}

	var result []string
	inBlock := false

	for i := 0; i < n; i++ {
		if include[i] {
			if !inBlock && len(result) > 0 && f.Separator != "" {
				result = append(result, f.Separator)
			}
			result = append(result, lines[i])
			inBlock = true
		} else {
			inBlock = false
		}
	}

	return result
}
