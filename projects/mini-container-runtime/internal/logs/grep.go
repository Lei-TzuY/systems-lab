package logs

import (
	"fmt"
	"regexp"
	"strings"
)

// GrepLogLines filters log entries matching a regex pattern or substring query.
func GrepLogLines(logContent, pattern string) ([]string, error) {
	if pattern == "" {
		return strings.Split(strings.TrimRight(logContent, "\n"), "\n"), nil
	}

	re, err := regexp.Compile(pattern)
	if err != nil {
		return nil, fmt.Errorf("compile regex pattern: %w", err)
	}

	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	var result []string
	for _, line := range lines {
		if re.MatchString(line) {
			result = append(result, line)
		}
	}

	return result, nil
}
