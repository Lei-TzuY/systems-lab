// Package logs provides container log processing utilities.
// This file implements a container log regular expression replacer that transforms
// or redacts matching patterns across log streams.

package logs

import (
	"fmt"
	"regexp"
)

// RegexReplacer holds the compiled regular expression pattern and replacement text.
type RegexReplacer struct {
	re          *regexp.Regexp
	replacement string
}

// NewRegexReplacer compiles a regex pattern with the given replacement string.
func NewRegexReplacer(pattern, replacement string) (*RegexReplacer, error) {
	if pattern == "" {
		return nil, fmt.Errorf("regex pattern cannot be empty")
	}
	re, err := regexp.Compile(pattern)
	if err != nil {
		return nil, fmt.Errorf("compile regex %q: %w", pattern, err)
	}
	return &RegexReplacer{
		re:          re,
		replacement: replacement,
	}, nil
}

// Replace executes the regex replacement on a single log line.
func (rr *RegexReplacer) Replace(line string) string {
	if rr.re == nil {
		return line
	}
	return rr.re.ReplaceAllString(line, rr.replacement)
}

// ReplaceLines executes the regex replacement over a slice of log lines.
func (rr *RegexReplacer) ReplaceLines(lines []string) []string {
	result := make([]string, len(lines))
	for i, line := range lines {
		result[i] = rr.Replace(line)
	}
	return result
}
