// Package logs provides container log processing utilities.
// This file implements a container log field extraction filter that parses
// structured log lines and extracts specific named fields.

package logs

import (
	"fmt"
	"strings"
)

// FieldExtractor holds the configuration for extracting fields from log lines.
type FieldExtractor struct {
	// Fields lists the field names to extract from key=value pairs.
	Fields []string
	// Separator is the delimiter between key=value pairs (default: space).
	Separator string
	// KVDelimiter is the delimiter between key and value (default: "=").
	KVDelimiter string
}

// NewFieldExtractor creates a FieldExtractor for the given field names.
// If sep is empty, whitespace splitting is used. If kvDelim is empty, "=" is used.
func NewFieldExtractor(fields []string, sep, kvDelim string) *FieldExtractor {
	if kvDelim == "" {
		kvDelim = "="
	}
	return &FieldExtractor{
		Fields:      fields,
		Separator:   sep,
		KVDelimiter: kvDelim,
	}
}

// Extract parses a log line and returns only the requested key=value pairs.
// If none of the requested fields are found, an empty string is returned.
func (fe *FieldExtractor) Extract(line string) string {
	if len(fe.Fields) == 0 {
		return line
	}

	// Build a set of wanted field names for O(1) lookup.
	wanted := make(map[string]bool, len(fe.Fields))
	for _, f := range fe.Fields {
		wanted[f] = true
	}

	var parts []string
	if fe.Separator == "" {
		parts = strings.Fields(line)
	} else {
		parts = strings.Split(line, fe.Separator)
	}

	var extracted []string
	for _, part := range parts {
		part = strings.TrimSpace(part)
		idx := strings.Index(part, fe.KVDelimiter)
		if idx < 0 {
			continue
		}
		key := part[:idx]
		if wanted[key] {
			extracted = append(extracted, part)
		}
	}

	return strings.Join(extracted, " ")
}

// ExtractLines processes multiple log lines, extracting fields from each.
// Lines where no fields are found are omitted from the result.
func (fe *FieldExtractor) ExtractLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		out := fe.Extract(line)
		if out != "" {
			result = append(result, out)
		}
	}
	return result
}

// FormatExtracted returns a human-readable summary of the extraction.
func (fe *FieldExtractor) FormatExtracted(lines []string) string {
	extracted := fe.ExtractLines(lines)
	if len(extracted) == 0 {
		return fmt.Sprintf("no matching fields %v found", fe.Fields)
	}
	return strings.Join(extracted, "\n")
}
