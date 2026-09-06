// Package logs provides container log processing utilities.
// This file implements a multi-line log event boundary assembler that merges
// fragmented lines (e.g., stack traces, exceptions) into logical records
// matching a start-of-message regular expression.

package logs

import (
	"fmt"
	"regexp"
	"strings"
)

// RegexLineSplitter re-assembles multi-line log events based on a line prefix regex.
type RegexLineSplitter struct {
	startPattern *regexp.Regexp
	Joiner       string
}

// NewRegexLineSplitter creates a new multi-line assembler.
// Lines matching startRegexPattern begin a new log record; non-matching lines are appended to the preceding record.
func NewRegexLineSplitter(startRegexPattern, joiner string) (*RegexLineSplitter, error) {
	if startRegexPattern == "" {
		return nil, fmt.Errorf("start regex pattern cannot be empty")
	}
	re, err := regexp.Compile(startRegexPattern)
	if err != nil {
		return nil, fmt.Errorf("compile multiline start regex %q: %w", startRegexPattern, err)
	}
	if joiner == "" {
		joiner = "\n"
	}
	return &RegexLineSplitter{
		startPattern: re,
		Joiner:       joiner,
	}, nil
}

// AssembleRecords groups raw stream lines into consolidated multi-line records.
func (s *RegexLineSplitter) AssembleRecords(rawLines []string) []string {
	if len(rawLines) == 0 {
		return nil
	}

	var records []string
	var currentRecord []string

	for _, line := range rawLines {
		if s.startPattern.MatchString(line) {
			if len(currentRecord) > 0 {
				records = append(records, strings.Join(currentRecord, s.Joiner))
				currentRecord = nil
			}
		}
		currentRecord = append(currentRecord, line)
	}

	if len(currentRecord) > 0 {
		records = append(records, strings.Join(currentRecord, s.Joiner))
	}

	return records
}
