package logs

import (
	"regexp"
)

var maskPatterns = []*regexp.Regexp{
	regexp.MustCompile(`(?i)(api[_-]?key|secret|password|passwd|token|bearer)\s*[:=]\s*["']?([^\s"']+)["']?`),
}

// MaskSensitiveLogs redacts sensitive credential patterns with [REDACTED].
func MaskSensitiveLogs(input string) string {
	res := input
	for _, pattern := range maskPatterns {
		res = pattern.ReplaceAllString(res, "$1=[REDACTED]")
	}
	return res
}
