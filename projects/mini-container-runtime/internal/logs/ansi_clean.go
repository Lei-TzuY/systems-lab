package logs

import (
	"regexp"
)

var ansiRegex = regexp.MustCompile(`\x1b\[[0-9;]*[a-zA-Z]`)

// CleanANSICodes removes ANSI escape sequences from input string.
func CleanANSICodes(input string) string {
	return ansiRegex.ReplaceAllString(input, "")
}
