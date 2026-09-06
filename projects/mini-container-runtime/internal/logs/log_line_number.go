// Package logs provides container log processing utilities.
// This file implements a line number annotator that prepends sequential
// line numbers to each log line for easier reference and debugging.

package logs

import (
	"fmt"
	"strings"
)

// LineNumberAnnotator prepends line numbers to log lines.
type LineNumberAnnotator struct {
	StartNum  int
	Separator string
	PadWidth  int
}

// NewLineNumberAnnotator creates a LineNumberAnnotator.
func NewLineNumberAnnotator(startNum int, separator string, padWidth int) *LineNumberAnnotator {
	if startNum < 0 {
		startNum = 1
	}
	if separator == "" {
		separator = ": "
	}
	if padWidth <= 0 {
		padWidth = 4
	}
	return &LineNumberAnnotator{
		StartNum:  startNum,
		Separator: separator,
		PadWidth:  padWidth,
	}
}

// Annotate prepends a formatted line number to a single line.
func (a *LineNumberAnnotator) Annotate(lineNum int, line string) string {
	format := fmt.Sprintf("%%%dd%s%%s", a.PadWidth, a.Separator)
	return fmt.Sprintf(format, lineNum, line)
}

// AnnotateLines prepends sequential line numbers to all lines.
func (a *LineNumberAnnotator) AnnotateLines(lines []string) []string {
	result := make([]string, len(lines))
	for i, line := range lines {
		result[i] = a.Annotate(a.StartNum+i, line)
	}
	return result
}

// FormatLineCount returns a summary of the total number of lines.
func FormatLineCount(lines []string) string {
	nonEmpty := 0
	for _, l := range lines {
		if strings.TrimSpace(l) != "" {
			nonEmpty++
		}
	}
	return fmt.Sprintf("Total: %d lines (%d non-empty)", len(lines), nonEmpty)
}
