package logs

import (
	"regexp"
)

// CountGrepMatches counts the total pattern match occurrences across log lines.
func CountGrepMatches(lines []string, pattern string) (int, error) {
	re, err := regexp.Compile(pattern)
	if err != nil {
		return 0, err
	}

	count := 0
	for _, line := range lines {
		matches := re.FindAllStringIndex(line, -1)
		count += len(matches)
	}

	return count, nil
}
