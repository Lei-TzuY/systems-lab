// Package logs provides container log processing utilities.
// This file implements a positional column/field extraction filter for
// delimited container log lines (CSV, TSV, space, or custom delimiters).

package logs

import (
	"fmt"
	"strings"
)

// ColumnExtractor extracts specific 1-indexed positional columns from delimited log lines.
type ColumnExtractor struct {
	// Columns is the list of 1-based column indices to extract.
	Columns []int
	// Delimiter is the field separator (empty string denotes whitespace splitting).
	Delimiter string
	// OutputSeparator is the delimiter used to join extracted columns (default: space).
	OutputSeparator string
}

// NewColumnExtractor creates a new ColumnExtractor.
func NewColumnExtractor(columns []int, delimiter, outSep string) *ColumnExtractor {
	if outSep == "" {
		outSep = " "
	}
	return &ColumnExtractor{
		Columns:         columns,
		Delimiter:       delimiter,
		OutputSeparator: outSep,
	}
}

// ExtractLine extracts the specified columns from a single log line.
// If none of the requested column indices exist on the line, an empty string is returned.
func (ce *ColumnExtractor) ExtractLine(line string) string {
	if len(ce.Columns) == 0 {
		return line
	}

	var fields []string
	if ce.Delimiter == "" {
		fields = strings.Fields(line)
	} else {
		fields = strings.Split(line, ce.Delimiter)
	}

	var extracted []string
	for _, colIdx := range ce.Columns {
		// 1-based index to 0-based
		idx := colIdx - 1
		if idx >= 0 && idx < len(fields) {
			extracted = append(extracted, strings.TrimSpace(fields[idx]))
		}
	}

	if len(extracted) == 0 {
		return ""
	}

	return strings.Join(extracted, ce.OutputSeparator)
}

// ExtractLines processes a slice of log lines and returns extracted columns for each.
func (ce *ColumnExtractor) ExtractLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		out := ce.ExtractLine(line)
		if out != "" {
			result = append(result, out)
		}
	}
	return result
}

// FormatColumnsSummary returns a summary string of extracted columns.
func (ce *ColumnExtractor) FormatColumnsSummary(totalLines, extractedLines int) string {
	return fmt.Sprintf("Extracted columns %v from %d/%d lines",
		ce.Columns, extractedLines, totalLines)
}
