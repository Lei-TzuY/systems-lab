package logs

import (
	"regexp"
)

// InvertGrepLogs returns lines that do NOT match the specified pattern.
func InvertGrepLogs(lines []string, pattern string) ([]string, error) {
	re, err := regexp.Compile(pattern)
	if err != nil {
		return nil, err
	}

	var result []string
	for _, line := range lines {
		if !re.MatchString(line) {
			result = append(result, line)
		}
	}

	return result, nil
}
