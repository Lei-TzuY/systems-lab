// Package logs provides container log processing utilities.
// This file implements an ANSI color highlighter for container log streams that
// decorates severity keywords and custom matched terms for terminal viewing.

package logs

import (
	"fmt"
	"regexp"
)

// ANSI color escape codes
const (
	ColorReset   = "\033[0m"
	ColorRed     = "\033[31m"
	ColorGreen   = "\033[32m"
	ColorYellow  = "\033[33m"
	ColorBlue    = "\033[34m"
	ColorMagenta = "\033[35m"
	ColorCyan    = "\033[36m"
	ColorBold    = "\033[1m"
)

// ColorHighlighter applies ANSI color formatting to log lines based on keywords and patterns.
type ColorHighlighter struct {
	HighlightSeverities bool
	customPatterns      []customHighlight
}

type customHighlight struct {
	re    *regexp.Regexp
	color string
}

// NewColorHighlighter creates a new log colorizer.
func NewColorHighlighter(highlightSeverities bool) *ColorHighlighter {
	return &ColorHighlighter{
		HighlightSeverities: highlightSeverities,
	}
}

// AddKeyword registers a custom regex pattern to be highlighted with the specified ANSI color.
func (ch *ColorHighlighter) AddKeyword(pattern string, color string) error {
	re, err := regexp.Compile("(?i)" + pattern)
	if err != nil {
		return fmt.Errorf("compile highlight pattern %q: %w", pattern, err)
	}
	ch.customPatterns = append(ch.customPatterns, customHighlight{
		re:    re,
		color: color,
	})
	return nil
}

var (
	errorWordRegex = regexp.MustCompile(`(?i)\b(error|fatal|panic|critical|crit|err)\b`)
	warnWordRegex  = regexp.MustCompile(`(?i)\b(warn|warning)\b`)
	infoWordRegex  = regexp.MustCompile(`(?i)\b(info|notice)\b`)
	debugWordRegex = regexp.MustCompile(`(?i)\b(debug|trace)\b`)
)

// HighlightLine decorates the line with ANSI color codes.
func (ch *ColorHighlighter) HighlightLine(line string) string {
	result := line

	if ch.HighlightSeverities {
		result = errorWordRegex.ReplaceAllStringFunc(result, func(m string) string {
			return ColorBold + ColorRed + m + ColorReset
		})
		result = warnWordRegex.ReplaceAllStringFunc(result, func(m string) string {
			return ColorYellow + m + ColorReset
		})
		result = infoWordRegex.ReplaceAllStringFunc(result, func(m string) string {
			return ColorGreen + m + ColorReset
		})
		result = debugWordRegex.ReplaceAllStringFunc(result, func(m string) string {
			return ColorCyan + m + ColorReset
		})
	}

	for _, cp := range ch.customPatterns {
		result = cp.re.ReplaceAllStringFunc(result, func(m string) string {
			return cp.color + m + ColorReset
		})
	}

	return result
}

// HighlightLines applies color formatting to a slice of log lines.
func (ch *ColorHighlighter) HighlightLines(lines []string) []string {
	result := make([]string, len(lines))
	for i, line := range lines {
		result[i] = ch.HighlightLine(line)
	}
	return result
}
