package logs

// SliceLogs returns lines from start index to end index (inclusive).
func SliceLogs(lines []string, start, end int) []string {
	if len(lines) == 0 {
		return nil
	}
	if start < 0 {
		start = 0
	}
	if end >= len(lines) {
		end = len(lines) - 1
	}
	if start > end || start >= len(lines) {
		return nil
	}

	return lines[start : end+1]
}
