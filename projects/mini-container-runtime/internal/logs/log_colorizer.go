// Package logs provides container log processing utilities.
// This file implements an ANSI terminal colorizer that applies
// color codes to log lines based on detected severity levels.

package logs

import (
	"strings"
)

const (
	ColorGray    = "\033[90m"
	ColorBoldRed = "\033[1;31m"
)

// ColorizeLogLine applies ANSI color to a log line based on severity keywords.
func ColorizeLogLine(line string) string {
	upper := strings.ToUpper(line)

	switch {
	case strings.Contains(upper, "PANIC") || strings.Contains(upper, "FATAL"):
		return ColorBoldRed + line + ColorReset
	case strings.Contains(upper, "ERROR") || strings.Contains(upper, "ERR"):
		return ColorRed + line + ColorReset
	case strings.Contains(upper, "WARN") || strings.Contains(upper, "WARNING"):
		return ColorYellow + line + ColorReset
	case strings.Contains(upper, "INFO"):
		return ColorGreen + line + ColorReset
	case strings.Contains(upper, "DEBUG"):
		return ColorGray + line + ColorReset
	case strings.Contains(upper, "TRACE"):
		return ColorCyan + line + ColorReset
	default:
		return line
	}
}

// ColorizeLogStream applies ANSI color codes to a slice of log lines.
func ColorizeLogStream(lines []string) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = ColorizeLogLine(line)
	}
	return out
}

// StripANSI removes all ANSI escape sequences from a string.
func StripANSI(s string) string {
	var sb strings.Builder
	i := 0
	for i < len(s) {
		if s[i] == '\033' && i+1 < len(s) && s[i+1] == '[' {
			// Skip to end of escape sequence (ending with a letter)
			j := i + 2
			for j < len(s) && !((s[j] >= 'A' && s[j] <= 'Z') || (s[j] >= 'a' && s[j] <= 'z')) {
				j++
			}
			if j < len(s) {
				j++ // skip the terminal letter
			}
			i = j
		} else {
			sb.WriteByte(s[i])
			i++
		}
	}
	return sb.String()
}
