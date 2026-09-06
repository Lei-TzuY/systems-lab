package logs

// FilterLogRate throttles maximum lines returned per second limit.
func FilterLogRate(lines []string, maxLinesPerSec int) []string {
	if maxLinesPerSec <= 0 || len(lines) <= maxLinesPerSec {
		return lines
	}

	return lines[:maxLinesPerSec]
}
