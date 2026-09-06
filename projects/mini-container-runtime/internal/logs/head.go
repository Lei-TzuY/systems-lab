package logs

// HeadLogs returns only the first headCount lines.
func HeadLogs(lines []string, headCount int) []string {
	if headCount <= 0 || len(lines) <= headCount {
		return lines
	}
	return lines[:headCount]
}
