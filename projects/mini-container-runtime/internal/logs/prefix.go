package logs

// PrefixLogs prepends a prefix string to each log line.
func PrefixLogs(lines []string, prefix string) []string {
	if prefix == "" || len(lines) == 0 {
		return lines
	}

	result := make([]string, len(lines))
	for i, line := range lines {
		result[i] = prefix + line
	}
	return result
}
